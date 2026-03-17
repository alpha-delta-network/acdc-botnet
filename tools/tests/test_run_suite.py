"""Tests for run-suite.py CLI entry point."""
import sys
import json
import pytest
from pathlib import Path
from unittest.mock import patch, MagicMock

# Ensure tools/ is in path
TOOLS_DIR = Path(__file__).parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import importlib.util
spec = importlib.util.spec_from_file_location("run_suite", TOOLS_DIR / "run-suite.py")
run_suite_mod = importlib.util.module_from_spec(spec)  # type: ignore
spec.loader.exec_module(run_suite_mod)  # type: ignore


SUITE_MAP = run_suite_mod.SUITE_MAP


# ─── Suite map ───────────────────────────────────────────────────────────────

class TestSuiteMap:
    def test_all_t1_present(self):
        assert "T1.3" in SUITE_MAP

    def test_all_t2_present(self):
        for sid in ["T2.1", "T2.2", "T2.3", "T2.4", "T2.5"]:
            assert sid in SUITE_MAP, f"Missing suite {sid}"

    def test_all_t3_present(self):
        for sid in ["T3.1", "T3.2", "T3.3", "T3.4", "T3.5"]:
            assert sid in SUITE_MAP, f"Missing suite {sid}"

    def test_t32_is_critical(self):
        assert SUITE_MAP["T3.2"].get("critical") is True

    def test_t1_timeout_shorter_than_t3(self):
        assert SUITE_MAP["T1.3"]["timeout"] < SUITE_MAP["T3.2"]["timeout"]

    def test_scenario_paths_exist(self):
        scenarios_root = TOOLS_DIR.parent / "scenarios"
        missing = []
        for sid, cfg in SUITE_MAP.items():
            p = scenarios_root / cfg["scenario"]
            if not p.exists():
                missing.append(f"{sid}: {p}")
        assert missing == [], f"Missing scenario files:\n" + "\n".join(missing)

    def test_all_have_tier(self):
        for sid, cfg in SUITE_MAP.items():
            assert "tier" in cfg, f"Suite {sid} missing 'tier'"
            assert cfg["tier"] in (1, 2, 3)


# ─── format_text ─────────────────────────────────────────────────────────────

class TestFormatText:
    def test_pass(self):
        from scenario_runner import ScenarioResult
        r = ScenarioResult("T3.2", "Frozen Heart", True)
        text = run_suite_mod.format_text(r)
        assert "PASS" in text
        assert "Frozen Heart" in text

    def test_fail_with_error(self):
        from scenario_runner import ScenarioResult
        r = ScenarioResult("T3.2", "Frozen Heart", False, error="test error")
        text = run_suite_mod.format_text(r)
        assert "FAIL" in text
        assert "test error" in text


# ─── resolve_output ──────────────────────────────────────────────────────────

class TestResolveOutput:
    def test_explicit_output(self):
        args = MagicMock(output="/tmp/custom.yaml", deploy_id="")
        result = run_suite_mod.resolve_output(args, "T1.3")
        assert result == "/tmp/custom.yaml"

    def test_deploy_id_generates_path(self):
        args = MagicMock(output=None, deploy_id="abc123")
        result = run_suite_mod.resolve_output(args, "T1.3")
        assert result is not None
        assert "abc123" in result
        assert "T1.3" in result

    def test_no_output_no_deploy_id(self):
        args = MagicMock(output=None, deploy_id="")
        result = run_suite_mod.resolve_output(args, "T1.3")
        assert result is None


# ─── run_suite (unit) ────────────────────────────────────────────────────────

class TestRunSuiteFunction:
    def test_unknown_suite_returns_error(self, stub_keys):
        result = run_suite_mod.run_suite(
            suite_id="T9.9",
            nodes=["localhost"],
            delta_node=None,
            keys=stub_keys,
            timeout_override=0,
            max_bots_override=0,
            dry_run=True,
            verbose=False,
        )
        assert result.passed is False
        assert "Unknown suite" in (result.error or "")

    def test_known_suite_dry_run(self, stub_keys):
        result = run_suite_mod.run_suite(
            suite_id="T1.3",
            nodes=["testnet001.ac-dc.network"],
            delta_node=None,
            keys=stub_keys,
            timeout_override=30,
            max_bots_override=2,
            dry_run=True,
            verbose=False,
        )
        assert result.passed is True

    def test_missing_scenario_file_returns_error(self, stub_keys, monkeypatch):
        # Temporarily remove the scenario path
        original = SUITE_MAP.get("T2.1", {}).copy()
        SUITE_MAP["T2.1"]["scenario"] = "functional/nonexistent_xyz.yaml"
        try:
            result = run_suite_mod.run_suite(
                suite_id="T2.1",
                nodes=["localhost"],
                delta_node=None,
                keys=stub_keys,
                timeout_override=10,
                max_bots_override=1,
                dry_run=True,
                verbose=False,
            )
            assert result.passed is False
            assert result.error is not None
        finally:
            SUITE_MAP["T2.1"].update(original)


# ─── main() ──────────────────────────────────────────────────────────────────

class TestMainCLI:
    def test_list_suites(self, capsys):
        with patch.object(sys, "argv", ["run-suite.py", "--list-suites"]):
            exit_code = run_suite_mod.main()
        assert exit_code == 0
        out = capsys.readouterr().out
        assert "T1.3" in out
        assert "T3.2" in out

    def test_no_suite_or_scenario_exits_2(self, capsys):
        with patch.object(sys, "argv", ["run-suite.py"]):
            exit_code = run_suite_mod.main()
        assert exit_code == 2

    def test_dry_run_suite(self):
        # No --keys arg → uses make_stub; --dry-run skips HTTP calls
        with patch.object(sys, "argv", [
            "run-suite.py",
            "--suite", "T1.3",
            "--dry-run",
        ]):
            exit_code = run_suite_mod.main()
        assert exit_code == 0

    def test_missing_key_file_exits_2(self, tmp_path):
        with patch.object(sys, "argv", [
            "run-suite.py",
            "--suite", "T1.3",
            "--keys", str(tmp_path / "nonexistent.yaml"),
        ]):
            exit_code = run_suite_mod.main()
        assert exit_code == 2
