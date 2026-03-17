"""Tests for assertions.py"""
import pytest
from behaviors import BehaviorResult
from assertions import (
    EvaluationContext,
    AssertionResult,
    evaluate_assertion,
    evaluate_all,
    phase_summary,
    CheckEvaluator,
)


@pytest.fixture
def ctx_empty():
    return EvaluationContext()


@pytest.fixture
def ctx_all_rejected():
    results = [
        BehaviorResult.rejected("submit_forged_proof", "INVALID_PROOF", 400),
        BehaviorResult.rejected("submit_forged_proof", "INVALID_PROOF", 400),
        BehaviorResult.rejected("submit_forged_proof", "INVALID_PROOF", 400),
    ]
    return EvaluationContext(behavior_results=results)


@pytest.fixture
def ctx_alert():
    results = [
        BehaviorResult.fail("submit_forged_proof", "FORGED_PROOF_ACCEPTED", 200,
                             metrics={"alert": "ZK_SOUNDNESS_VIOLATION"})
    ]
    return EvaluationContext(behavior_results=results)


@pytest.fixture
def ctx_accepted():
    results = [
        BehaviorResult.ok("submit_shielded_transfer", tx_id="tx1", http_status=200),
    ]
    return EvaluationContext(behavior_results=results)


@pytest.fixture
def ctx_rpc_down():
    return EvaluationContext(
        behavior_results=[],
        network_state={"rpc_ok": False, "node_crashed": True}
    )


# ─── EvaluationContext helpers ────────────────────────────────────────────────

class TestEvaluationContext:
    def test_all_rejected(self, ctx_all_rejected):
        assert ctx_all_rejected.all_rejected() is True

    def test_none_rejected(self, ctx_accepted):
        assert ctx_accepted.none_rejected() is True

    def test_any_alert_false(self, ctx_all_rejected):
        assert ctx_all_rejected.any_alert() is False

    def test_any_alert_true(self, ctx_alert):
        assert ctx_alert.any_alert() is True

    def test_count_rejected(self, ctx_all_rejected):
        assert ctx_all_rejected.count_rejected() == 3

    def test_count_accepted(self, ctx_accepted):
        assert ctx_accepted.count_accepted() == 1

    def test_rpc_responding(self, ctx_empty):
        assert ctx_empty.rpc_responding() is True  # default True

    def test_rpc_down(self, ctx_rpc_down):
        assert ctx_rpc_down.rpc_responding() is False

    def test_results_for_behavior(self, ctx_all_rejected):
        r = ctx_all_rejected.results_for("submit_forged_proof")
        assert len(r) == 3


# ─── CheckEvaluator ──────────────────────────────────────────────────────────

class TestCheckEvaluator:
    ev = CheckEvaluator()

    def test_all_3_forged_proofs_rejected(self, ctx_all_rejected):
        passed, reason = self.ev.evaluate("ALL 3 forged proofs rejected with INVALID_PROOF", ctx_all_rejected)
        assert passed is True
        assert "3/3" in reason

    def test_all_3_forged_proofs_fails_when_alert(self, ctx_alert):
        passed, reason = self.ev.evaluate("ALL 3 forged proofs rejected with INVALID_PROOF", ctx_alert)
        assert passed is False

    def test_all_rejected_generic(self, ctx_all_rejected):
        passed, _ = self.ev.evaluate("ALL substituted transcript submissions rejected", ctx_all_rejected)
        assert passed is True

    def test_proof_200_ok(self, ctx_accepted):
        passed, _ = self.ev.evaluate("proof submission returns 200 OK", ctx_accepted)
        assert passed is True

    def test_proof_200_ok_false(self, ctx_empty):
        passed, _ = self.ev.evaluate("proof submission returns 200 OK", ctx_empty)
        assert passed is False

    def test_balance_unchanged_no_alert(self, ctx_all_rejected):
        passed, _ = self.ev.evaluate("proof_forger balance unchanged (no credits transferred)", ctx_all_rejected)
        assert passed is True

    def test_balance_changed_alert(self, ctx_alert):
        passed, _ = self.ev.evaluate("proof_forger balance unchanged", ctx_alert)
        assert passed is False

    def test_rpc_responding(self, ctx_empty):
        passed, _ = self.ev.evaluate("node remains stable", ctx_empty)
        assert passed is True

    def test_rpc_not_responding(self, ctx_rpc_down):
        passed, _ = self.ev.evaluate("rpc_responding after all phases", ctx_rpc_down)
        assert passed is False

    def test_no_bypass(self, ctx_all_rejected):
        passed, _ = self.ev.evaluate("no_bypass present", ctx_all_rejected)
        assert passed is True

    def test_bypass_detected(self, ctx_alert):
        passed, _ = self.ev.evaluate("no_bypass present", ctx_alert)
        assert passed is False

    def test_tx_confirmed(self, ctx_accepted):
        passed, _ = self.ev.evaluate("tx confirmed within 30s", ctx_accepted)
        assert passed is True

    def test_original_accepted(self, ctx_accepted):
        passed, _ = self.ev.evaluate("original proof ACCEPTED — protocol not disrupted", ctx_accepted)
        assert passed is True

    def test_fallback_no_alert(self, ctx_all_rejected):
        # Unknown check string — fallback based on alert presence
        passed, _ = self.ev.evaluate("some unknown check", ctx_all_rejected)
        assert passed is True

    def test_fallback_alert(self, ctx_alert):
        passed, _ = self.ev.evaluate("some unknown check", ctx_alert)
        assert passed is False


# ─── evaluate_assertion ───────────────────────────────────────────────────────

class TestEvaluateAssertion:
    def test_t005_style_dict(self, ctx_all_rejected):
        raw = {
            "id": "all_frozen_rng_rejected",
            "check": "ALL 3 forged proofs rejected with INVALID_PROOF",
            "must_pass": True,
        }
        result = evaluate_assertion(raw, ctx_all_rejected)
        assert result.id == "all_frozen_rng_rejected"
        assert result.passed is True
        assert result.must_pass is True

    def test_t005_style_critical(self, ctx_all_rejected):
        raw = {
            "id": "no_bypass",
            "check": "no_bypass present",
            "must_pass": True,
            "severity": "critical",
        }
        result = evaluate_assertion(raw, ctx_all_rejected)
        assert result.severity == "critical"

    def test_legacy_true_assertion_rejected(self, ctx_all_rejected):
        result = evaluate_assertion({"all_replays_rejected": True}, ctx_all_rejected)
        assert result.passed is True

    def test_legacy_working_assertion(self, ctx_all_rejected):
        result = evaluate_assertion({"nonce_tracking_working": True}, ctx_all_rejected)
        assert result.passed is True

    def test_string_assertion(self, ctx_empty):
        result = evaluate_assertion("some check string", ctx_empty)
        assert isinstance(result, AssertionResult)

    def test_unknown_format(self, ctx_empty):
        result = evaluate_assertion(42, ctx_empty)
        assert result.passed is True  # unrecognized = pass with caveat


# ─── evaluate_all ────────────────────────────────────────────────────────────

class TestEvaluateAll:
    def test_empty_list(self, ctx_empty):
        results = evaluate_all([], ctx_empty)
        assert results == []

    def test_none(self, ctx_empty):
        results = evaluate_all(None, ctx_empty)
        assert results == []

    def test_multiple(self, ctx_all_rejected):
        assertions = [
            {"id": "a1", "check": "ALL rejected", "must_pass": True},
            {"id": "a2", "check": "no_bypass present", "must_pass": True},
        ]
        results = evaluate_all(assertions, ctx_all_rejected)
        assert len(results) == 2
        assert all(r.passed for r in results)


# ─── phase_summary ────────────────────────────────────────────────────────────

class TestPhaseSummary:
    def test_all_pass(self):
        results = [
            AssertionResult("a1", True, True),
            AssertionResult("a2", True, True),
        ]
        passed, failures = phase_summary(results)
        assert passed is True
        assert failures == []

    def test_one_must_pass_fails(self):
        results = [
            AssertionResult("a1", True, True),
            AssertionResult("a2", False, True, "something failed"),
        ]
        passed, failures = phase_summary(results)
        assert passed is False
        assert len(failures) == 1

    def test_warn_only_still_passes(self):
        results = [
            AssertionResult("a1", True, True),
            AssertionResult("a2", False, False, "warning only"),  # must_pass=False
        ]
        passed, failures = phase_summary(results)
        assert passed is True
        assert failures == []

    def test_str_format(self):
        r_pass = AssertionResult("test", True, True, "ok")
        r_fail = AssertionResult("test", False, True, "err")
        r_warn = AssertionResult("test", False, False, "warn")
        assert "[PASS]" in str(r_pass)
        assert "[FAIL]" in str(r_fail)
        assert "[WARN]" in str(r_warn)
