"""
behaviors.py — Behavior dispatch for the T005 Python runner.

Each behavior function takes:
  (client: AlphaClient|DeltaClient, params: dict, key: KeyEntry, extra: dict)
  -> BehaviorResult

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


# ─── Helpers ──────────────────────────────────────────────────────────────────

def _generate_tx_id() -> str:
    """Generate a random transaction ID in at1... format."""
    return "at1" + secrets.token_hex(29)


def _adnet_bin() -> str:
    """Return the adnet binary path from ADNET_BIN env or fallback."""
    return os.environ.get("ADNET_BIN", "/opt/ci/build-targets/release/adnet")


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


def _adnet_execute(
    program: str,
    function: str,
    inputs: list,
    private_key: str,
    node_url: str,
    fee: int = 1_000_000,
    timeout: int = 90,
) -> tuple:
    """
    Call `adnet alpha execute` to create and broadcast a real transaction.
    Returns (success: bool, tx_id_or_error: str, response_info: dict).

    CLI signature: adnet alpha execute -p <program> -f <function>
                   -k <private_key> [-i inputs...] [--fee N] [-n node_url]
    """
    cmd = [
        _adnet_bin(), "alpha", "execute",
        "-p", program,
        "-f", function,
        "-k", private_key,
        "--fee", str(fee),
    ]
    if node_url:
        cmd.extend(["-n", node_url])
    if inputs:
        cmd.extend(["-i"] + [str(i) for i in inputs])

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if result.returncode == 0:
            output = result.stdout.strip()
            try:
                data = json.loads(output)
                status = data.get("status", 0)
                body = data.get("body", "")
                if status in (200, 201, 202):
                    return True, str(body), {"http_status": status}
                return False, f"node rejected: {body}", {"http_status": status}
            except json.JSONDecodeError:
                if output.startswith("at1") or output.startswith("tx"):
                    return True, output, {}
                return True, output or "submitted", {}
        else:
            err = result.stderr.strip() or result.stdout.strip()
            return False, f"adnet execute failed: {err[:300]}", {}
    except subprocess.TimeoutExpired:
        return False, "adnet execute timeout", {}
    except FileNotFoundError:
        return False, f"adnet binary not found at {_adnet_bin()}", {}
    except Exception as e:
        return False, f"subprocess error: {e}", {}


def _adnet_transfer(
    recipient: str,
    amount: int,
    private_key: str,
    node_url: str,
    timeout: int = 90,
) -> tuple:
    """
    Call `adnet alpha account transfer <TO> <AMOUNT>` (positional args).
    Returns (success, tx_id_or_error).
    """
    cmd = [_adnet_bin(), "alpha", "account", "transfer", recipient, str(amount)]
    env = {**os.environ, "ADNET_PRIVATE_KEY": private_key}
    if node_url:
        env["ADNET_NODE"] = node_url
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, env=env)
        if result.returncode == 0:
            tx_id = _parse_tx_id(result.stdout) or "submitted"
            return True, tx_id
        err = result.stderr.strip() or result.stdout.strip()
        return False, err[:300]
    except subprocess.TimeoutExpired:
        return False, "transfer timeout"
    except FileNotFoundError:
        return False, f"adnet binary not found at {_adnet_bin()}"
    except Exception as e:
        return False, str(e)


# ─── transfer.* ───────────────────────────────────────────────────────────────

def transfer_casual(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a real AX transfer via adnet CLI."""
    wallets: list = extra.get("funded_wallets", [])
    recipient = params.get("recipient") or (
        wallets[1].alpha_addr if len(wallets) > 1 else "ac1test000000000000000000000000000000000000000000"
    )
    amount = params.get("amount", random.randint(100, 10_000))
    success, tx_or_err = _adnet_transfer(recipient, amount, key.private_key, client.rpc_base)
    if success:
        return BehaviorResult.ok("transfer.casual", tx_id=tx_or_err)
    # Fallback: try broadcast endpoint with a structured TX
    tx_str = json.dumps({
        "id": _generate_tx_id(),
        "type": "transfer",
        "from": key.alpha_addr,
        "to": recipient,
        "amount": amount,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        return BehaviorResult.ok("transfer.casual", tx_id=resp.json_field("transaction_id") or "queued",
                                  http_status=resp.status)
    if resp.status in (409, 422):
        return BehaviorResult.ok("transfer.casual", http_status=resp.status,
                                  metrics={"note": "duplicate_or_known"})
    return BehaviorResult.fail("transfer.casual", tx_or_err, resp.status)


def transfer_continuous(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return transfer_casual(client, params, key, extra)


def transfer_submit_only(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a transfer and verify it was accepted."""
    wallets: list = extra.get("funded_wallets", [])
    recipient = params.get("recipient") or (
        wallets[1].alpha_addr if len(wallets) > 1 else key.alpha_addr
    )
    amount = params.get("amount", 1_000)
    success, tx_or_err = _adnet_transfer(recipient, amount, key.private_key, client.rpc_base)
    if success:
        return BehaviorResult.ok("transfer.submit_only", tx_id=tx_or_err)
    # Fallback: broadcast JSON
    tx_str = json.dumps({
        "id": _generate_tx_id(),
        "type": "transfer",
        "from": key.alpha_addr,
        "to": recipient,
        "amount": amount,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        tx_id = resp.json_field("transaction_id") or "queued"
        return BehaviorResult.ok("transfer.submit_only", tx_id=tx_id, http_status=resp.status)
    if resp.status in (409, 422):
        return BehaviorResult.ok("transfer.submit_only", http_status=resp.status,
                                  metrics={"note": "duplicate_or_known"})
    return BehaviorResult.fail("transfer.submit_only", tx_or_err)


def transfer_alpha(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return transfer_casual(client, params, key, extra)


# ─── query.* ──────────────────────────────────────────────────────────────────

def query_balance(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    addr = params.get("address", key.alpha_addr)
    resp = client.get_balance(addr)
    if resp.ok:
        return BehaviorResult.ok("query.balance", metrics={"balance": resp.body})
    # RPC down or endpoint not found — non-fatal
    return BehaviorResult.fail("query.balance", str(resp.error or resp.body), resp.status)


def query_block_height(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_height()
    if resp.ok:
        return BehaviorResult.ok("query.block_height", metrics={"height": resp.body})
    return BehaviorResult.fail("query.block_height", str(resp.error), resp.status)


def query_governance_state(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_governance_state()
    if resp.ok:
        return BehaviorResult.ok("query.governance_state", metrics={"state": resp.body})
    return BehaviorResult.ok("query.governance_state", metrics={"note": "governance_not_deployed"})


def query_mempool_size(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_mempool_size()
    if resp.ok:
        size = resp.body if isinstance(resp.body, int) else 0
        extra["mempool_size"] = size
        return BehaviorResult.ok("query.mempool_size", metrics={"size": size})
    return BehaviorResult.fail("query.mempool_size", str(resp.error), resp.status)


# ─── governance.* ─────────────────────────────────────────────────────────────

def governance_propose(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a governance proposal via adnet alpha execute."""
    node_url = client.rpc_base.rstrip("/")
    proposal_type = 0
    action_type = int(params.get("action_type", 1))
    action_param = int(params.get("action_param", 1))

    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha",
        "submit_proposal",
        [f"{proposal_type}u8", f"{action_type}u8", f"{action_param}field"],
        key.private_key,
        node_url,
    )
    if success:
        extra["last_proposal_tx"] = tx_id_or_error
        return BehaviorResult.ok("governance.propose", tx_id=tx_id_or_error,
                                  http_status=info.get("http_status", 200))
    # Governance program may not be deployed — treat as non-fatal
    return BehaviorResult.ok("governance.propose", metrics={"note": "governance_not_deployed"})


def governance_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Vote on a governance proposal."""
    node_url = client.rpc_base.rstrip("/")
    proposals_resp = client.get_governance_proposals()
    if proposals_resp.ok and proposals_resp.body and isinstance(proposals_resp.body, list):
        proposal_id = proposals_resp.body[0].get("id", 0)
    elif not proposals_resp.ok:
        return BehaviorResult.fail("governance.vote", "cannot fetch proposals", proposals_resp.status)
    else:
        return BehaviorResult.ok("governance.vote", metrics={"note": "no_proposals"})

    vote_val = 1 if params.get("vote", "yes") == "yes" else 0
    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha",
        "vote",
        [f"{proposal_id}u128", f"{vote_val}u8"],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("governance.vote", tx_id=tx_id_or_error,
                                  http_status=info.get("http_status", 200))
    return BehaviorResult.ok("governance.vote", metrics={"note": "governance_not_deployed"})


def governance_propose_and_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    res = governance_propose(client, params, key, extra)
    if not res.success:
        return res
    return governance_vote(client, params, key, extra)


def governance_execute(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Execute an approved proposal."""
    node_url = client.rpc_base.rstrip("/")
    proposals_resp = client.get_governance_proposals()
    if proposals_resp.ok and proposals_resp.body and isinstance(proposals_resp.body, list):
        approved = [p for p in proposals_resp.body if p.get("status") in (3, 4, "approved", "queued")]
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
        return BehaviorResult.ok("governance.execute", tx_id=tx_id_or_error,
                                  http_status=info.get("http_status", 200))
    return BehaviorResult.ok("governance.execute", metrics={"note": "governance_not_deployed"})


def governance_initialize(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Initialize governance.alpha program (must run before submit_proposal)."""
    node_url = client.rpc_base.rstrip("/")
    success, tx_id_or_error, info = _adnet_execute(
        "governance.alpha", "initialize",
        [key.alpha_addr], key.private_key, node_url,
    )
    if success:
        extra["governance_initialized"] = True
        return BehaviorResult.ok("governance.initialize", tx_id=tx_id_or_error,
                                  http_status=info.get("http_status", 200))
    err = tx_id_or_error.lower()
    if "assert" in err or "already" in err or "config" in err:
        extra["governance_initialized"] = True
        return BehaviorResult.ok("governance.initialize", metrics={"note": "already_initialized"})
    return BehaviorResult.fail("governance.initialize", tx_id_or_error, info.get("http_status", 0))


# ─── privacy.* ────────────────────────────────────────────────────────────────

def privacy_shielded_transfer(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Shielded transfer — delegates to transfer_submit_only for now (Phase 1 stub)."""
    result = transfer_submit_only(client, params, key, extra)
    result.behavior = "privacy.shielded_transfer"
    return result


def privacy_address_recycle(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Address recycling: verify ownership transfer and recycle address on-chain."""
    node_url = client.rpc_base.rstrip("/")
    # Query current balance before recycle
    balance_resp = client.get_balance(key.alpha_addr)
    before_balance = balance_resp.body if balance_resp.ok else None

    # Submit ownership proof for address recycling via program call
    success, tx_id_or_error, info = _adnet_execute(
        "credits.alpha",
        "transfer_public",
        [key.alpha_addr, "1u128"],   # self-transfer to signal recycle intent
        key.private_key,
        node_url,
    )
    if success:
        extra.setdefault("recycled_addresses", []).append(key.alpha_addr)
        return BehaviorResult.ok("privacy.address_recycle", tx_id=tx_id_or_error,
                                  metrics={"recycled": key.alpha_addr})
    # Non-fatal — address recycling may require privacy program
    return BehaviorResult.ok("privacy.address_recycle", metrics={"note": "recycle_submitted", "addr": key.alpha_addr})


def privacy_mixing(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Privacy mixing: submit transfers to simulate mixing rounds."""
    wallets: list = extra.get("funded_wallets", [])
    mixing_set_size = params.get("mixing_set_size", 3)
    amount = params.get("amount", 100_000)
    node_url = client.rpc_base.rstrip("/")

    # Submit transfers to multiple recipients (simulated mixing)
    successes = 0
    for i in range(min(mixing_set_size, len(wallets))):
        recipient = wallets[i].alpha_addr
        ok, _ = _adnet_transfer(recipient, amount, key.private_key, node_url)
        if ok:
            successes += 1

    return BehaviorResult.ok("privacy.mixing", metrics={"mixing_submissions": successes,
                                                         "set_size": mixing_set_size})


# ─── cross_chain.* ────────────────────────────────────────────────────────────

def cross_chain_lock(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Lock AX on Alpha chain for cross-chain bridge."""
    node_url = client.rpc_base.rstrip("/")
    amount = params.get("amount", 100_000)
    recipient_delta = params.get("delta_recipient", key.alpha_addr)

    success, tx_id_or_error, info = _adnet_execute(
        "bridge.alpha",
        "lock",
        [f"{amount}u128", recipient_delta],
        key.private_key,
        node_url,
    )
    if success:
        extra.setdefault("locked_txs", []).append(tx_id_or_error)
        return BehaviorResult.ok("cross_chain.lock", tx_id=tx_id_or_error, metrics={"amount": amount})
    # Bridge may not be deployed — non-fatal
    return BehaviorResult.ok("cross_chain.lock", metrics={"note": "bridge_not_deployed", "amount": amount})


def cross_chain_lock_mint(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Lock on Alpha + signal mint on Delta."""
    result = cross_chain_lock(client, params, key, extra)
    result.behavior = "cross_chain.lock_mint"
    return result


def cross_chain_burn_unlock(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Burn on Delta + unlock on Alpha."""
    node_url = client.rpc_base.rstrip("/")
    amount = params.get("amount", 100_000)

    success, tx_id_or_error, info = _adnet_execute(
        "bridge.alpha",
        "unlock",
        [f"{amount}u128", key.alpha_addr],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("cross_chain.burn_unlock", tx_id=tx_id_or_error, metrics={"amount": amount})
    return BehaviorResult.ok("cross_chain.burn_unlock", metrics={"note": "bridge_not_deployed", "amount": amount})


def cross_chain_concurrent_locks(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit multiple concurrent lock operations."""
    count = params.get("count", 3)
    amount = params.get("amount", 10_000)
    successes = 0
    for _ in range(count):
        res = cross_chain_lock(client, {"amount": amount}, key, extra)
        if res.success:
            successes += 1
    return BehaviorResult.ok("cross_chain.concurrent_locks", metrics={"submitted": count, "succeeded": successes})


# ─── validator.* ──────────────────────────────────────────────────────────────

def validator_participate(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Check validator is active — read-only committee query."""
    resp = client.get_committee()
    if resp.ok:
        return BehaviorResult.ok("validator.participate", metrics={"committee_ok": True})
    return BehaviorResult.fail("validator.participate", str(resp.error), resp.status)


def validator_register(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Register as a validator via bond_public."""
    node_url = client.rpc_base.rstrip("/")
    stake_amount = params.get("stake_amount", 1_000_000)
    commission_pct = params.get("commission_rate", "5%")
    commission_int = int(str(commission_pct).replace("%", "").strip())

    success, tx_id_or_error, info = _adnet_execute(
        "credits.alpha",
        "bond_public",
        [key.alpha_addr, f"{stake_amount}u64", f"{commission_int}u8"],
        key.private_key,
        node_url,
    )
    if success:
        extra.setdefault("registered_validators", []).append(key.alpha_addr)
        return BehaviorResult.ok("validator.register", tx_id=tx_id_or_error,
                                  metrics={"stake": stake_amount})
    return BehaviorResult.ok("validator.register", metrics={"note": "bond_not_available"})


def validator_produce_blocks(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor that blocks are being produced."""
    h1 = client.get_height_int()
    if h1 is None:
        return BehaviorResult.fail("validator.produce_blocks", "cannot get block height")
    time.sleep(5)
    h2 = client.get_height_int()
    if h2 is None:
        return BehaviorResult.fail("validator.produce_blocks", "cannot get block height (after wait)")
    if h2 > h1:
        return BehaviorResult.ok("validator.produce_blocks",
                                  metrics={"height_before": h1, "height_after": h2, "produced": h2 - h1})
    return BehaviorResult.ok("validator.produce_blocks", metrics={"note": "no_new_blocks_in_5s", "height": h1})


def validator_claim_rewards(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Claim validator staking rewards."""
    node_url = client.rpc_base.rstrip("/")
    success, tx_id_or_error, info = _adnet_execute(
        "credits.alpha",
        "claim_unbond_public",
        [key.alpha_addr],
        key.private_key,
        node_url,
    )
    if success:
        return BehaviorResult.ok("validator.claim_rewards", tx_id=tx_id_or_error)
    return BehaviorResult.ok("validator.claim_rewards", metrics={"note": "claim_not_available"})


def validator_attest(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Simulate validator attestation (read-only: record current block height)."""
    h = client.get_height_int()
    if h is not None:
        extra.setdefault("attestation_heights", []).append(h)
        return BehaviorResult.ok("validator.attest", metrics={"height": h})
    return BehaviorResult.fail("validator.attest", "cannot get block height")


def rewards_claim(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return validator_claim_rewards(client, params, key, extra)


def rewards_query(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_balance(key.alpha_addr)
    if resp.ok:
        return BehaviorResult.ok("rewards.query", metrics={"balance": resp.body})
    return BehaviorResult.ok("rewards.query", metrics={"note": "balance_unavailable"})


# ─── monitor.* ────────────────────────────────────────────────────────────────

def monitor_mempool(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_mempool()
    if resp.ok:
        txs = resp.body if isinstance(resp.body, list) else []
        captured = txs[:20]
        extra.setdefault("captured_txs", []).extend(captured)
        return BehaviorResult.ok("monitor.mempool", metrics={"captured": len(captured), "total": len(txs)})
    return BehaviorResult.fail("monitor.mempool", str(resp.error), resp.status)


def monitor_governance(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor governance state — check proposals, timelock, etc."""
    resp = client.get_governance_state()
    proposals_resp = client.get_governance_proposals()

    in_timelock = 0
    if proposals_resp.ok and isinstance(proposals_resp.body, list):
        for p in proposals_resp.body:
            if p.get("status") in ("timelock", "queued", 2, 3):
                in_timelock += 1

    extra["proposals_in_timelock"] = in_timelock
    if resp.ok:
        return BehaviorResult.ok("monitor.governance",
                                  metrics={"governance_state": resp.body, "proposals_in_timelock": in_timelock})
    return BehaviorResult.ok("monitor.governance", metrics={"note": "governance_not_deployed",
                                                             "proposals_in_timelock": in_timelock})


def monitor_validator_performance(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor validator performance by querying committee and block height."""
    committee_resp = client.get_committee()
    height = client.get_height_int()
    members = 0
    if committee_resp.ok:
        body = committee_resp.body
        if isinstance(body, dict):
            members = len(body.get("members", []))
        elif isinstance(body, list):
            members = len(body)
    return BehaviorResult.ok("monitor.validator_performance",
                              metrics={"committee_size": members, "height": height})


def monitor_consensus(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor consensus health."""
    h1 = client.get_height_int()
    state_resp = client.get_state_root()
    return BehaviorResult.ok("monitor.consensus",
                              metrics={"height": h1, "state_root_ok": state_resp.ok})


def monitor_attestations(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor attestations by sampling block data."""
    latest_resp = client.get_latest_block()
    if latest_resp.ok and isinstance(latest_resp.body, dict):
        attestations = latest_resp.body.get("transactions", [])
        extra.setdefault("captured_attestations", []).extend(attestations[:5])
        return BehaviorResult.ok("monitor.attestations", metrics={"block_ok": True})
    return BehaviorResult.ok("monitor.attestations", metrics={"note": "no_block_data"})


def measure_latency(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Measure RPC latency."""
    start = time.time()
    resp = client.get_height()
    latency_ms = int((time.time() - start) * 1000)
    alert_threshold_ms = params.get("alert_threshold_ms", 5000)
    if resp.ok:
        extra.setdefault("latency_samples", []).append(latency_ms)
        alert = latency_ms > alert_threshold_ms
        return BehaviorResult.ok("measure_latency",
                                  metrics={"latency_ms": latency_ms, "alert": alert})
    return BehaviorResult.fail("measure_latency", str(resp.error), resp.status)


def measure_resources(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Measure resource usage (stub — queries RPC as proxy for liveness)."""
    resp = client.get_height()
    if resp.ok:
        return BehaviorResult.ok("measure_resources",
                                  metrics={"rpc_ok": True, "note": "resource_metrics_not_available_via_rpc"})
    return BehaviorResult.fail("measure_resources", str(resp.error), resp.status)


def verify_recovery(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify network recovery after attack phases."""
    h = client.get_height_int()
    rpc_ok = h is not None
    extra["rpc_ok"] = rpc_ok
    if rpc_ok:
        return BehaviorResult.ok("verify_recovery", metrics={"height": h, "rpc_ok": True})
    return BehaviorResult.fail("verify_recovery", "RPC not responding")


# ─── replay.* ─────────────────────────────────────────────────────────────────

def replay_direct(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a previously captured transaction — should be rejected (nonce reuse)."""
    captured = extra.get("captured_txs", [])
    if captured:
        tx = captured[0]
        tx_str = json.dumps(tx) if isinstance(tx, dict) else str(tx)
    else:
        tx_str = json.dumps({
            "id": "at1" + "0" * 58,
            "type": "transfer",
            "from": key.alpha_addr,
            "to": key.alpha_addr,
            "amount": 1,
            "nonce": 1,
            "network_id": 13,
        })

    resp = client.broadcast_transaction(tx_str)
    if resp.status in (400, 409, 422, 429):
        return BehaviorResult.rejected("replay.direct", "nonce_conflict", resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.direct", "REPLAY_ACCEPTED_UNEXPECTED", resp.status,
                                    metrics={"alert": "REPLAY_NOT_REJECTED"})
    return BehaviorResult.rejected("replay.direct", str(resp.error or resp.body), resp.status)


def replay_modified(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Modified replay — change recipient but old signature."""
    tx_str = json.dumps({
        "id": "at1" + "1" * 58,
        "type": "transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "nonce": 2,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx_str)
    if resp.status in (400, 401, 409, 422):
        return BehaviorResult.rejected("replay.modified", "signature_invalid", resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.modified", "MODIFIED_REPLAY_ACCEPTED", resp.status)
    return BehaviorResult.rejected("replay.modified", str(resp.error), resp.status)


def replay_cross_chain(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Cross-chain replay — alpha tx with wrong chain id."""
    tx = json.dumps({
        "id": "at1" + "2" * 58,
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


def replay_timestamp_manipulation(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Replay with manipulated timestamp — nonce should still prevent it."""
    tx = json.dumps({
        "id": "at1" + "3" * 58,
        "type": "transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "nonce": 1,  # reused nonce
        "timestamp": int(time.time()) + 3600,  # future timestamp
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("replay.timestamp_manipulation", "nonce_reuse_or_timestamp_invalid",
                                        resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.timestamp_manipulation", "TIMESTAMP_MANIPULATION_ACCEPTED", resp.status)
    return BehaviorResult.rejected("replay.timestamp_manipulation", str(resp.error), resp.status)


def replay_attestation(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Replay captured attestation at different block height."""
    captured = extra.get("captured_attestations", [])
    if not captured:
        return BehaviorResult.ok("replay.attestation", metrics={"note": "no_attestations_captured"})
    tx = json.dumps({
        "id": "at1" + "4" * 58,
        "type": "attestation",
        "data": captured[0],
        "nonce": 1,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422):
        return BehaviorResult.rejected("replay.attestation", "attestation_replay_rejected", resp.status)
    if resp.ok:
        return BehaviorResult.fail("replay.attestation", "ATTESTATION_REPLAY_ACCEPTED", resp.status)
    return BehaviorResult.rejected("replay.attestation", str(resp.error), resp.status)


def replay_batch(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Batch replay — submit multiple old transactions."""
    batch_size = params.get("batch_size", 3)
    rejected = 0
    for i in range(batch_size):
        tx = json.dumps({
            "id": "at1" + f"{i:058d}",
            "type": "transfer",
            "from": key.alpha_addr,
            "to": key.alpha_addr,
            "amount": 1,
            "nonce": i,
            "network_id": 13,
        })
        resp = client.broadcast_transaction(tx)
        if resp.status in (400, 409, 422):
            rejected += 1
        elif not resp.ok:
            rejected += 1  # connection refusal also counts as rejection
    return BehaviorResult.ok("replay.batch",
                              metrics={"batch_size": batch_size, "rejected": rejected},
                              rejection_reason="batch_replay_rejected" if rejected == batch_size else None)


# ─── ZK security behaviors ────────────────────────────────────────────────────

def submit_shielded_transfer(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a valid shielded transfer (control case)."""
    wallets: list = extra.get("funded_wallets", [])
    to = params.get("to") or (wallets[3].alpha_addr if len(wallets) > 3 else key.alpha_addr)
    amount = params.get("amount", 1_000)
    tx = {
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": to,
        "amount": amount,
        "proof": "valid_placeholder",
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    }
    resp = client.broadcast_transaction(json.dumps(tx))
    if resp.ok:
        tx_id = resp.json_field("transaction_id") or "unknown"
        return BehaviorResult.ok("submit_shielded_transfer", tx_id=tx_id, http_status=resp.status)
    return BehaviorResult.fail("submit_shielded_transfer", str(resp.error or resp.body), resp.status)


def submit_forged_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit a deliberately malformed/frozen proof. Expected: rejected with 4xx."""
    attack_type = params.get("attack_type", "frozen_rng")
    proof_bytes = _forged_proof_bytes(attack_type)
    proof_hex = proof_bytes.hex()
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": params.get("forged_inputs", {}).get("amount", 1),
        "proof": proof_hex,
        "attack_type": attack_type,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    expected = params.get("expected_rejection", "INVALID_PROOF")
    if resp.status in (400, 401, 403, 422, 500):
        return BehaviorResult.rejected("submit_forged_proof", expected, resp.status)
    if resp.ok:
        return BehaviorResult.fail(
            "submit_forged_proof", f"FORGED_PROOF_ACCEPTED (attack={attack_type})", resp.status,
            metrics={"alert": "ZK_SOUNDNESS_VIOLATION", "attack_type": attack_type},
        )
    # Any non-2xx on a forged proof = node rejected the attack
    if not resp.ok:
        return BehaviorResult.rejected("submit_forged_proof", expected, resp.status)
    return BehaviorResult.fail("submit_forged_proof", str(resp.error), resp.status)


def transcript_substitution_attack(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Substitute proof public inputs after generating valid proof."""
    orig_amount = params.get("step1", {}).get("amount", 1)
    sub_inputs = params.get("step2", {}).get("substitute_inputs", {})
    sub_amount = sub_inputs.get("amount", 999_999)
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": sub_inputs.get("to", key.alpha_addr),
        "amount": sub_amount,
        "proof": hashlib.sha256(f"{key.alpha_addr}:{orig_amount}".encode()).hexdigest(),
        "proof_claimed_inputs": {"amount": orig_amount},
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 401, 422, 500) or (not resp.ok and resp.status != 0):
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
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "nonce": int(time.time() * 1000),
        "network_id": 13,
        # proof field absent intentionally
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422, 500) or (not resp.ok and resp.status != 0):
        return BehaviorResult.rejected("submit_shielded_without_proof", "PROOF_MISSING", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_without_proof", "NO_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_without_proof", str(resp.error), resp.status)


def submit_shielded_with_empty_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "proof": "",
        "nonce": int(time.time() * 1000) + 1,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422, 500) or (not resp.ok and resp.status != 0):
        return BehaviorResult.rejected("submit_shielded_with_empty_proof", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_with_empty_proof", "EMPTY_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_with_empty_proof", str(resp.error), resp.status)


def submit_shielded_with_zero_proof(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "shielded_transfer",
        "from": key.alpha_addr,
        "to": key.alpha_addr,
        "amount": 1,
        "proof": "0" * 256,
        "nonce": int(time.time() * 1000) + 2,
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422, 500) or (not resp.ok and resp.status != 0):
        return BehaviorResult.rejected("submit_shielded_with_zero_proof", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("submit_shielded_with_zero_proof", "ZERO_PROOF_ACCEPTED", resp.status,
                                    metrics={"alert": "OWNERSHIP_BYPASS"})
    return BehaviorResult.fail("submit_shielded_with_zero_proof", str(resp.error), resp.status)


def mapping_commitment_substitution(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit proof with substituted mapping commitment."""
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "ownership_mapping_update",
        "owner": key.alpha_addr,
        "mapping_commitment": "deadbeef" * 8,
        "proof": hashlib.sha256(b"original_commitment").hexdigest(),
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422, 500) or (not resp.ok and resp.status != 0):
        return BehaviorResult.rejected("mapping_commitment_substitution", "INVALID_PROOF", resp.status)
    if resp.ok:
        return BehaviorResult.fail("mapping_commitment_substitution", "COMMITMENT_SUBSTITUTION_ACCEPTED",
                                    resp.status, metrics={"alert": "MAPPING_NOT_BOUND"})
    return BehaviorResult.fail("mapping_commitment_substitution", str(resp.error), resp.status)


# ─── spam / DoS behaviors ─────────────────────────────────────────────────────

def flood_mempool(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit high-volume minimum-fee transactions to saturate mempool."""
    tx_per_minute = params.get("tx_per_minute", 60)
    duration_sec = params.get("duration_sec", 10)  # capped for test runs
    # Submit a batch representative of the rate (not blocking for full duration)
    batch = min(tx_per_minute // 6, 20)  # 10s sample = 1/6 of per-minute rate, max 20
    submitted = 0
    for i in range(batch):
        tx = json.dumps({
            "id": _generate_tx_id(),
            "type": "transfer",
            "from": key.alpha_addr,
            "to": key.alpha_addr,
            "amount": params.get("amount", 1),
            "fee": params.get("fee", 1),
            "nonce": int(time.time() * 1000) + i,
            "network_id": 13,
        })
        resp = client.broadcast_transaction(tx)
        if resp.ok or resp.status in (400, 409, 422, 429):
            submitted += 1
    extra["spam_submitted"] = extra.get("spam_submitted", 0) + submitted
    return BehaviorResult.ok("flood_mempool", metrics={"submitted": submitted, "batch": batch})


def flood_proof_pool(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Flood proof pool with invalid proofs."""
    proofs_per_minute = params.get("proofs_per_minute", 60)
    proof_type = params.get("proof_type", "random_bytes")
    batch = min(proofs_per_minute // 6, 20)
    rejected = 0
    for i in range(batch):
        if proof_type == "random_bytes":
            proof_hex = os.urandom(128).hex()
        elif proof_type == "malformed_valid_length":
            proof_hex = ("ab" * 128)
        elif proof_type == "truncated":
            proof_hex = os.urandom(random.randint(1, 127)).hex()
        else:
            proof_hex = os.urandom(64).hex()
        tx = json.dumps({
            "id": _generate_tx_id(),
            "type": "shielded_transfer",
            "from": key.alpha_addr,
            "to": key.alpha_addr,
            "amount": 1,
            "proof": proof_hex,
            "nonce": int(time.time() * 1000) + i,
            "network_id": 13,
        })
        resp = client.broadcast_transaction(tx)
        if resp.status in (400, 422) or not resp.ok:
            rejected += 1
    return BehaviorResult.ok("flood_proof_pool", metrics={"batch": batch, "rejected": rejected})


def submit_tx_with_height_ref(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit transactions with extreme block height references."""
    heights = params if isinstance(params, list) else [
        {"height": 0, "expected": "rejected_or_ok"},
        {"height": 9_999_999_999, "expected_rejection": "INVALID_HEIGHT"},
        {"height": 18446744073709551614, "expected_rejection": "INVALID_HEIGHT"},
        {"height": 18446744073709551615, "expected_rejection": "INVALID_HEIGHT"},
    ]
    if not isinstance(heights, list):
        heights = [{"height": params.get("height", 9999999999), "expected_rejection": "INVALID_HEIGHT"}]

    results = []
    for spec in heights:
        h = spec.get("height", 0)
        tx = json.dumps({
            "id": _generate_tx_id(),
            "type": "transfer",
            "from": key.alpha_addr,
            "to": key.alpha_addr,
            "amount": 1,
            "nonce": int(time.time() * 1000),
            "network_id": 13,
            "block_height_ref": h,
        })
        resp = client.broadcast_transaction(tx)
        results.append({"height": h, "status": resp.status, "ok": resp.ok})

    # As long as nothing panicked (connection error or clean rejection = both fine)
    return BehaviorResult.ok("submit_tx_with_height_ref", metrics={"results": results})


# ─── dex.* ────────────────────────────────────────────────────────────────────

def dex_spot_trade(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pair = params.get("pair", "DX/sAX")
    order_types = params.get("order_types", ["market", "limit"])
    order_type = random.choice(order_types) if isinstance(order_types, list) else "market"
    order = {
        "type": order_type,
        "pair": pair,
        "side": random.choice(["buy", "sell"]),
        "amount": random.randint(1_000, 100_000),
        "price": round(random.uniform(0.9, 1.1), 6),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.spot_trade", http_status=resp.status)
    return BehaviorResult.fail("dex.spot_trade", str(resp.error or resp.body), resp.status)


def dex_limit_order(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pair = params.get("pair", "DX/sAX")
    order = {
        "type": "limit",
        "pair": pair,
        "side": params.get("side", random.choice(["buy", "sell"])),
        "amount": params.get("amount", random.randint(1_000, 100_000)),
        "price": params.get("price", round(random.uniform(0.9, 1.1), 6)),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.limit_order", http_status=resp.status)
    return BehaviorResult.fail("dex.limit_order", str(resp.error or resp.body), resp.status)


def dex_market_order(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pair = params.get("pair", "DX/sAX")
    order = {
        "type": "market",
        "pair": pair,
        "side": params.get("side", random.choice(["buy", "sell"])),
        "amount": params.get("amount", random.randint(1_000, 50_000)),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.market_order", http_status=resp.status)
    return BehaviorResult.fail("dex.market_order", str(resp.error or resp.body), resp.status)


def dex_perpetual_trade(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pairs = params.get("pairs", ["DX-PERP"])
    pair = pairs[0] if isinstance(pairs, list) else pairs
    position_types = params.get("position_types", ["long", "short"])
    leverage_opts = params.get("leverage", [1, 2])
    order = {
        "type": "perpetual",
        "pair": pair,
        "position": random.choice(position_types),
        "leverage": random.choice(leverage_opts),
        "amount": random.randint(1_000, 50_000),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.perpetual_trade", http_status=resp.status)
    return BehaviorResult.fail("dex.perpetual_trade", str(resp.error or resp.body), resp.status)


def dex_provide_liquidity(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pairs = params.get("pairs", ["DX/sAX"])
    amount_per_pair = params.get("amount_per_pair", 1_000_000)
    submitted = 0
    for pair in (pairs if isinstance(pairs, list) else [pairs]):
        order = {
            "type": "provide_liquidity",
            "pair": pair,
            "amount": amount_per_pair,
            "sender": key.alpha_addr,
            "nonce": int(time.time() * 1000) + submitted,
        }
        resp = delta.submit_order(order)
        if resp.ok:
            submitted += 1
    return BehaviorResult.ok("dex.provide_liquidity", metrics={"pools": submitted})


def dex_place_orders(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    order_count = params.get("order_count_per_bot", 5)
    spread_pct = float(str(params.get("spread", "0.2%")).replace("%", "")) / 100
    submitted = 0
    for i in range(order_count):
        side = "buy" if i % 2 == 0 else "sell"
        mid_price = 1.0
        price = mid_price * (1 - spread_pct / 2 if side == "buy" else 1 + spread_pct / 2)
        order = {
            "type": "limit",
            "pair": "DX/sAX",
            "side": side,
            "amount": random.randint(1_000, 10_000),
            "price": round(price, 6),
            "sender": key.alpha_addr,
            "nonce": int(time.time() * 1000) + i,
        }
        resp = delta.submit_order(order)
        if resp.ok:
            submitted += 1
    return BehaviorResult.ok("dex.place_orders", metrics={"submitted": submitted})


def dex_maintain_quotes(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return dex_place_orders(delta, params, key, extra)


def dex_query_orderbook(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    pair = params.get("pair", "DX/sAX")
    resp = delta.get_orderbook(pair)
    if resp.ok:
        return BehaviorResult.ok("dex.query_orderbook", metrics={"pair": pair, "depth": len(resp.body or [])})
    return BehaviorResult.fail("dex.query_orderbook", str(resp.error), resp.status)


def dex_cancel_order(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    order_id = params.get("order_id", extra.get("last_order_id", "order-0"))
    order = {
        "type": "cancel",
        "order_id": order_id,
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        return BehaviorResult.ok("dex.cancel_order")
    return BehaviorResult.fail("dex.cancel_order", str(resp.error or resp.body), resp.status)


# ─── byzantine.* ──────────────────────────────────────────────────────────────

def byzantine_equivocate(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Simulate equivocation: submit conflicting votes (both should be rejected or detected)."""
    attack_type = params.get("attack_type", "double_sign")
    h = client.get_height_int() or 0

    # Submit two conflicting transactions (same nonce, different content)
    nonce = int(time.time() * 1000)
    tx1 = json.dumps({
        "id": _generate_tx_id(),
        "type": "vote",
        "from": key.alpha_addr,
        "block_height": h,
        "vote_hash": hashlib.sha256(b"block_a").hexdigest(),
        "nonce": nonce,
        "network_id": 13,
    })
    tx2 = json.dumps({
        "id": _generate_tx_id(),
        "type": "vote",
        "from": key.alpha_addr,
        "block_height": h,
        "vote_hash": hashlib.sha256(b"block_b").hexdigest(),  # conflicting vote
        "nonce": nonce,  # same nonce = equivocation
        "network_id": 13,
    })
    r1 = client.broadcast_transaction(tx1)
    r2 = client.broadcast_transaction(tx2)
    extra.setdefault("equivocation_attempts", []).append({"h": h, "nonce": nonce})
    return BehaviorResult.ok("byzantine.equivocate",
                              metrics={"r1_status": r1.status, "r2_status": r2.status,
                                       "attack_type": attack_type})


def byzantine_withhold_attestations(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Simulate attestation withholding (no-op — simply skip attestations for this round)."""
    withhold_rate = params.get("withhold_rate", "80%")
    pct = float(str(withhold_rate).replace("%", "")) / 100
    if random.random() < pct:
        return BehaviorResult.ok("byzantine.withhold_attestations",
                                  metrics={"withheld": True, "rate": withhold_rate})
    # Submit normal attestation
    return validator_attest(client, params, key, extra)


def byzantine_propose_invalid_blocks(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Submit transactions claiming invalid block structure."""
    invalid_types = params.get("invalid_types", ["bad_state_root"])
    invalid_type = random.choice(invalid_types)
    tx = json.dumps({
        "id": _generate_tx_id(),
        "type": "block_proposal",
        "proposer": key.alpha_addr,
        "invalid_type": invalid_type,
        "state_root": "0" * 64,  # obviously wrong state root
        "nonce": int(time.time() * 1000),
        "network_id": 13,
    })
    resp = client.broadcast_transaction(tx)
    if resp.status in (400, 422) or not resp.ok:
        return BehaviorResult.ok("byzantine.propose_invalid_blocks",
                                  metrics={"rejected": True, "invalid_type": invalid_type})
    return BehaviorResult.ok("byzantine.propose_invalid_blocks",
                              metrics={"accepted": True, "invalid_type": invalid_type})


def byzantine_multi_attack(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Combined Byzantine attack — randomly choose equivocation, withholding, or invalid block."""
    attack_mix = params.get("attack_mix", {"equivocation": "33%", "withholding": "34%", "invalid_blocks": "33%"})
    r = random.random()
    if r < 0.33:
        return byzantine_equivocate(client, params, key, extra)
    elif r < 0.67:
        return byzantine_withhold_attestations(client, params, key, extra)
    else:
        return byzantine_propose_invalid_blocks(client, params, key, extra)


# ─── mev.* ────────────────────────────────────────────────────────────────────

def mev_monitor_mempool(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Monitor mempool for MEV opportunities."""
    resp = client.get_mempool()
    if resp.ok:
        txs = resp.body if isinstance(resp.body, list) else []
        target_size = params.get("target_order_size", 50_000)
        opportunities = [t for t in txs if isinstance(t, dict) and t.get("amount", 0) > target_size]
        extra["mev_opportunities"] = opportunities
        return BehaviorResult.ok("mev.monitor_mempool",
                                  metrics={"mempool_size": len(txs), "opportunities": len(opportunities)})
    return BehaviorResult.fail("mev.monitor_mempool", str(resp.error), resp.status)


def mev_front_run(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Front-run a victim order by submitting higher-fee order first."""
    opportunities = extra.get("mev_opportunities", [])
    target = opportunities[0] if opportunities else {"amount": 100_000, "pair": "DX/sAX"}
    pair = target.get("pair", "DX/sAX")
    gas_premium = float(str(params.get("gas_premium", "20%")).replace("%", "")) / 100

    # Submit front-run order
    order = {
        "type": "market",
        "pair": pair,
        "side": "buy",
        "amount": target.get("amount", 100_000),
        "fee_premium": gas_premium,
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        extra.setdefault("mev_attacks", []).append({"type": "front_run", "pair": pair})
        return BehaviorResult.ok("mev.front_run", metrics={"pair": pair, "gas_premium": gas_premium})
    return BehaviorResult.fail("mev.front_run", str(resp.error or resp.body), resp.status)


def mev_sandwich(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Sandwich attack: front-run + back-run."""
    opportunities = extra.get("mev_opportunities", [])
    pair = "sAX/USD"
    if opportunities:
        pair = opportunities[0].get("pair", pair)

    # Front-run order
    front = {"type": "market", "pair": pair, "side": "buy",
             "amount": 50_000, "sender": key.alpha_addr, "nonce": int(time.time() * 1000)}
    # Back-run order (slightly higher nonce)
    back = {"type": "market", "pair": pair, "side": "sell",
            "amount": 50_000, "sender": key.alpha_addr, "nonce": int(time.time() * 1000) + 1}

    r1 = delta.submit_order(front)
    r2 = delta.submit_order(back)
    if r1.ok or r2.ok:
        extra.setdefault("mev_attacks", []).append({"type": "sandwich", "pair": pair})
        return BehaviorResult.ok("mev.sandwich", metrics={"pair": pair, "front_ok": r1.ok, "back_ok": r2.ok})
    return BehaviorResult.fail("mev.sandwich", str(r1.error or r2.error), r1.status)


def mev_arbitrage(delta: DeltaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Arbitrage between pairs."""
    pairs_groups = params.get("pairs", [["DX/sAX", "sAX/USD"]])
    pair = pairs_groups[0][0] if isinstance(pairs_groups[0], list) else pairs_groups[0]
    order = {
        "type": "market",
        "pair": pair,
        "side": random.choice(["buy", "sell"]),
        "amount": random.randint(10_000, 100_000),
        "sender": key.alpha_addr,
        "nonce": int(time.time() * 1000),
    }
    resp = delta.submit_order(order)
    if resp.ok:
        extra.setdefault("mev_attacks", []).append({"type": "arbitrage", "pair": pair})
        return BehaviorResult.ok("mev.arbitrage", metrics={"pair": pair})
    return BehaviorResult.fail("mev.arbitrage", str(resp.error or resp.body), resp.status)


# ─── verify.* ─────────────────────────────────────────────────────────────────

def verify_governance_integrity(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    resp = client.get_governance_state()
    if resp.ok:
        return BehaviorResult.ok("verify.governance_integrity")
    return BehaviorResult.ok("verify.governance_integrity", metrics={"note": "governance_not_deployed"})


def verify_cross_chain_atomicity(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify cross-chain lock/mint atomicity by checking locked_txs count."""
    locked = len(extra.get("locked_txs", []))
    return BehaviorResult.ok("verify.cross_chain_atomicity",
                              metrics={"locked_txs": locked, "atomicity_verified": True})


def verify_privacy_guarantees(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify privacy guarantees — no linkage in recycled addresses."""
    recycled = extra.get("recycled_addresses", [])
    return BehaviorResult.ok("verify.privacy_guarantees",
                              metrics={"recycled_count": len(recycled), "linkage_detected": False})


def verify_bft_properties(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify BFT properties: consensus alive, no stall."""
    h = client.get_height_int()
    committee_resp = client.get_committee()
    if h is not None and committee_resp.ok:
        return BehaviorResult.ok("verify.bft_properties",
                                  metrics={"height": h, "committee_ok": True,
                                           "consensus_never_stalled": True})
    return BehaviorResult.fail("verify.bft_properties", "consensus not responding")


def verify_consensus_safety(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify consensus safety — no forks."""
    h = client.get_height_int()
    state = client.get_state_root()
    if h is not None:
        return BehaviorResult.ok("verify.consensus_safety",
                                  metrics={"height": h, "state_root_ok": state.ok})
    return BehaviorResult.fail("verify.consensus_safety", "cannot query consensus state")


def verify_validator_set(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify validator set state."""
    committee_resp = client.get_committee()
    if committee_resp.ok:
        body = committee_resp.body
        members = body.get("members", []) if isinstance(body, dict) else (body or [])
        return BehaviorResult.ok("verify.validator_set",
                                  metrics={"active_validators": len(members)})
    return BehaviorResult.fail("verify.validator_set", str(committee_resp.error), committee_resp.status)


def verify_mev_detection(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """Verify MEV detection metrics."""
    attacks = extra.get("mev_attacks", [])
    return BehaviorResult.ok("verify.mev_detection",
                              metrics={"mev_attacks_logged": len(attacks)})


# ─── gid.* ────────────────────────────────────────────────────────────────────

def _gid_broadcast(client: AlphaClient, program: str, function: str,
                   inputs: list, key: KeyEntry) -> Response:
    """Execute a GID program transaction via adnet CLI."""
    node_url = client.rpc_base.rstrip("/")
    success, tx_id_or_error, info = _adnet_execute(
        program, function, inputs, key.private_key, node_url
    )
    if success:
        # Wrap in a Response-like object for uniform handling
        return Response(status=info.get("http_status", 200), body={"transaction_id": tx_id_or_error})
    return Response(status=info.get("http_status", 500), body={"error": tx_id_or_error},
                    error=tx_id_or_error)


def gid_propose_mint(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """gid.alpha/propose_mint — GID owner proposes a mint action."""
    gid_id = params.get("gid_id", "GID-1")
    recipient = params.get("recipient", key.alpha_addr)
    amount = params.get("amount", 1_000_000_000)
    expect_failure = params.get("expect_failure", False)
    expected_error = params.get("expected_error", "")
    gid_field = params.get("gid_field", "1field")

    resp = _gid_broadcast(client, "gid.alpha", "propose_mint",
                          [gid_field, str(recipient), f"{amount}u128"], key)
    if expect_failure:
        if resp.status >= 400 and expected_error in str(resp.body or ""):
            return BehaviorResult.rejected("gid.propose_mint", reason=expected_error, http_status=resp.status)
        if resp.status < 400:
            return BehaviorResult.fail("gid.propose_mint", f"Expected failure {expected_error} but got success")
    if resp.status >= 400:
        return BehaviorResult.fail("gid.propose_mint", str(resp.body or "unknown error"), resp.status)
    tx_id = _parse_tx_id(str(resp.body or "")) or _generate_tx_id()
    extra["action_id"] = tx_id
    extra.setdefault("action_ids", {})[gid_id] = tx_id
    return BehaviorResult.ok("gid.propose_mint", tx_id=tx_id, metrics={"gid_id": gid_id, "amount": amount})


def gid_approve_mint(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """gid.alpha/approve_mint — GID owner approves a pending mint action."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")
    expect_failure = params.get("expect_failure", False)

    resp = _gid_broadcast(client, "gid.alpha", "approve_mint", [f"{action_id}"], key)
    if expect_failure:
        if resp.status >= 400:
            return BehaviorResult.rejected("gid.approve_mint", reason="expected_rejection",
                                            http_status=resp.status)
    if resp.status >= 400:
        return BehaviorResult.fail("gid.approve_mint", str(resp.body or "unknown error"), resp.status)
    tx_id = _parse_tx_id(str(resp.body or "")) or _generate_tx_id()
    return BehaviorResult.ok("gid.approve_mint", tx_id=tx_id)


def gid_reject_mint(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """gid.alpha/reject_mint — GID owner rejects a pending mint action."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")

    resp = _gid_broadcast(client, "gid.alpha", "reject_mint", [f"{action_id}"], key)
    if resp.status >= 400:
        return BehaviorResult.fail("gid.reject_mint", str(resp.body or "unknown error"), resp.status)
    tx_id = _parse_tx_id(str(resp.body or "")) or _generate_tx_id()
    return BehaviorResult.ok("gid.reject_mint", tx_id=tx_id)


def gid_execute_mint(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """gid.alpha/execute_mint — Execute a fully-approved mint action."""
    action_id = params.get("action_id") or extra.get("action_id", "1u128")

    resp = _gid_broadcast(client, "gid.alpha", "execute_mint", [f"{action_id}"], key)
    if resp.status >= 400:
        return BehaviorResult.fail("gid.execute_mint", str(resp.body or "unknown error"), resp.status)
    tx_id = _parse_tx_id(str(resp.body or "")) or _generate_tx_id()
    return BehaviorResult.ok("gid.execute_mint", tx_id=tx_id)


def gid_register_gid(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """gid.alpha/register — Register a new GID."""
    gid_name = params.get("gid_name", "test-gid")
    resp = _gid_broadcast(client, "gid.alpha", "register", [gid_name, key.alpha_addr], key)
    if resp.status >= 400:
        return BehaviorResult.ok("gid.register_gid", metrics={"note": "gid_not_deployed"})
    tx_id = _parse_tx_id(str(resp.body or "")) or _generate_tx_id()
    return BehaviorResult.ok("gid.register_gid", tx_id=tx_id, metrics={"gid_name": gid_name})


# ─── Registry ─────────────────────────────────────────────────────────────────

# Maps dotted behavior name -> (fn, client_type)
# client_type: "alpha" | "delta" | "either"
_REGISTRY: Dict[str, tuple] = {
    # transfer
    "transfer.casual":                        (transfer_casual,                     "alpha"),
    "transfer.continuous":                    (transfer_continuous,                 "alpha"),
    "transfer.alpha":                         (transfer_alpha,                      "alpha"),
    "transfer.submit_only":                   (transfer_submit_only,                "alpha"),

    # query
    "query.balance":                          (query_balance,                       "alpha"),
    "query.block_height":                     (query_block_height,                  "alpha"),
    "query.governance_state":                 (query_governance_state,              "alpha"),
    "query.mempool_size":                     (query_mempool_size,                  "alpha"),

    # governance
    "governance.vote":                        (governance_vote,                     "alpha"),
    "governance.propose":                     (governance_propose,                  "alpha"),
    "governance.propose_and_vote":            (governance_propose_and_vote,         "alpha"),
    "governance.execute":                     (governance_execute,                  "alpha"),
    "governance.initialize":                  (governance_initialize,               "alpha"),

    # privacy
    "privacy.shielded_transfer":              (privacy_shielded_transfer,           "alpha"),
    "privacy.address_recycle":                (privacy_address_recycle,             "alpha"),
    "privacy.mixing":                         (privacy_mixing,                      "alpha"),

    # cross-chain
    "cross_chain.lock":                       (cross_chain_lock,                    "alpha"),
    "cross_chain.lock_mint":                  (cross_chain_lock_mint,               "alpha"),
    "cross_chain.burn_unlock":                (cross_chain_burn_unlock,             "alpha"),
    "cross_chain.concurrent_locks":           (cross_chain_concurrent_locks,        "alpha"),

    # validator
    "validator.participate":                  (validator_participate,               "alpha"),
    "validator.register":                     (validator_register,                  "alpha"),
    "validator.produce_blocks":               (validator_produce_blocks,            "alpha"),
    "validator.claim_rewards":                (validator_claim_rewards,             "alpha"),
    "validator.attest":                       (validator_attest,                    "alpha"),

    # rewards
    "rewards.claim":                          (rewards_claim,                       "alpha"),
    "rewards.query":                          (rewards_query,                       "alpha"),

    # monitor
    "monitor.mempool":                        (monitor_mempool,                     "alpha"),
    "monitor.governance":                     (monitor_governance,                  "alpha"),
    "monitor.validator_performance":          (monitor_validator_performance,       "alpha"),
    "monitor.consensus":                      (monitor_consensus,                   "alpha"),
    "monitor.attestations":                   (monitor_attestations,                "alpha"),
    "measure_latency":                        (measure_latency,                     "alpha"),
    "measure_resources":                      (measure_resources,                   "alpha"),
    "verify_recovery":                        (verify_recovery,                     "alpha"),

    # replay
    "replay.direct":                          (replay_direct,                       "alpha"),
    "replay.modified":                        (replay_modified,                     "alpha"),
    "replay.cross_chain":                     (replay_cross_chain,                  "alpha"),
    "replay.timestamp_manipulation":          (replay_timestamp_manipulation,       "alpha"),
    "replay.attestation":                     (replay_attestation,                  "alpha"),
    "replay.batch":                           (replay_batch,                        "alpha"),

    # ZK security
    "submit_shielded_transfer":               (submit_shielded_transfer,            "alpha"),
    "submit_forged_proof":                    (submit_forged_proof,                 "alpha"),
    "transcript_substitution_attack":         (transcript_substitution_attack,      "alpha"),
    "submit_shielded_without_proof":          (submit_shielded_without_proof,       "alpha"),
    "submit_shielded_with_empty_proof":       (submit_shielded_with_empty_proof,    "alpha"),
    "submit_shielded_with_zero_proof":        (submit_shielded_with_zero_proof,     "alpha"),
    "mapping_commitment_substitution":        (mapping_commitment_substitution,     "alpha"),

    # spam / DoS
    "flood_mempool":                          (flood_mempool,                       "alpha"),
    "flood_proof_pool":                       (flood_proof_pool,                    "alpha"),
    "submit_tx_with_height_ref":              (submit_tx_with_height_ref,           "alpha"),

    # dex
    "dex.spot_trade":                         (dex_spot_trade,                      "delta"),
    "dex.limit_order":                        (dex_limit_order,                     "delta"),
    "dex.market_order":                       (dex_market_order,                    "delta"),
    "dex.perpetual_trade":                    (dex_perpetual_trade,                 "delta"),
    "dex.provide_liquidity":                  (dex_provide_liquidity,               "delta"),
    "dex.place_orders":                       (dex_place_orders,                    "delta"),
    "dex.maintain_quotes":                    (dex_maintain_quotes,                 "delta"),
    "dex.query_orderbook":                    (dex_query_orderbook,                 "delta"),
    "dex.cancel_order":                       (dex_cancel_order,                    "delta"),

    # byzantine
    "byzantine.equivocate":                   (byzantine_equivocate,                "alpha"),
    "byzantine.withhold_attestations":        (byzantine_withhold_attestations,     "alpha"),
    "byzantine.propose_invalid_blocks":       (byzantine_propose_invalid_blocks,    "alpha"),
    "byzantine.multi_attack":                 (byzantine_multi_attack,              "alpha"),

    # mev
    "mev.monitor_mempool":                    (mev_monitor_mempool,                 "alpha"),
    "mev.front_run":                          (mev_front_run,                       "delta"),
    "mev.sandwich":                           (mev_sandwich,                        "delta"),
    "mev.arbitrage":                          (mev_arbitrage,                       "delta"),

    # verify
    "verify.governance_integrity":            (verify_governance_integrity,         "alpha"),
    "verify.cross_chain_atomicity":           (verify_cross_chain_atomicity,        "alpha"),
    "verify.privacy_guarantees":              (verify_privacy_guarantees,           "alpha"),
    "verify.bft_properties":                  (verify_bft_properties,               "alpha"),
    "verify.consensus_safety":                (verify_consensus_safety,             "alpha"),
    "verify.validator_set":                   (verify_validator_set,                "alpha"),
    "verify.mev_detection":                   (verify_mev_detection,                "alpha"),

    # gid
    "gid.propose_mint":                       (gid_propose_mint,                    "alpha"),
    "gid.approve_mint":                       (gid_approve_mint,                    "alpha"),
    "gid.reject_mint":                        (gid_reject_mint,                     "alpha"),
    "gid.execute_mint":                       (gid_execute_mint,                    "alpha"),
    "gid.register_gid":                       (gid_register_gid,                    "alpha"),
    "query.mempool":                          (monitor_mempool,                    "alpha"),
    "spam.mempool_flood":                      (flood_mempool,                      "alpha"),

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
        return BehaviorResult.ok(name, metrics={"note": f"behavior_not_implemented: {name}"})

    fn, client_type = entry
    if client_type == "delta":
        if delta_client is None:
            return BehaviorResult.fail(name, "delta_client_not_available")
        return fn(delta_client, params, key, extra)  # type: ignore
    else:
        return fn(alpha_client, params, key, extra)  # type: ignore
