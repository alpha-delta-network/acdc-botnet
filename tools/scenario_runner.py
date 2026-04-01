"""
scenario_runner.py — Core runner for T005 botnet scenarios.

Handles both YAML formats:
  - T005 format (metadata/setup/phases at top level, wallets-based setup)
  - Legacy format (scenario.metadata/setup/phases, bots-based setup)

Flow per phase:
  1. Resolve bots and their key entries from KeySet
  2. Dispatch behavior calls (concurrent within a phase via threading)
  3. Collect BehaviorResult list
  4. Build EvaluationContext with results + network_state snapshot
  5. Evaluate assertions
  6. Accumulate phase results

Timeouts: per-phase via threading.Timer + threading.Event.
"""
from __future__ import annotations

import concurrent.futures
import time
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

from network_client import AlphaClient, DeltaClient
from key_loader import KeySet, KeyEntry
from behaviors import BehaviorResult, dispatch
from assertions import (
    AssertionResult, EvaluationContext, evaluate_all, phase_summary
)

# ─── Result types ─────────────────────────────────────────────────────────────

@dataclass
class PhaseResult:
    id: str
    name: str
    passed: bool
    skipped: bool = False
    behavior_results: List[BehaviorResult] = field(default_factory=list)
    assertion_results: List[AssertionResult] = field(default_factory=list)
    failures: List[str] = field(default_factory=list)
    duration_sec: float = 0.0
    timed_out: bool = False

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "name": self.name,
            "passed": self.passed,
            "skipped": self.skipped,
            "duration_sec": round(self.duration_sec, 2),
            "timed_out": self.timed_out,
            "behaviors_run": len(self.behavior_results),
            "behaviors_failed": sum(1 for r in self.behavior_results if not r.success),
            "assertions": [str(a) for a in self.assertion_results],
            "failures": self.failures,
        }


@dataclass
class ScenarioResult:
    scenario_id: str
    scenario_name: str
    passed: bool
    phases: List[PhaseResult] = field(default_factory=list)
    total_duration_sec: float = 0.0
    error: Optional[str] = None
    metrics: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "scenario_id": self.scenario_id,
            "scenario_name": self.scenario_name,
            "passed": self.passed,
            "total_duration_sec": round(self.total_duration_sec, 2),
            "error": self.error,
            "phases": [p.to_dict() for p in self.phases],
            "metrics": self.metrics,
            "phase_summary": {
                p.id: ("PASS" if p.passed else ("SKIP" if p.skipped else "FAIL"))
                for p in self.phases
            },
        }


# ─── Scenario normaliser ──────────────────────────────────────────────────────

def _normalise(raw: Dict[str, Any]) -> Dict[str, Any]:
    """
    Return a canonical scenario dict regardless of YAML format variant.

    Canonical:
      id, name, type, phases, setup (wallets list), success_criteria, metrics
    """
    # T005 format: metadata + setup.wallets + phases at top level
    if "metadata" in raw and "phases" in raw:
        meta = raw["metadata"]
        return {
            "id": meta.get("id", "unknown"),
            "name": meta.get("description", meta.get("id", "unnamed")),
            "type": meta.get("category", "security"),
            "setup": raw.get("setup", {}),
            "phases": raw.get("phases", []),
            "success_criteria": raw.get("success_criteria", {}),
            "metrics": raw.get("metrics", {}),
            "format": "t005",
        }

    # Legacy format: top-level "scenario" key
    if "scenario" in raw:
        sc = raw["scenario"]
        meta = sc.get("metadata", {})
        return {
            "id": meta.get("id", "unknown"),
            "name": meta.get("name", "unnamed"),
            "type": meta.get("type", "functional"),
            "setup": sc.get("setup", {}),
            "phases": sc.get("phases", []),
            "success_criteria": sc.get("success_criteria", {}),
            "metrics": sc.get("metrics", {}),
            "format": "legacy",
        }

    # Assume already canonical or unknown
    return raw


# ─── Key resolver ─────────────────────────────────────────────────────────────

def _resolve_key(wallet_spec: Any, keys: KeySet, funded_index: int) -> KeyEntry:
    """Resolve a wallet specification from scenario YAML to a KeyEntry."""
    if isinstance(wallet_spec, str):
        # Could be "keys.funded_wallets[0]" etc.
        val = keys.resolve(wallet_spec)
        if isinstance(val, dict):
            from key_loader import _parse_entry
            return _parse_entry(val)
    if isinstance(wallet_spec, dict):
        source = wallet_spec.get("source", "")
        if source:
            val = keys.resolve(source)
            if isinstance(val, dict):
                from key_loader import _parse_entry
                return _parse_entry(val)
        role = wallet_spec.get("role", "general_user")
        return keys.wallet_for_role(role, funded_index) or keys.funded_wallets[funded_index % max(1, len(keys.funded_wallets))]
    # Fallback
    return keys.funded_wallets[funded_index % max(1, len(keys.funded_wallets))]


# ─── Runner ───────────────────────────────────────────────────────────────────

class ScenarioRunner:
    def __init__(
        self,
        scenario: Dict[str, Any],
        alpha_host: str,
        delta_host: Optional[str],
        keys: KeySet,
        timeout_sec: int = 600,
        max_bots: int = 10,
        dry_run: bool = False,
        behavior_override: Optional[str] = None,
    ):
        self.scenario = _normalise(scenario)
        self.alpha_client = AlphaClient(alpha_host)
        self.delta_client = DeltaClient(delta_host) if delta_host else None
        self.keys = keys
        self.timeout_sec = timeout_sec
        self.max_bots = max_bots
        self.dry_run = dry_run
        self.behavior_override = behavior_override
        self._extra: Dict[str, Any] = {
            "funded_wallets": keys.funded_wallets,
            "governor_keys": keys.governor_keys,
        }

    def run(self) -> ScenarioResult:
        start = time.monotonic()
        sc = self.scenario
        result = ScenarioResult(
            scenario_id=sc.get("id", "unknown"),
            scenario_name=sc.get("name", "unnamed"),
            passed=False,
        )

        if self.dry_run:
            result.passed = True
            result.metrics["dry_run"] = True
            return result

        phases = sc.get("phases", [])
        scenario_format = sc.get("format", "legacy")
        all_passed = True

        for i, phase in enumerate(phases):
            phase_result = self._run_phase(phase, i, scenario_format)
            result.phases.append(phase_result)
            if not phase_result.passed and not phase_result.skipped:
                all_passed = False

        result.passed = all_passed
        result.total_duration_sec = time.monotonic() - start
        result.metrics = self._aggregate_metrics(result.phases)
        return result

    def _run_phase(self, phase: Dict[str, Any], index: int, fmt: str) -> PhaseResult:
        start = time.monotonic()
        phase_id = phase.get("id", f"phase_{index}")
        phase_name = phase.get("name", phase_id)
        phase_timeout = phase.get("timeout_sec", self.timeout_sec)

        pr = PhaseResult(id=phase_id, name=phase_name, passed=False)

        # Collect all actions (T005 format) or bot-behavior pairs (legacy)
        if fmt == "t005":
            actions = phase.get("actions", [])
        else:
            actions = self._legacy_phase_to_actions(phase)

        if not actions:
            pr.passed = True
            pr.skipped = True
            pr.duration_sec = time.monotonic() - start
            return pr

        # Run actions (concurrently up to max_bots)
        behavior_results: List[BehaviorResult] = []
        concurrent_limit = min(len(actions), self.max_bots)

        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrent_limit) as pool:
            futures = []
            deadline = time.monotonic() + phase_timeout
            for action in actions[:self.max_bots]:
                if time.monotonic() > deadline:
                    pr.timed_out = True
                    break
                fut = pool.submit(self._execute_action, action, fmt)
                futures.append(fut)

            try:
                for fut in concurrent.futures.as_completed(futures, timeout=phase_timeout):
                    try:
                        res = fut.result(timeout=30)
                        behavior_results.append(res)
                    except concurrent.futures.TimeoutError:
                        behavior_results.append(BehaviorResult.fail("timeout", "action timed out"))
                    except Exception as e:
                        behavior_results.append(BehaviorResult.fail("error", str(e)))
            except concurrent.futures.TimeoutError:
                # Phase-level timeout — collect already-completed futures
                pr.timed_out = True
                for fut in futures:
                    if fut.done():
                        try:
                            res = fut.result(timeout=1)
                            if not any(r is res for r in behavior_results):
                                behavior_results.append(res)
                        except Exception:
                            pass
                    else:
                        behavior_results.append(BehaviorResult.fail("timeout", "phase timed out"))
                        fut.cancel()

        pr.behavior_results = behavior_results

        # Network state snapshot for assertion context
        network_state = self._snapshot_network_state()

        # Build metrics
        metrics: Dict[str, Any] = {
            "total_forged_proofs_submitted": sum(
                1 for r in behavior_results if "forged_proof" in r.behavior or "forged" in r.behavior
            ),
            "total_forged_proofs_rejected": sum(
                1 for r in behavior_results
                if r.rejection_reason is not None and "forged" in r.behavior
            ),
        }
        for r in behavior_results:
            metrics.update(r.metrics)

        ctx = EvaluationContext(
            behavior_results=behavior_results,
            metrics=metrics,
            network_state=network_state,
            phase_id=phase_id,
            extra=self._extra,
        )

        assertions = phase.get("assertions", [])
        if isinstance(assertions, list):
            pr.assertion_results = evaluate_all(assertions, ctx)
        elif isinstance(assertions, dict):
            pr.assertion_results = evaluate_all([assertions], ctx)

        pr.passed, pr.failures = phase_summary(pr.assertion_results)
        pr.duration_sec = time.monotonic() - start

        # If no assertions defined but behaviors ran without errors, pass
        if not pr.assertion_results:
            no_errors = all(r.success for r in behavior_results)
            pr.passed = no_errors

        return pr

    def _execute_action(self, action: Dict[str, Any], fmt: str) -> BehaviorResult:
        """Execute a single action dict."""
        if fmt == "t005":
            behavior = self.behavior_override or action.get("behavior", "")
            params = action.get("params", {})
            wallet_spec = action.get("bot")
            # Find wallet setup from scenario
            wallet_index = 0
            wallets_spec = self.scenario.get("setup", {}).get("wallets", [])
            for wi, ws in enumerate(wallets_spec):
                if isinstance(ws, dict) and ws.get("id") == wallet_spec:
                    wallet_index = wi
                    break
            key = _resolve_key(
                wallets_spec[wallet_index] if wallets_spec else {},
                self.keys,
                wallet_index,
            )
        else:
            behavior = self.behavior_override or action.get("behavior", "")
            params = action.get("params", {})
            key = action.get("_key", self.keys.funded_wallets[0] if self.keys.funded_wallets else None)
            if key is None:
                return BehaviorResult.fail(behavior, "no_key_available")

        return dispatch(behavior, self.alpha_client, self.delta_client, params, key, self._extra)

    def _legacy_phase_to_actions(self, phase: Dict[str, Any]) -> List[Dict[str, Any]]:
        """Convert a legacy scenario phase into a list of action dicts."""
        actions = []
        behavior = phase.get("behavior", "")
        params = phase.get("params", {})

        if behavior:
            # Determine key(s) to use
            bots_spec = phase.get("bots", "")
            keys_for_phase = self._resolve_bot_keys(bots_spec)
            for key in keys_for_phase[:self.max_bots]:
                actions.append({"behavior": behavior, "params": params, "_key": key})

        # Concurrent bots
        for concurrent in phase.get("concurrent", []):
            c_behavior = concurrent.get("behavior", "")
            c_params = concurrent.get("params", {})
            c_bots = concurrent.get("bots", "")
            c_keys = self._resolve_bot_keys(c_bots)
            for key in c_keys[:max(1, self.max_bots // 2)]:
                actions.append({"behavior": c_behavior, "params": c_params, "_key": key})

        return actions

    def _resolve_bot_keys(self, bots_spec: str) -> List[KeyEntry]:
        """Map a bot spec string (e.g. 'user-*', 'governor-{1-5}') to KeyEntry list."""
        if not bots_spec:
            return [self.keys.funded_wallets[0]] if self.keys.funded_wallets else []

        bots_lower = bots_spec.lower()
        if "governor" in bots_lower:
            return list(self.keys.governor_keys) or list(self.keys.funded_wallets[:3])
        if "validator" in bots_lower:
            return list(self.keys.validator_keys) or list(self.keys.funded_wallets[:3])
        if "prover" in bots_lower and self.keys.prover_key:
            return [self.keys.prover_key]
        if "attacker" in bots_lower:
            return list(self.keys.funded_wallets[1:4])
        # Default: funded wallets
        return list(self.keys.funded_wallets[:3])

    def _snapshot_network_state(self) -> Dict[str, Any]:
        """Fetch a lightweight network state snapshot for assertion evaluation."""
        state: Dict[str, Any] = {}
        try:
            h_resp = self.alpha_client.get_height()
            state["rpc_ok"] = h_resp.ok
            state["block_height"] = (
                h_resp.body if isinstance(h_resp.body, int) else
                self.alpha_client.get_height_int()
            )
            state["node_crashed"] = not h_resp.ok
        except Exception as e:
            state["rpc_ok"] = False
            state["node_crashed"] = True
            state["rpc_error"] = str(e)
        return state

    @staticmethod
    def _aggregate_metrics(phases: List[PhaseResult]) -> Dict[str, Any]:
        totals: Dict[str, Any] = {
            "phases_run": len(phases),
            "phases_passed": sum(1 for p in phases if p.passed),
            "phases_failed": sum(1 for p in phases if not p.passed and not p.skipped),
            "total_behaviors": sum(len(p.behavior_results) for p in phases),
            "total_rejections": sum(
                sum(1 for r in p.behavior_results if r.rejection_reason) for p in phases
            ),
            "security_alerts": sum(
                sum(1 for r in p.behavior_results if "alert" in r.metrics) for p in phases
            ),
        }
        return totals


# ─── Convenience loader ───────────────────────────────────────────────────────

def load_scenario(path: str) -> Dict[str, Any]:
    """Load a scenario YAML file and return the raw dict."""
    try:
        import yaml
        with open(path) as f:
            return yaml.safe_load(f)
    except ImportError:
        import json
        with open(path) as f:
            return json.load(f)


def run_scenario_file(
    path: str,
    alpha_host: str,
    delta_host: Optional[str],
    keys: KeySet,
    timeout_sec: int = 300,
    max_bots: int = 5,
    dry_run: bool = False,
    behavior_override: Optional[str] = None,
) -> ScenarioResult:
    """Load a scenario from file and run it. Returns ScenarioResult."""
    try:
        raw = load_scenario(path)
    except FileNotFoundError:
        return ScenarioResult("unknown", path, passed=False,
                               error=f"Scenario file not found: {path}")
    except Exception as e:
        return ScenarioResult("unknown", path, passed=False, error=str(e))
    runner = ScenarioRunner(
        scenario=raw,
        alpha_host=alpha_host,
        delta_host=delta_host,
        keys=keys,
        timeout_sec=timeout_sec,
        max_bots=max_bots,
        dry_run=dry_run,
        behavior_override=behavior_override,
    )
    return runner.run()
