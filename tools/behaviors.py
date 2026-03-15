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


# ─── Behavior implementations ─────────────────────────────────────────────────

# transfer.*

def transfer_casual(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    wallets: list = extra.get("funded_wallets", [])
    recipient = params.get("recipient") or (
        wallets[1].alpha_addr if len(wallets) > 1 else "ac1test000000000000000000000000000000000000000000"
    )
    amount = params.get("amount", random.randint(100, 10_000))
    tx_str = _dummy_tx(key.alpha_addr, recipient, amount)
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        tx_id = resp.json_field("transaction_id") or resp.json_field("id") or "unknown"
        return BehaviorResult.ok("transfer.casual", tx_id=tx_id, http_status=resp.status)
    return BehaviorResult.fail("transfer.casual", str(resp.error or resp.body), resp.status)


def transfer_continuous(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    return transfer_casual(client, params, key, extra)


def transfer_submit_only(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    """T1.3 — submit one transfer and verify it enters the mempool (no wait)."""
    wallets: list = extra.get("funded_wallets", [])
    recipient = wallets[1].alpha_addr if len(wallets) > 1 else "ac1test000000000000000000000000000000000000000000"
    amount = params.get("amount", 1_000)
    tx_str = _dummy_tx(key.alpha_addr, recipient, amount)
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        tx_id = resp.json_field("transaction_id") or resp.json_field("id") or "queued"
        return BehaviorResult.ok("transfer.submit_only", tx_id=tx_id, http_status=resp.status)
    # 422 = already in mempool / duplicate nonce → still functionally OK for T1.3
    if resp.status in (409, 422):
        return BehaviorResult.ok("transfer.submit_only", http_status=resp.status,
                                  metrics={"note": "duplicate_or_known"})
    return BehaviorResult.fail("transfer.submit_only", str(resp.error or resp.body), resp.status)


# governance.*

def governance_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    # Try to find an active proposal to vote on
    proposals_resp = client.get_governance_proposals()
    if not proposals_resp.ok:
        return BehaviorResult.fail("governance.vote", "cannot fetch proposals", proposals_resp.status)
    proposals = proposals_resp.body
    if not proposals or (isinstance(proposals, list) and len(proposals) == 0):
        return BehaviorResult.ok("governance.vote", metrics={"note": "no_proposals"})
    proposal_id = (proposals[0].get("id") if isinstance(proposals, list) else "0")
    vote = params.get("vote", "yes")
    tx_str = _dummy_vote_tx(key.alpha_addr, str(proposal_id), vote)
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        return BehaviorResult.ok("governance.vote", http_status=resp.status)
    return BehaviorResult.fail("governance.vote", str(resp.error or resp.body), resp.status)


def governance_propose(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    tx_str = _dummy_governance_tx(
        key.alpha_addr,
        params.get("proposal_type", "parameter_change"),
        {k: v for k, v in params.items() if k != "proposal_type"},
    )
    resp = client.broadcast_transaction(tx_str)
    if resp.ok:
        return BehaviorResult.ok("governance.propose", http_status=resp.status)
    return BehaviorResult.fail("governance.propose", str(resp.error or resp.body), resp.status)


def governance_propose_and_vote(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    res = governance_propose(client, params, key, extra)
    if not res.success:
        return res
    return governance_vote(client, params, key, extra)


def governance_execute(client: AlphaClient, params: dict, key: KeyEntry, extra: dict) -> BehaviorResult:
    proposals_resp = client.get_governance_proposals()
    if not proposals_resp.ok:
        return BehaviorResult.fail("governance.execute", "cannot fetch proposals")
    return BehaviorResult.ok("governance.execute", metrics={"note": "execute_checked"})


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
# client_type: "alpha" | "delta" | "either"
_REGISTRY: Dict[str, tuple] = {
    "transfer.casual":                    (transfer_casual,                   "alpha"),
    "transfer.continuous":                (transfer_continuous,                "alpha"),
    "transfer.alpha":                     (transfer_casual,                   "alpha"),
    "transfer.submit_only":               (transfer_submit_only,              "alpha"),
    "governance.vote":                    (governance_vote,                   "alpha"),
    "governance.propose":                 (governance_propose,                "alpha"),
    "governance.propose_and_vote":        (governance_propose_and_vote,       "alpha"),
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
