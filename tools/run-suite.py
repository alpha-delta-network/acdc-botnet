#!/usr/bin/env python3
"""
run-suite.py — CLI entry point for the T005 Python test runner.

Usage:
  python3 run-suite.py --suite T1.3 --keys /tmp/testnet-keys-<id>.yaml \
      --nodes testnet001.ac-dc.network \
      --output /tmp/suite-result.yaml \
      --deploy-id <id>

  python3 run-suite.py --scenario scenarios/security/frozen_heart_soundness.yaml \
      --keys /tmp/testnet-keys-<id>.yaml \
      --nodes testnet001.ac-dc.network \
      --output /tmp/result.yaml

Exit codes:
  0  PASS
  1  FAIL (assertions failed)
  2  ERROR (infrastructure / load error)
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Any

# Ensure tools/ is on PYTHONPATH
_HERE = Path(__file__).parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

try:
    import yaml
    def _dump(d: Any) -> str:
        return yaml.dump(d, default_flow_style=False, sort_keys=False)
    def _load(s: str) -> Any:
        return yaml.safe_load(s)
except ImportError:
    def _dump(d: Any) -> str:  # type: ignore
        return json.dumps(d, indent=2)
    def _load(s: str) -> Any:  # type: ignore
        return json.loads(s)

from key_loader import KeySet, load as load_keys, make_stub
from scenario_runner import run_scenario_file, ScenarioResult

# ─── Suite definitions ────────────────────────────────────────────────────────

# Base dir for scenario files (relative to tools/)
_SCENARIOS_ROOT = _HERE.parent / "scenarios"

# T005 suite ID → (scenario path, timeout_sec, max_bots, tier)
SUITE_MAP: Dict[str, Dict[str, Any]] = {
    # Tier 1 — fail-fast, short timeout
    "T1.3": {
        "scenario": "functional/daily_network_ops.yaml",
        "timeout": 120,
        "max_bots": 3,
        "tier": 1,
        "description": "Basic tx lifecycle (submit transfer + confirm)",
        "behavior_override": "transfer.submit_only",   # run only the transfer part
    },
    # Tier 2 — feature complete
    "T2.1": {
        "scenario": "functional/governance_lifecycle.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 2,
        "description": "Governance lifecycle (propose, vote, execute)",
    },
    "T2.2": {
        "scenario": "functional/cross_chain_stress.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 2,
        "description": "Cross-chain bridge stress",
    },
    "T2.3": {
        "scenario": "functional/privacy_operations.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 2,
        "description": "ZK privacy operations",
    },
    "T2.4": {
        "scenario": "functional/validator_operations.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 2,
        "description": "Validator operations",
    },
    "T2.5": {
        "scenario": "functional/dex_trading_session.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 2,
        "description": "DEX trading session",
    },
    # Tier 3 — security, async
    "T3.1": {
        "scenario": "security/byzantine_validators.yaml",
        "timeout": 600,
        "max_bots": 5,
        "tier": 3,
        "description": "Byzantine mesh fault injection",
    },
    "T3.2": {
        "scenario": "security/frozen_heart_soundness.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 3,
        "description": "Frozen Heart ZK soundness (Fiat-Shamir)",
        "critical": True,
    },
    "T3.3": {
        "scenario": "security/pool_dos_extreme_heights.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 3,
        "description": "Pool DoS + extreme heights",
    },
    "T3.4": {
        "scenario": "security/replay_attack.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 3,
        "description": "Replay attack (nonce, signature, cross-chain)",
    },
    "T3.5": {
        "scenario": "security/mev_extraction.yaml",
        "timeout": 300,
        "max_bots": 5,
        "tier": 3,
        "description": "MEV extraction",
    },
}


# ─── CLI ──────────────────────────────────────────────────────────────────────

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="T005 Python botnet test runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--suite", metavar="T1.3|T2.1|...", help="Run a named T005 suite")
    p.add_argument("--scenario", metavar="PATH", help="Run a specific scenario YAML file")
    p.add_argument("--keys", metavar="PATH", help="Path to testnet key YAML (default: stub)")
    p.add_argument("--nodes", metavar="HOST[,HOST,...]",
                   default="testnet001.ac-dc.network",
                   help="Comma-separated validator hostnames")
    p.add_argument("--delta-node", metavar="HOST", help="DeltaOS REST host (optional)")
    p.add_argument("--output", metavar="PATH", help="Write result YAML to this path")
    p.add_argument("--deploy-id", metavar="ID", default="",
                   help="Deploy ID (for output filename resolution)")
    p.add_argument("--timeout", metavar="SEC", type=int, default=0,
                   help="Override timeout (0 = use suite default)")
    p.add_argument("--max-bots", metavar="N", type=int, default=0,
                   help="Override max concurrent bots (0 = use suite default)")
    p.add_argument("--dry-run", action="store_true",
                   help="Parse and validate scenario without executing")
    p.add_argument("--list-suites", action="store_true", help="List all suite IDs and exit")
    p.add_argument("--format", choices=["yaml", "json", "text"], default="yaml",
                   help="Output format (default: yaml)")
    p.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    return p.parse_args()


def list_suites() -> None:
    print("Available suites:")
    print(f"  {'Suite':<8} {'Tier':<6} {'Timeout':<10} {'Description'}")
    print(f"  {'-'*8} {'-'*6} {'-'*10} {'-'*40}")
    for sid, cfg in SUITE_MAP.items():
        tier = f"T{cfg['tier']}"
        timeout = f"{cfg['timeout']}s"
        desc = cfg.get("description", "")
        crit = " [CRITICAL]" if cfg.get("critical") else ""
        print(f"  {sid:<8} {tier:<6} {timeout:<10} {desc}{crit}")


def resolve_output(args: argparse.Namespace, suite_or_scenario: str) -> Optional[str]:
    if args.output:
        return args.output
    if args.deploy_id:
        name = suite_or_scenario.replace("/", "_").replace(".yaml", "")
        return f"/tmp/botnet-result-{args.deploy_id}-{name}.yaml"
    return None


def write_output(result: ScenarioResult, path: Optional[str], fmt: str) -> None:
    d = result.to_dict()
    if fmt == "json":
        text = json.dumps(d, indent=2)
    elif fmt == "text":
        text = format_text(result)
    else:
        text = _dump(d)
    if path:
        with open(path, "w") as f:
            f.write(text)
    else:
        print(text)


def format_text(result: ScenarioResult) -> str:
    lines = [
        f"{'='*60}",
        f"SCENARIO: {result.scenario_name} ({result.scenario_id})",
        f"RESULT:   {'PASS' if result.passed else 'FAIL'}",
        f"DURATION: {result.total_duration_sec:.1f}s",
        f"{'='*60}",
    ]
    if result.error:
        lines.append(f"ERROR: {result.error}")
    for p in result.phases:
        status = "PASS" if p.passed else ("SKIP" if p.skipped else "FAIL")
        lines.append(f"  [{status}] {p.name} ({p.duration_sec:.1f}s)")
        for f_msg in p.failures:
            lines.append(f"       {f_msg}")
    if result.metrics:
        lines.append("")
        lines.append("Metrics:")
        for k, v in result.metrics.items():
            lines.append(f"  {k}: {v}")
    return "\n".join(lines)


def run_suite(
    suite_id: str,
    nodes: List[str],
    delta_node: Optional[str],
    keys: KeySet,
    timeout_override: int,
    max_bots_override: int,
    dry_run: bool,
    verbose: bool,
) -> ScenarioResult:
    cfg = SUITE_MAP.get(suite_id)
    if not cfg:
        return ScenarioResult(
            scenario_id=suite_id,
            scenario_name=suite_id,
            passed=False,
            error=f"Unknown suite: {suite_id}",
        )

    scenario_path = _SCENARIOS_ROOT / cfg["scenario"]
    if not scenario_path.exists():
        return ScenarioResult(
            scenario_id=suite_id,
            scenario_name=cfg.get("description", suite_id),
            passed=False,
            error=f"Scenario file not found: {scenario_path}",
        )

    timeout = timeout_override or cfg["timeout"]
    max_bots = max_bots_override or cfg["max_bots"]
    alpha_host = nodes[0]  # primary node

    if verbose:
        print(f"[{suite_id}] Running: {cfg.get('description', '')} "
              f"(node={alpha_host}, timeout={timeout}s, max_bots={max_bots})")

    return run_scenario_file(
        str(scenario_path),
        alpha_host=alpha_host,
        delta_host=delta_node,
        keys=keys,
        timeout_sec=timeout,
        max_bots=max_bots,
        dry_run=dry_run,
    )


def main() -> int:
    args = parse_args()

    if args.list_suites:
        list_suites()
        return 0

    if not args.suite and not args.scenario:
        print("ERROR: Specify --suite or --scenario", file=sys.stderr)
        return 2

    # Load keys
    if args.keys:
        try:
            keys = load_keys(args.keys)
        except FileNotFoundError:
            print(f"ERROR: Key file not found: {args.keys}", file=sys.stderr)
            return 2
        except Exception as e:
            print(f"ERROR: Failed to load key file: {e}", file=sys.stderr)
            return 2
    else:
        keys = make_stub(args.deploy_id or "test-000")
        if args.verbose:
            print("[warn] No --keys specified; using stub keys (read-only scenarios only)")

    nodes = [n.strip() for n in args.nodes.split(",") if n.strip()]
    if not nodes:
        print("ERROR: No nodes specified", file=sys.stderr)
        return 2

    start = time.monotonic()

    if args.suite:
        # Run a named suite
        result = run_suite(
            suite_id=args.suite,
            nodes=nodes,
            delta_node=args.delta_node,
            keys=keys,
            timeout_override=args.timeout,
            max_bots_override=args.max_bots,
            dry_run=args.dry_run,
            verbose=args.verbose,
        )
        output_path = resolve_output(args, args.suite)
    else:
        # Run a specific scenario file
        scenario_path = args.scenario
        if not os.path.exists(scenario_path):
            print(f"ERROR: Scenario file not found: {scenario_path}", file=sys.stderr)
            return 2

        timeout = args.timeout or 300
        max_bots = args.max_bots or 5
        alpha_host = nodes[0]

        if args.verbose:
            print(f"Running scenario: {scenario_path} (node={alpha_host}, timeout={timeout}s)")

        result = run_scenario_file(
            scenario_path,
            alpha_host=alpha_host,
            delta_host=args.delta_node,
            keys=keys,
            timeout_sec=timeout,
            max_bots=max_bots,
            dry_run=args.dry_run,
        )
        output_path = resolve_output(args, scenario_path)

    elapsed = time.monotonic() - start

    # Print to stdout
    if args.verbose or not output_path:
        print(format_text(result))

    # Write structured output
    write_output(result, output_path, args.format)

    if args.verbose and output_path:
        print(f"\nResult written to: {output_path}")
    if args.verbose:
        print(f"Total elapsed: {elapsed:.1f}s")

    return 0 if result.passed else (2 if result.error else 1)


if __name__ == "__main__":
    sys.exit(main())
