"""Tests for behaviors.py"""
import pytest
from unittest.mock import MagicMock
from network_client import Response
from behaviors import (
    BehaviorResult,
    transfer_casual, transfer_submit_only,
    governance_vote, governance_propose,
    replay_direct, replay_modified, replay_cross_chain,
    submit_forged_proof, submit_shielded_without_proof,
    submit_shielded_with_empty_proof, submit_shielded_with_zero_proof,
    transcript_substitution_attack, mapping_commitment_substitution,
    monitor_mempool, dispatch,
)
from key_loader import make_stub


@pytest.fixture
def key(stub_keys):
    return stub_keys.funded_wallets[0]


@pytest.fixture
def extra(stub_keys):
    return {"funded_wallets": stub_keys.funded_wallets}


# ─── BehaviorResult ───────────────────────────────────────────────────────────

class TestBehaviorResult:
    def test_ok(self):
        r = BehaviorResult.ok("test_behavior", tx_id="abc")
        assert r.success is True
        assert r.tx_id == "abc"
        assert r.behavior == "test_behavior"

    def test_fail(self):
        r = BehaviorResult.fail("test_behavior", "some error", http_status=500)
        assert r.success is False
        assert r.error == "some error"
        assert r.http_status == 500

    def test_rejected(self):
        r = BehaviorResult.rejected("test_behavior", "INVALID_PROOF", 400)
        assert r.success is True   # correctly rejected = test success
        assert r.rejection_reason == "INVALID_PROOF"
        assert r.http_status == 400
        assert r.confirmed is False


# ─── Transfer behaviors ───────────────────────────────────────────────────────

class TestTransferCasual:
    def test_success(self, mock_alpha_client, key, extra):
        result = transfer_casual(mock_alpha_client, {}, key, extra)
        assert result.success is True
        assert result.tx_id is not None

    def test_failure_on_server_error(self, key, extra):
        client = MagicMock()
        client.broadcast_transaction.return_value = Response(500, {"error": "server error"}, error="HTTP 500")
        result = transfer_casual(client, {}, key, extra)
        assert result.success is False
        assert result.http_status == 500

    def test_submit_only_success(self, mock_alpha_client, key, extra):
        result = transfer_submit_only(mock_alpha_client, {}, key, extra)
        assert result.success is True

    def test_submit_only_409_ok(self, key, extra):
        client = MagicMock()
        client.broadcast_transaction.return_value = Response(409, {"error": "duplicate"}, error="HTTP 409")
        result = transfer_submit_only(client, {}, key, extra)
        assert result.success is True
        assert result.metrics.get("note") == "duplicate_or_known"


# ─── Governance behaviors ─────────────────────────────────────────────────────

class TestGovernanceBehaviors:
    def test_vote_success(self, mock_alpha_client, key, extra):
        result = governance_vote(mock_alpha_client, {"vote": "yes"}, key, extra)
        assert result.success is True

    def test_vote_no_proposals(self, key, extra):
        client = MagicMock()
        client.get_governance_proposals.return_value = Response(200, [], raw=b"[]")
        result = governance_vote(client, {}, key, extra)
        assert result.success is True
        assert result.metrics.get("note") == "no_proposals"

    def test_propose_success(self, mock_alpha_client, key, extra):
        result = governance_propose(mock_alpha_client, {"proposal_type": "parameter_change"}, key, extra)
        assert result.success is True

    def test_proposals_api_failure(self, key, extra):
        """governance_vote treats API failures as non-fatal (governance may not be deployed)."""
        client = MagicMock()
        client.get_governance_proposals.return_value = Response(503, None, error="HTTP 503")
        result = governance_vote(client, {}, key, extra)
        # Non-fatal: governance_vote returns ok with note when proposals unreachable
        assert result.success is True
        assert result.metrics.get("note") == "governance_not_deployed"


# ─── Replay attack behaviors ──────────────────────────────────────────────────

class TestReplayBehaviors:
    def test_replay_direct_correctly_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = replay_direct(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True
        assert result.rejection_reason is not None

    def test_replay_direct_failure_when_accepted(self, mock_alpha_client, key, extra):
        """If replay is accepted (bug), behavior returns failure."""
        result = replay_direct(mock_alpha_client, {}, key, extra)
        # The mock accepts everything — this exposes the bug
        assert result.success is False
        assert "REPLAY_ACCEPTED" in str(result.error)

    def test_replay_modified_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = replay_modified(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True
        assert result.rejection_reason == "signature_invalid"

    def test_replay_cross_chain_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = replay_cross_chain(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True
        assert result.rejection_reason == "chain_id_mismatch"


# ─── ZK security behaviors ───────────────────────────────────────────────────

class TestZKBehaviors:
    def test_forged_proof_correctly_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = submit_forged_proof(mock_alpha_client_rejecting, {"attack_type": "frozen_rng"}, key, extra)
        assert result.success is True
        assert result.rejection_reason is not None

    def test_forged_proof_accepted_is_alert(self, mock_alpha_client, key, extra):
        """If forged proof accepted — ZK soundness violation alert."""
        result = submit_forged_proof(mock_alpha_client, {"attack_type": "frozen_rng"}, key, extra)
        assert result.success is False
        assert result.metrics.get("alert") == "ZK_SOUNDNESS_VIOLATION"

    def test_shielded_without_proof_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = submit_shielded_without_proof(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True
        assert result.rejection_reason == "PROOF_MISSING"

    def test_shielded_without_proof_accepted_is_alert(self, mock_alpha_client, key, extra):
        result = submit_shielded_without_proof(mock_alpha_client, {}, key, extra)
        assert result.success is False
        assert result.metrics.get("alert") == "OWNERSHIP_BYPASS"

    def test_empty_proof_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = submit_shielded_with_empty_proof(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True

    def test_zero_proof_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = submit_shielded_with_zero_proof(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True

    def test_transcript_substitution_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = transcript_substitution_attack(mock_alpha_client_rejecting, {
            "step1": {"amount": 1},
            "step2": {"substitute_inputs": {"amount": 999999}},
        }, key, extra)
        assert result.success is True

    def test_transcript_substitution_accepted_is_alert(self, mock_alpha_client, key, extra):
        result = transcript_substitution_attack(mock_alpha_client, {
            "step1": {"amount": 1},
            "step2": {"substitute_inputs": {"amount": 999999}},
        }, key, extra)
        assert result.success is False
        assert result.metrics.get("alert") == "ZK_TRANSCRIPT_NOT_BOUND"

    def test_mapping_commitment_rejected(self, mock_alpha_client_rejecting, key, extra):
        result = mapping_commitment_substitution(mock_alpha_client_rejecting, {}, key, extra)
        assert result.success is True

    def test_mapping_commitment_accepted_is_alert(self, mock_alpha_client, key, extra):
        result = mapping_commitment_substitution(mock_alpha_client, {}, key, extra)
        assert result.success is False
        assert result.metrics.get("alert") == "MAPPING_NOT_BOUND"


# ─── Monitor ─────────────────────────────────────────────────────────────────

class TestMonitor:
    def test_monitor_mempool(self, mock_alpha_client, key, extra):
        result = monitor_mempool(mock_alpha_client, {}, key, extra)
        assert result.success is True

    def test_monitor_mempool_failure(self, key, extra):
        client = MagicMock()
        client.get_mempool.return_value = Response(503, None, error="HTTP 503")
        result = monitor_mempool(client, {}, key, extra)
        assert result.success is False


# ─── Dispatch ────────────────────────────────────────────────────────────────

class TestDispatch:
    def test_dispatch_known_alpha(self, mock_alpha_client, key, extra):
        result = dispatch("transfer.casual", mock_alpha_client, None, {}, key, extra)
        assert result.behavior == "transfer.casual"

    def test_dispatch_unknown_returns_ok_stub(self, mock_alpha_client, key, extra):
        result = dispatch("unknown.behavior.xyz", mock_alpha_client, None, {}, key, extra)
        assert result.success is True
        assert "behavior_not_implemented" in str(result.metrics.get("note", ""))

    def test_dispatch_delta_without_client(self, mock_alpha_client, key, extra):
        result = dispatch("dex.spot_trade", mock_alpha_client, None, {}, key, extra)
        assert result.success is False
        assert "delta_client_not_available" in str(result.error)
