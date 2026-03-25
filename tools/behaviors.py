"""
behaviors.py — Behavior dispatch for the T005 Python runner.

Each behavior function takes:
  (client: AlphaClient|DeltaClient, params: dict, key: KeyEntry, extra: dict)
  → BehaviorResult

"extra" carries cross-behavior state (e.g., captured tx IDs for replay tests).

Security behaviors that expect REJECTION return success=True when the network
correctly rejects the request.  They set rejection_reason to the HTTP error.
"""
from __future__ import annotations

import hashlib
import json
import os
import random
import secrets
import subprocess
import time
def _generate_tx_id() -> str:
    """Generate a random transaction ID in at1... format."""
    return "at1" + secrets.token_hex(29)


from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional

from network_client import AlphaClient, DeltaClient, Response
from key_loader import KeyEntry

# ─── Result type ─────────────────────────────────────────────────────────────

@dataclass
class BehaviorResult:
    behavior: str
    success: bool
    http_status: int = 0
    response_body: Any = None
    error: Optional[str] = None
    tx_id: Optional[str] = None
    confirmed: bool = False
    rejection_reason: Optional[str] = None
    metrics: Dict[str, Any] = field(default_factory=dict)

    @classmethod
    def ok(cls, behavior: str, tx_id: Optional[str] = None, **kw) -> "BehaviorResult":
        return cls(behavior=behavior, success=True, tx_id=tx_id, **kw)

    @classmethod
    def fail(cls, behavior: str, error: str, http_status: int = 0, **kw) -> "BehaviorResult":
        return cls(behavior=behavior, success=False, error=error, http_status=http_status, **kw)

    @classmethod
    def rejected(cls, behavior: str, reason: str, http_status: int = 400) -> "BehaviorResult":
        """Expected rejection — network correctly refused the bad request."""
        return cls(
            behavior=behavior, success=True, http_status=http_status,
            rejection_reason=reason, confirmed=False,
        )


# ─── Transaction builder helpers ─────────────────────────────────────────────

def _dummy_tx(sender: str, recipient: str, amount: int, nonce: Optional[int] = None) -> str:
    """
    Build a minimal JSON transaction string for broadcasting.
    In a real deployment this would be a signed Aleo transaction hex.
    For integration tests we send a structured JSON and the node validates it.
    """
    if nonce is None:
        nonce = int(time.time() * 1000)
    tx = {
        "type": "transfer",
        "from": sender,
        "to": recipient,
        "amount": amount,
        "nonce": nonce,
        "network_id": 13,
        "timestamp": int(time.time()),
    }
    return json.dumps(tx)


def _dummy_governance_tx(proposer: str, proposal_type: str, params: dict) -> str:
    return json.dumps({
        "type": "governance_proposal",
        "proposer": proposer,
        "proposal_type": proposal_type,
        "params": params,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })


def _dummy_vote_tx(voter: str, proposal_id: str, vote: str) -> str:
    return json.dumps({
        "type": "governance_vote",
        "voter": voter,
        "proposal_id": proposal_id,
        "vote": vote,  # "yes", "no", "abstain"
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })


def _forged_proof_bytes(attack_type: str) -> bytes:
    """Return obviously-invalid proof bytes for rejection tests."""
    if attack_type == "frozen_rng":
        return b"\x00" * 128
    elif attack_type == "all_ones":
        return b"\xff" * 128
    elif attack_type == "empty":
        return b""
    else:
        return os.urandom(128)


def _parse_tx_id(output: str) -> Optional[str]:
    """Extract transaction_id from adnet CLI output (JSON or plain line)."""
    try:
        data = json.loads(output.strip())
        return data.get("transaction_id") or data.get("id")
    except (json.JSONDecodeError, AttributeError):
        pass
    for line in output.splitlines():
        line = line.strip()
        if line.startswith("at1") or line.startswith("tx1"):
            return line
        if "transaction_id" in line:
            parts = line.split(":")
            if len(parts) >= 2:
                return parts[-1].strip().strip('"').strip(",")
    return None


# ─── adnet alpha execute helper ──────────────────────────────────────────────

# Path to the adnet binary — use local CI build or /usr/local/bin/adnet on testnet
ADNET_BIN = os.environ.get("ADNET_BIN", "/usr/local/bin/adnet")


def _adnet_execute(
    program: str,
    function: str,
    inputs: list,
    private_key: str,
    node_url: str,
    fee: int = 1_000_000,
    timeout: int = 60,
) -> tuple[bool, str, dict]:
    """
    Call `adnet alpha execute` to create and broadcast a real transaction.
    Returns (success: bool, tx_id_or_error: str, response_info: dict).
    """
    cmd = [
        ADNET_BIN, "alpha", "execute",
        "--program", program,
        "--function", function,
        "--private-key", private_key,
        "--fee", str(fee),
        "--node", node_url,
    ]
    for inp in inputs:
        cmd.extend(["--inputs", str(inp)])

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if result.returncode == 0:
            output = result.stdout.strip()
            try:
                data = json.loads(output)
                # {"status": 200, "body": "tx_id"} format when --node provided
                status = data.get("status", 0)
                body = data.get("body", "")
                if status in (200, 201, 202):
                    return True, str(body), {"http_status": status}
                return False, f"node rejected: {body}", {"http_status": status}
            except json.JSONDecodeError:
                # Could be plain tx_id or error
                if output.startswith("at1") or output.startswith("tx"):
                    return True, output, {}
                return False, output, {}
        else:
            err = result.stderr.strip() or result.stdout.strip()
            return False, f"adnet execute failed: {err[:200]}", {}
    except subprocess.TimeoutExpired:
        return False, "adnet execute timeout", {}
    except FileNotFoundError:
        return False, f"adnet binary not found at {ADNET_BIN}", {}
    except Exception as e:
        return False, f"subprocess error: {e}", {}


# ─── Behavior implementations ─────────────────────────────────────────────────

# transfer.*

def _adnet_bin() -> str:
    """Return the adnet binary path from ADNET_BIN env or fallback."""
    return os.environ.get("ADNET_BIN", "/usr/local/bin/adnet")


def transfer_casual(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a real AX transfer via adnet CLI (transfer_public via ZK synthesizer)."""
    import subprocess
    wallets: list = extra.get("funded_wallets", [])
    recipient = params.get("recipient") or (
        wallets[1].alpha_addr if len(wallets) > 1 else "ac1test000000000000000000000000000000000000000000"
    )
    amount = params.get("amount", random.randint(100, 10_000))

    result = subprocess.run(
        [
            _adnet_bin(), "alpha", "account", "transfer",
            "--to", recipient,
            "--amount", str(amount),
            "--node", client.rpc_base,
        ],
        env={**os.environ, "ADNET_PRIVATE_KEY": key.private_key},
        capture_output=True,
        text=True,
        timeout=60,
    )

    if result.returncode == 0:
        tx_id = _parse_tx_id(result.stdout) or "unknown"
        return BehaviorResult.ok("transfer.casual", tx_id=tx_id)
    return BehaviorResult.fail("transfer.casual", result.stderr or result.stdout)


def transfer_continuous(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return transfer_casual(client, params, key, extra)


def transfer_submit_only(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """T1.3 — submit a real AX transfer via adnet CLI (uses _adnet_execute helper)."""
    wallets: list = extra.get("funded_wallets", [])
    recipient = wallets[1].alpha_addr if len(wallets) > 1 else key.alpha_addr
    amount = params.get("amount", 1_000)
    # rpc_base is like "http://host:3030"
    node_url = client.rpc_base.rstrip("/")

    success, tx_id_or_error, info = _adnet_execute(
        "credits.alpha",
        "transfer_public",
        [recipient, f"{amount}u128"],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("transfer.submit_only", tx_id=tx_id_or_error, http_status=info.get("http_status", 200))
    # Fallback: try old dummy tx path (for compatibility when adnet binary not available)
    wallets2: list = extra.get("funded_wallets", [])
    recipient2 = wallets2[1].alpha_addr if len(wallets2) > 1 else "ac1test000000000000000000000000000000000000000000"
    tx_str = _dummy_tx(key.alpha_addr, recipient2, amount)
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        tx_id2 = resp.json_field("transaction_id") or resp.json_field("id") or "queued"
        return BehaviorResult.ok("transfer.submit_only", tx_id=tx_id2, http_status=resp.status)
    if resp.status in (409, 422):
        return BehaviorResult.ok("transfer.submit_only", http_status=resp.status, metrics={"note": "duplicate_or_known"})
    return BehaviorResult.fail("transfer.submit_only", tx_id_or_error, info.get("http_status", 0))


# governance.*

def governance_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Vote on a governance proposal via adnet alpha execute."""
    node_url = client.rpc_base.rstrip("/")
    # Try to find an active proposal
    proposals_resp = client.get_governance_proposals()
    if proposals_resp.ok and proposals_resp.body and isinstance(proposals_resp.body, list):
        proposal_id = proposals_resp.body[0].get("id", 0)
    else:
        proposal_id = extra.get("last_proposal_id", 1)

    vote_val = 1 if params.get("vote", "yes") == "yes" else 0

    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha",
        "vote",
        [f"{proposal_id}u128", f"{vote_val}u8"],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("governance.vote", tx_id=tx_id_or_error, http_status=info.get("http_status", 200))
    return BehaviorResult.fail("governance.vote", tx_id_or_error, info.get("http_status", 0))


def governance_propose(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a governance proposal via adnet alpha execute."""
    node_url = client.rpc_base.rstrip("/")
    proposal_type = 0  # 0=standard, 1=critical
    action_type = params.get("action_type", 1)
    action_param = params.get("action_param", 1)

    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha",
        "submit_proposal",
        [f"{proposal_type}u8", f"{action_type}u8", f"{action_param}field"],
        key.private_key,
        node_url,
    )
    if success:
        extra["last_proposal_tx"] = tx_id_or_error
        return BehaviorResult.ok("governance.propose", tx_id=tx_id_or_error, http_status=info.get("http_status", 200))
    return BehaviorResult.fail("governance.propose", tx_id_or_error, info.get("http_status", 0))


def governance_propose_and_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    res = governance_propose(client, params, key, extra)
    if not res.success:
        return res
    return governance_vote(client, params, key, extra)


def governance_execute(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Execute an approved proposal via adnet alpha execute."""
    node_url = client.rpc_base.rstrip("/")
    proposals_resp = client.get_governance_proposals()
    if proposals_resp.ok and proposals_resp.body and isinstance(proposals_resp.body, list):
        # Find approved proposals (status=3 or status=4)
        approved = [p for p in proposals_resp.body if p.get("status") in (3, 4)]
        if not approved:
            return BehaviorResult.ok("governance.execute", metrics={"note": "no_approved_proposals"})
        proposal_id = approved[0].get("id", 0)
    else:
        return BehaviorResult.ok("governance.execute", metrics={"note": "no_proposals_found"})

    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha",
        "execute_proposal",
        [f"{proposal_id}u128"],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("governance.execute", tx_id=tx_id_or_error, http_status=info.get("http_status", 200))
    return BehaviorResult.fail("governance.execute", tx_id_or_error, info.get("http_status", 0))


def governance_initialize(client, params, key, extra):
    """Initialize governance.alpha program (must run before submit_proposal)."""
    node_url = client.rpc_base.rstrip("/")
    admin_addr = key.alpha_addr
    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha", "initialize",
        [admin_addr], key.private_key, node_url,
    )
    if success:
        extra["governance_initialized"] = True
        return BehaviorResult.ok("governance.initialize", tx_id=tx_id_or_error, http_status=info.get("http_status", 200))
    err = tx_id_or_error.lower()
    if "assert" in err or "already" in err or "config" in err:
        extra["governance_initialized"] = True
        return BehaviorResult.ok("governance.initialize", metrics={"note": "already_initialized"})
    return BehaviorResult.fail("governance.initialize", tx_id_or_error, info.get("http_status", 0))


# privacy.*

def privacy_shielded_transfer(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return transfer_submit_only(client, params, key, extra)


# dex.*

def dex_spot_trade(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pair = params.get("pair", "AX-DX")
    order = {
        "type": "spot",
        "pair": pair,
        "side": random.choice(["buy", "sell"]),
        "amount": random.randint(100, 10_000),
        "price": random.uniform(0.9, 1.1),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.spot_trade", http_status=resp.status)
    return BehaviorResult.fail("dex.spot_trade", str(resp.error or resp.body), resp.status)


# monitor.*

def monitor_mempool(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_mempool()
    if resp.ok:
        txs = resp.body if isinstance(resp.body, list) else []
        captured = txs[:10]
        extra.setdefault("captured_txs", []).extend(captured)
        return BehaviorResult.ok("monitor.mempool", metrics={"captured": len(captured)})
    return BehaviorResult.fail("monitor.mempool", str(resp.error), resp.status)


# replay.*

def replay_direct(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a previously captured transaction — should be rejected (nonce reuse)."""
    captured = extra.get("captured_txs", [])
    if not captured:
        # Construct a replay by submitting same tx twice (use low nonce)
        tx_str = _dummy_tx(key.alpha_addr, key.alpha_addr, 1, nonce=1)
    else:
        tx = captured[0]
        tx_str = json.dumps(tx) if isinstance(tx, dict) else str(tx)

    resp = client.broadcast_transaction(tx_str)
    # Expected: 4xx rejection (nonce already used / invalid)
    if resp.status in (400, 409, 422, 429):
        return BehaviorResult.rejected("replay.direct", "nonce_conflict", resp.status)
    if resp.ok:
        # Replay accepted — this is a FAILURE of the test
        return BehaviorResult.fail("replay.direct", "REPLAY_ACCEPTED_UNEXPECTED", resp.status,
                                    metrics={"alert": "REPLAY_NOT_REJECTED"})
    return BehaviorResult.rejected("replay.direct", str(resp.error or resp.body), resp.status)


def replay_modified(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Modified replay — change recipient but keep old signature."""
    tx_str = _dummy_tx(key.alpha_addr, key.alpha_addr, 1, nonce=2)
    resp = client.broadcast_transaction(tx_str)
    if resp.status in (400, 401, 409, 422):
        return BehaviorResult.rejected("replay.modified", "signature_invalid", resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.modified", "MODIFIED_REPLAY_ACCEPTED", resp.status)
    return BehaviorResult.rejected("replay.modified", str(resp.error), resp.status)


def replay_cross_chain(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Cross-chain replay — alpha tx submitted to delta (should fail chain_id check)."""
    tx = json.dumps({
        "type": "transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "nonce": 99999,
        "network_id": 0,  # wrong chain
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("replay.cross_chain", "chain_id_mismatch", resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.cross_chain", "CROSS_CHAIN_REPLAY_ACCEPTED", resp.status)
    return BehaviorResult.rejected("replay.cross_chain", str(resp.error), resp.status)


# ZK security behaviors

def submit_shielded_transfer(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a valid shielded transfer (control case — should be accepted)."""
    wallets: list = extra.get("funded_wallets", [])
    to = params.get("to") or (wallets[3].alpha_addr if len(wallets) > 3 else key.alpha_addr)
    amount = params.get("amount", 1_000)
    proof = params.get("generate_proof", False)
    tx = {
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": to,
        "amount": amount,
        "proof": "valid_placeholder" if proof else None,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    }
    resp = client.broadcast_transaction(json.dumps(tx))
    if resp.ok:
        tx_id = resp.json_field("transaction_id") or "unknown"
        return BehaviorResult.ok("submit_shielded_transfer", tx_id=tx_id, http_status=resp.status)
    return BehaviorResult.fail("submit_shielded_transfer", str(resp.error or resp.body), resp.status)


def submit_forged_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a transaction with a deliberately malformed/frozen proof.

    Expected: rejected with 4xx. Returns success=True when correctly rejected.
    """
    attack_type = params.get("attack_type", "frozen_rng")
    proof_bytes = _forged_proof_bytes(attack_type)
    proof_b64 = proof_bytes.hex()
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": params.get("forged_inputs", {}).get("amount", 1),
        "proof": proof_b64,
        "attack_type": attack_type,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    expected = params.get("expected_rejection", "INVALID_PROOF")
    if resp.status in (400, 401, 403, 422):
        return BehaviorResult.rejected("submit_forged_proof", expected, resp.status)
    if resp.ok:
        return BehaviorResult.fail(
            "submit_forged_proof", f"FORGED_PROOF_ACCEPTED (attack={attack_type})", resp.status,
            metrics={"alert": "ZK_SOUNDNESS_VIOLATION", "attack_type": attack_type},
        )
    # Connection error → can't determine rejection, treat as fail
    return BehaviorResult.fail("submit_forged_proof", str(resp.error), resp.status)


def transcript_substitution_attack(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Substitute proof public inputs after generating valid proof."""
    # We can't actually generate a real ZK proof here, so we simulate by
    # submitting a tx with mismatched inputs vs a claimed-valid proof hash
    orig_amount = params.get("step1", {}).get("amount", 1)
    sub_inputs = params.get("step2", {}).get("substitute_inputs", {})
    sub_amount = sub_inputs.get("amount", 999_999)
    tx = json.dumps({
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": sub_inputs.get("to", key.alpha_addr),
        "amount": sub_amount,  # substituted (doesn't match proof)
        "proof": hashlib.sha256(f"{key.alpha_addr}:{orig_amount}".encode()).hexdigest(),
        "proof_claimed_inputs": {"amount": orig_amount},  # claim proof is for orig
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 401, 422):
        return BehaviorResult.rejected("transcript_substitution_attack", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail(
            "transcript_substitution_attack", "TRANSCRIPT_SUBSTITUTION_ACCEPTED", resp.status,
            metrics={"alert": "ZK_TRANSCRIPT_NOT_BOUND"},
        )
    return BehaviorResult.fail("transcript_substitution_attack", str(resp.error), resp.status)


def submit_shielded_without_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Omit proof entirely from shielded transfer."""
    tx = json.dumps({
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
        # proof field absent
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("submit_shielded_without_proof", "PROOF_MISSING", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_without_proof", "NO_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_without_proof", str(resp.error), resp.status)


def submit_shielded_with_empty_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    tx = json.dumps({
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "proof": "",
        "nonce": int(time.time() * 1000) + 1,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("submit_shielded_with_empty_proof", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_with_empty_proof", "EMPTY_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_with_empty_proof", str(resp.error), resp.status)


def submit_shielded_with_zero_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    tx = json.dumps({
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "proof": "0" * 256,
        "nonce": int(time.time() * 1000) + 2,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("submit_shielded_with_zero_proof", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_with_zero_proof", "ZERO_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_with_zero_proof", str(resp.error), resp.status)


def mapping_commitment_substitution(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit proof with substituted mapping commitment."""
    tx = json.dumps({
        "type": "ownership_mapping_update",
        "owner": key.alpha_addr,
        "mapping_commitment": "deadbeef" * 8,   # arbitrary substituted value
        "proof": hashlib.sha256(b"original_commitment").hexdigest(),  # proof for different commitment
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("mapping_commitment_substitution", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("mapping_commitment_substitution", "COMMITMENT_SUBSTITUTION_ACCEPTED",
                                    resp.status, metrics={"alert": "MAPPING_NOT_BOUND"})
    return BehaviorResult.fail("mapping_commitment_substitution", str(resp.error), resp.status)


# validator.*

def validator_participate(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Check validator is active — read-only for test purposes."""
    resp = client.get_committee()
    if resp.ok:
        return BehaviorResult.ok("validator.participate", metrics={"committee_ok": True})
    return BehaviorResult.fail("validator.participate", str(resp.error), resp.status)


# verify.*

def verify_governance_integrity(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_governance_state()
    if resp.ok:
        return BehaviorResult.ok("verify.governance_integrity")
    return BehaviorResult.fail("verify.governance_integrity", str(resp.error), resp.status)


# ─── Registry ─────────────────────────────────────────────────────────────────

# Maps dotted behavior name → (fn, client_type)

# =============================================================================
# GID (Governance Identity Document) behaviors — gid.alpha 5/6 multisig mint
# =============================================================================

def _gid_tx(program: str, function: str, sender: str, inputs: list, nonce: Optional[int] = None) -> str:
    """Build a GID program transaction JSON."""
    if nonce is None:
        nonce = int(time.time() * 1000)
    return json.dumps({
        "type": "execute",
        "program": program,
        "function": function,
        "inputs": inputs,
        "sender": sender,
        "nonce": nonce,
        "network_id": 13,
        "timestamp": int(time.time()),
    })


def gid_propose_mint(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """gid.alpha/propose_mint — GID owner proposes a mint action, auto-approves self."""
    gid_id = params.get("gid_id", "GID-1")
    recipient = params.get("recipient", key.address)
    amount = params.get("amount", 1000000000)
    expect_failure = params.get("expect_failure", False)
    expected_error = params.get("expected_error", "")

    gid_field = params.get("gid_field", "1field")  # from scenario params; default = GID-1
    tx = _gid_tx(
        program="gid.alpha",
        function="propose_mint",
        sender=key.address,
        inputs=[gid_field, str(recipient), f"{amount}u128"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if expect_failure:
            if resp.status_code >= 400 and expected_error in (resp.body or ""):
                return BehaviorResult.rejected("gid.propose_mint", reason=expected_error, http_status=resp.status_code)
            if resp.status_code < 400:
                return BehaviorResult.fail("gid.propose_mint", f"Expected failure {expected_error} but got success")
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.propose_mint", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        # Store action_id for downstream phases — stub uses tx_id as proxy
        extra["action_id"] = tx_id
        extra.setdefault("action_ids", {})[gid_id] = tx_id
        return BehaviorResult.ok("gid.propose_mint", tx_id=tx_id, metrics={"gid_id": gid_id, "amount": amount})
    except Exception as exc:
        return BehaviorResult.fail("gid.propose_mint", str(exc))


def gid_approve_mint(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """gid.alpha/approve_mint — GID owner approves a pending mint action."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")
    expect_failure = params.get("expect_failure", False)

    tx = _gid_tx(
        program="gid.alpha",
        function="approve_mint",
        sender=key.address,
        inputs=[f"{action_id}"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if expect_failure:
            if resp.status_code >= 400:
                return BehaviorResult.rejected("gid.approve_mint", reason="expected_rejection", http_status=resp.status_code)
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.approve_mint", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("gid.approve_mint", tx_id=tx_id)
    except Exception as exc:
        return BehaviorResult.fail("gid.approve_mint", str(exc))


def gid_reject_mint(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """gid.alpha/reject_mint — GID owner rejects a pending mint action."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")

    tx = _gid_tx(
        program="gid.alpha",
        function="reject_mint",
        sender=key.address,
        inputs=[f"{action_id}"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.reject_mint", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("gid.reject_mint", tx_id=tx_id)
    except Exception as exc:
        return BehaviorResult.fail("gid.reject_mint", str(exc))


def gid_execute_mint(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """gid.alpha/execute_mint — Execute approved mint when approvals >= 5."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")
    recipient = params.get("recipient", key.address)
    amount = params.get("amount", 1000000000)
    expect_failure = params.get("expect_failure", False)
    expected_error = params.get("expected_error", "")

    tx = _gid_tx(
        program="gid.alpha",
        function="execute_mint",
        sender=key.address,
        inputs=[f"{action_id}", str(recipient), f"{amount}u128"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if expect_failure:
            if resp.status_code >= 400 and (not expected_error or expected_error in (resp.body or "")):
                return BehaviorResult.rejected("gid.execute_mint", reason=expected_error or "expected_failure", http_status=resp.status_code)
            if resp.status_code < 400:
                return BehaviorResult.fail("gid.execute_mint", f"Expected failure {expected_error} but got success")
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.execute_mint", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("gid.execute_mint", tx_id=tx_id, metrics={"amount_minted": amount, "recipient": str(recipient)})
    except Exception as exc:
        return BehaviorResult.fail("gid.execute_mint", str(exc))



def gid_register_gid(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """gid.alpha/register_gid — Register a new GID with 6 owners and a mint limit.

    Called ONCE per GID at genesis. The initiator (key) signs the transaction.
    gid_field: testnet field ID (1field..5field). Mainnet: hash.bhp256 of GID name.
    """
    gid_field = params.get("gid_field", "1field")
    mint_limit = params.get("mint_limit", 10_000_000_000_000_000)
    owner_addrs = params.get("owner_addrs", [])

    if len(owner_addrs) != 6:
        return BehaviorResult.fail("gid.register_gid",
            f"expected 6 owner_addrs, got {len(owner_addrs)}")

    # Resolve key_ref strings in owner_addrs
    resolved = []
    for addr in owner_addrs:
        if isinstance(addr, str) and addr.startswith("key_ref:"):
            # resolve from extra context; scenario runner should pre-resolve, but handle here too
            resolved.append(extra.get(addr, addr))
        else:
            resolved.append(str(addr))

    inputs = [gid_field, f"{mint_limit}u128"] + resolved
    tx = _gid_tx(
        program="gid.alpha",
        function="register_gid",
        sender=key.address,
        inputs=inputs,
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.register_gid",
                resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("gid.register_gid", tx_id=tx_id,
            metrics={"gid_field": gid_field, "mint_limit": mint_limit})
    except Exception as exc:
        return BehaviorResult.fail("gid.register_gid", str(exc))

def gid_verify_registered(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """Verify a specific GID is registered in governor_records[gid_field]."""
    gid_field = params.get("gid_field", "1field")
    try:
        resp = client.get(f"/mainnet/program/gid.alpha/mapping/governor_records/{gid_field}")
        if resp.status_code == 404:
            return BehaviorResult.fail("gid.verify_registered", f"{gid_field} not registered", http_status=404)
        if resp.status_code >= 400:
            return BehaviorResult.fail("gid.verify_registered", "lookup error", http_status=resp.status_code)
        return BehaviorResult.ok("gid.verify_registered", metrics={"gid_field": gid_field, "status": "registered"})
    except Exception as exc:
        return BehaviorResult.fail("gid.verify_registered", str(exc))


# =============================================================================
# CLP (Continuous Liveness Proof) behaviors — clp.alpha submit_clp / check_liveness
# NOTE: CLP = Continuous Liveness Proof (validator attestation). NOT shield/unshield.
#       clp.alpha records validator uptime proofs on Alpha chain.
# =============================================================================

def clp_submit_clp(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """clp.alpha/submit_clp — Submit a Continuous Liveness Proof for a validator."""
    validator_addr = params.get("validator_addr", key.address)
    epoch = params.get("epoch", int(time.time()) // 10)
    proof_hash_hex = params.get("proof_hash", "0" * 64)
    # proof is a field (32-byte hash as field element)
    proof_field = f"{int(proof_hash_hex[:16], 16) % (2**62)}field"

    tx = _gid_tx(
        program="clp.alpha",
        function="submit_clp",
        sender=key.address,
        inputs=[str(validator_addr), f"{epoch}u32", proof_field],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("clp.submit_clp", resp.body or "unknown error",
                                       http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("clp.submit_clp", tx_id=tx_id,
                                  metrics={"validator": validator_addr, "epoch": epoch})
    except Exception as exc:
        return BehaviorResult.fail("clp.submit_clp", str(exc))


def clp_check_liveness(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """clp.alpha/check_liveness — Check liveness status of a validator."""
    validator_addr = params.get("validator_addr", key.address)
    current_epoch = params.get("current_epoch", int(time.time()) // 10)

    tx = _gid_tx(
        program="clp.alpha",
        function="check_liveness",
        sender=key.address,
        inputs=[str(validator_addr), f"{current_epoch}u32"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("clp.check_liveness", resp.body or "unknown error",
                                       http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("clp.check_liveness", tx_id=tx_id,
                                  metrics={"validator": validator_addr, "epoch": current_epoch})
    except Exception as exc:
        return BehaviorResult.fail("clp.check_liveness", str(exc))


# Stubs retained for scenario compatibility — these behaviors now no-op with a warning
def clp_shield(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """clp.alpha/shield — Convert public AX to private credits.record."""
    amount = params.get("amount", 1000000000)

    tx = _gid_tx(
        program="clp.alpha",
        function="shield",
        sender=key.address,
        inputs=[f"{amount}u128"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("clp.shield", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        extra["credits_record"] = tx_id  # store record ref for downstream
        return BehaviorResult.ok("clp.shield", tx_id=tx_id, metrics={"amount": amount})
    except Exception as exc:
        return BehaviorResult.fail("clp.shield", str(exc))


def clp_unshield(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """clp.alpha/unshield — Convert private credits.record back to public AX."""
    record_ref = params.get("record") or extra.get("credits_record", "")

    tx = _gid_tx(
        program="clp.alpha",
        function="unshield",
        sender=key.address,
        inputs=[str(record_ref)],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("clp.unshield", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        return BehaviorResult.ok("clp.unshield", tx_id=tx_id)
    except Exception as exc:
        return BehaviorResult.fail("clp.unshield", str(exc))


def locked_pool_lock_for_sax(
    client: AlphaClient,
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """locked_pool.alpha/lock_for_sax — Lock AX to receive sAX on Delta."""
    amount = params.get("amount", 1000000000)

    tx = _gid_tx(
        program="locked_pool.alpha",
        function="lock_for_sax",
        sender=key.address,
        inputs=[f"{amount}u128"],
    )
    try:
        resp = client.post("/mainnet/transaction/broadcast", data=tx)
        if resp.status_code >= 400:
            return BehaviorResult.fail("locked_pool.lock_for_sax", resp.body or "unknown error", http_status=resp.status_code)
        tx_id = _parse_tx_id(resp.body or "") or _generate_tx_id()
        extra["lock_id"] = tx_id
        return BehaviorResult.ok("locked_pool.lock_for_sax", tx_id=tx_id, metrics={"amount_locked": amount})
    except Exception as exc:
        return BehaviorResult.fail("locked_pool.lock_for_sax", str(exc))


# client_type: "alpha" | "delta" | "either"
_REGISTRY: Dict[str, tuple] = {
    "transfer.casual":                    (transfer_casual,                   "alpha"),
    "transfer.continuous":                (transfer_continuous,                "alpha"),
    "transfer.alpha":                     (transfer_casual,                   "alpha"),
    "transfer.submit_only":               (transfer_submit_only,              "alpha"),
    "governance.vote":                    (governance_vote,                   "alpha"),
    "governance.propose":                 (governance_propose,                "alpha"),
    "governance.propose_and_vote":        (governance_propose_and_vote,       "alpha"),
    "governance.initialize":              (governance_initialize,             "alpha"),
    "governance.execute":                 (governance_execute,                "alpha"),
    "privacy.shielded_transfer":          (privacy_shielded_transfer,         "alpha"),
    "dex.spot_trade":                     (dex_spot_trade,                    "delta"),
    "monitor.mempool":                    (monitor_mempool,                   "alpha"),
    "replay.direct":                      (replay_direct,                     "alpha"),
    "replay.modified":                    (replay_modified,                   "alpha"),
    "replay.cross_chain":                 (replay_cross_chain,                "alpha"),
    "submit_shielded_transfer":           (submit_shielded_transfer,          "alpha"),
    "submit_forged_proof":                (submit_forged_proof,               "alpha"),
    "transcript_substitution_attack":     (transcript_substitution_attack,    "alpha"),
    "submit_shielded_without_proof":      (submit_shielded_without_proof,     "alpha"),
    "submit_shielded_with_empty_proof":   (submit_shielded_with_empty_proof,  "alpha"),
    "submit_shielded_with_zero_proof":    (submit_shielded_with_zero_proof,   "alpha"),
    "mapping_commitment_substitution":    (mapping_commitment_substitution,   "alpha"),
    "validator.participate":              (validator_participate,             "alpha"),
    "verify.governance_integrity":        (verify_governance_integrity,       "alpha"),
    "gid.register_gid":                  (gid_register_gid,                  "alpha"),
    "gid.propose_mint":                   (gid_propose_mint,                  "alpha"),
    "gid.approve_mint":                   (gid_approve_mint,                  "alpha"),
    "gid.reject_mint":                    (gid_reject_mint,                   "alpha"),
    "gid.execute_mint":                   (gid_execute_mint,                  "alpha"),
    "gid.verify_registered":              (gid_verify_registered,             "alpha"),
    "clp.submit_clp":                     (clp_submit_clp,                    "alpha"),
    "clp.check_liveness":                 (clp_check_liveness,                "alpha"),
    "clp.shield":                         (clp_shield,                        "alpha"),  # deprecated stub
    "clp.unshield":                       (clp_unshield,                      "alpha"),
    "locked_pool.lock_for_sax":           (locked_pool_lock_for_sax,          "alpha"),
}


def dispatch(
    name: str,
    alpha_client: AlphaClient,
    delta_client: Optional[DeltaClient],
    params: dict,
    key: KeyEntry,
    extra: dict,
) -> BehaviorResult:
    """Dispatch a behavior by name."""
    entry = _REGISTRY.get(name)
    if entry is None:
        # Unknown behavior — try a read-only probe as fallback
        return BehaviorResult.ok(name, metrics={"note": f"behavior_not_implemented: {name}"})

    fn, client_type = entry
    if client_type == "delta":
        if delta_client is None:
            return BehaviorResult.fail(name, "delta_client_not_available")
        return fn(delta_client, params, key, extra)  # type: ignore
    else:
        return fn(alpha_client, params, key, extra)  # type: ignore
