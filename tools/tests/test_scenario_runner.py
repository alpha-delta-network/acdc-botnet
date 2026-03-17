"""Tests for scenario_runner.py"""
import pytest
from unittest.mock import patch, MagicMock
from network_client import Response
from key_loader import make_stub
from scenario_runner import (
    ScenarioRunner, ScenarioResult, PhaseResult,
    _normalise, run_scenario_file, load_scenario,
)


# ─── _normalise ───────────────────────────────────────────────────────────────

class TestNormalise:
    def test_t005_format(self):
        raw = {
            "metadata": {"id": "frozen_heart", "category": "security"},
            "setup": {"wallets": []},
            "phases": [{"id": "phase1"}],
        }
        n = _normalise(raw)
        assert n["id"] == "frozen_heart"
        assert n["type"] == "security"
        assert n["format"] == "t005"
        assert n["phases"] == [{"id": "phase1"}]

    def test_legacy_format(self):
        raw = {
            "scenario": {
                "metadata": {"id": "SEC-008", "name": "Replay Attack", "type": "security"},
                "setup": {},
                "phases": [{"name": "p1"}],
            }
        }
        n = _normalise(raw)
        assert n["id"] == "SEC-008"
        assert n["name"] == "Replay Attack"
        assert n["format"] == "legacy"

    def test_unknown_format_passes_through(self):
        raw = {"id": "x", "phases": []}
        n = _normalise(raw)
        assert n["id"] == "x"


# ─── ScenarioResult ───────────────────────────────────────────────────────────

class TestScenarioResult:
    def test_to_dict(self):
        r = ScenarioResult(scenario_id="T3.2", scenario_name="Frozen Heart", passed=True)
        d = r.to_dict()
        assert d["scenario_id"] == "T3.2"
        assert d["passed"] is True
        assert "phases" in d
        assert "metrics" in d

    def test_phase_summary_in_dict(self):
        pr = PhaseResult(id="p1", name="Phase 1", passed=True)
        r = ScenarioResult("x", "y", True, phases=[pr])
        d = r.to_dict()
        assert d["phase_summary"]["p1"] == "PASS"

    def test_skipped_phase_in_summary(self):
        pr = PhaseResult(id="p1", name="Phase 1", passed=False, skipped=True)
        r = ScenarioResult("x", "y", True, phases=[pr])
        d = r.to_dict()
        assert d["phase_summary"]["p1"] == "SKIP"


# ─── ScenarioRunner — dry run ─────────────────────────────────────────────────

class TestScenarioRunnerDryRun:
    def test_dry_run_always_passes(self, stub_keys):
        scenario = {
            "scenario": {
                "metadata": {"id": "test"},
                "phases": [{"name": "p1", "behavior": "transfer.casual"}],
            }
        }
        runner = ScenarioRunner(
            scenario=scenario,
            alpha_host="localhost",
            delta_host=None,
            keys=stub_keys,
            dry_run=True,
        )
        result = runner.run()
        assert result.passed is True
        assert result.metrics.get("dry_run") is True

    def test_dry_run_no_http_calls(self, stub_keys):
        scenario = {"scenario": {"metadata": {"id": "test"}, "phases": []}}
        with patch("scenario_runner.AlphaClient") as mock_cls:
            runner = ScenarioRunner(
                scenario=scenario, alpha_host="localhost",
                delta_host=None, keys=stub_keys, dry_run=True,
            )
            result = runner.run()
        # broadcast should not be called in dry run
        if mock_cls.return_value.broadcast_transaction.called:
            assert False, "HTTP calls made in dry_run mode"


# ─── ScenarioRunner — T005 format ────────────────────────────────────────────

class TestScenarioRunnerT005:
    @pytest.fixture
    def t005_scenario(self, stub_keys):
        return {
            "metadata": {"id": "test_scenario", "category": "security"},
            "setup": {
                "wallets": [
                    {"id": "honest_user", "role": "general_user", "source": "keys.funded_wallets[0]"},
                    {"id": "attacker", "role": "general_user", "source": "keys.funded_wallets[1]"},
                ]
            },
            "phases": [
                {
                    "id": "phase_rejection",
                    "timeout_sec": 30,
                    "actions": [
                        {"bot": "attacker", "behavior": "submit_forged_proof",
                         "params": {"attack_type": "frozen_rng", "expected_rejection": "INVALID_PROOF"}},
                    ],
                    "assertions": [
                        {"id": "rejected", "check": "ALL rejected", "must_pass": True},
                    ],
                }
            ],
        }

    def test_forged_proof_rejection_passes(self, t005_scenario, stub_keys):
        key = stub_keys.funded_wallets[1]

        with patch("scenario_runner.AlphaClient") as MockAlpha, \
             patch("scenario_runner.DeltaClient"):
            client = MockAlpha.return_value
            client.broadcast_transaction.return_value = Response(400, {"error": "INVALID_PROOF"},
                                                                  error="HTTP 400")
            client.get_height.return_value = Response(200, 42, raw=b"42")
            client.get_height_int.return_value = 42

            runner = ScenarioRunner(t005_scenario, "localhost", None, stub_keys, timeout_sec=30, max_bots=3)
            result = runner.run()

        assert result.passed is True
        assert result.phases[0].passed is True

    def test_forged_proof_accepted_fails(self, t005_scenario, stub_keys):
        with patch("scenario_runner.AlphaClient") as MockAlpha, \
             patch("scenario_runner.DeltaClient"):
            client = MockAlpha.return_value
            client.broadcast_transaction.return_value = Response(200, {"transaction_id": "tx1"},
                                                                  raw=b'{"transaction_id":"tx1"}')
            client.get_height.return_value = Response(200, 42, raw=b"42")
            client.get_height_int.return_value = 42

            runner = ScenarioRunner(t005_scenario, "localhost", None, stub_keys, timeout_sec=30, max_bots=3)
            result = runner.run()

        assert result.passed is False


# ─── ScenarioRunner — legacy format ──────────────────────────────────────────

class TestScenarioRunnerLegacy:
    @pytest.fixture
    def legacy_scenario(self):
        return {
            "scenario": {
                "metadata": {"id": "FUNC-001", "name": "Test", "type": "functional"},
                "setup": {"network": {"alpha_rest": "http://localhost:3030"}},
                "phases": [
                    {
                        "name": "Transfer phase",
                        "bots": "user-*",
                        "behavior": "transfer.casual",
                        "params": {"amount": 100},
                        "assertions": [{"all_transfers_successful": True}],
                    }
                ],
            }
        }

    def test_transfer_phase_passes(self, legacy_scenario, stub_keys):
        with patch("scenario_runner.AlphaClient") as MockAlpha:
            client = MockAlpha.return_value
            client.broadcast_transaction.return_value = Response(
                200, {"transaction_id": "tx1"}, raw=b'{"transaction_id":"tx1"}'
            )
            client.get_height.return_value = Response(200, 100, raw=b"100")
            client.get_height_int.return_value = 100

            runner = ScenarioRunner(legacy_scenario, "localhost", None, stub_keys,
                                     timeout_sec=30, max_bots=3)
            result = runner.run()

        # Phase should pass (transfers accepted + legacy assertion "all_transfers_successful" evaluates true)
        assert result.passed is True

    def test_no_phases_passes(self, stub_keys):
        scenario = {"scenario": {"metadata": {"id": "empty"}, "phases": []}}
        runner = ScenarioRunner(scenario, "localhost", None, stub_keys, dry_run=True)
        result = runner.run()
        assert result.passed is True

    def test_empty_phase_skipped(self, stub_keys):
        scenario = {
            "scenario": {
                "metadata": {"id": "test"},
                "phases": [{"name": "no_behavior"}],  # no behavior or actions
            }
        }
        with patch("scenario_runner.AlphaClient"):
            runner = ScenarioRunner(scenario, "localhost", None, stub_keys, timeout_sec=5, max_bots=1)
            result = runner.run()
        assert result.phases[0].skipped is True
        assert result.passed is True


# ─── Aggregate metrics ────────────────────────────────────────────────────────

class TestAggregateMetrics:
    def test_metrics_structure(self):
        phases = [
            PhaseResult(id="p1", name="p1", passed=True),
            PhaseResult(id="p2", name="p2", passed=False),
        ]
        metrics = ScenarioRunner._aggregate_metrics(phases)
        assert metrics["phases_run"] == 2
        assert metrics["phases_passed"] == 1
        assert metrics["phases_failed"] == 1

    def test_security_alerts_counted(self):
        from behaviors import BehaviorResult
        pr = PhaseResult(id="p1", name="p1", passed=False)
        pr.behavior_results = [
            BehaviorResult.fail("b1", "err", metrics={"alert": "ZK_SOUNDNESS"}),
        ]
        metrics = ScenarioRunner._aggregate_metrics([pr])
        assert metrics["security_alerts"] == 1


# ─── load_scenario ────────────────────────────────────────────────────────────

class TestLoadScenario:
    def test_load_yaml_file(self, tmp_path):
        f = tmp_path / "test.yaml"
        f.write_text("metadata:\n  id: test_id\nphases: []\n")
        raw = load_scenario(str(f))
        assert raw["metadata"]["id"] == "test_id"


# ─── run_scenario_file integration ───────────────────────────────────────────

class TestRunScenarioFile:
    def test_dry_run(self, tmp_path, stub_keys):
        f = tmp_path / "scenario.yaml"
        f.write_text("""
metadata:
  id: test
  category: security
setup:
  wallets: []
phases: []
""")
        result = run_scenario_file(
            str(f),
            alpha_host="localhost",
            delta_host=None,
            keys=stub_keys,
            dry_run=True,
        )
        assert result.passed is True

    def test_missing_file_returns_error(self, stub_keys):
        result = run_scenario_file(
            "/tmp/nonexistent_scenario_xyz.yaml",
            alpha_host="localhost",
            delta_host=None,
            keys=stub_keys,
        )
        assert result.passed is False
        assert result.error is not None
