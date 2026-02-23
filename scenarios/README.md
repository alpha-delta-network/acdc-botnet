# AdNet Testbots - Scenario Definitions

This directory contains 24 pre-built scenario jobs for comprehensive testing of the Alpha/Delta protocol.

## Quick Reference

```bash
# Run any scenario by name
adnet-testbots run <scenario-name>

# Examples
adnet-testbots run daily-network-ops
adnet-testbots run mev-extraction
adnet-testbots run peak-tps-stress
adnet-testbots run network-partition-recovery
```

## Scenario Categories

### Functional Scenarios (8)

Testing normal protocol functionality and user workflows.

| ID | Scenario | Duration | Bots | Description |
|----|----------|----------|------|-------------|
| FUNC-001 | `daily-network-ops` | 24h | 500 | Simulates typical network activity across all operations |
| FUNC-002 | `cross-chain-stress` | 30m | 800 | Heavy lock/mint and burn/unlock operations (100 locks/sec, 100 burns/sec) |
| FUNC-003 | `governance-lifecycle` | 2h | 150 | Complete proposal → vote → timelock → execute flow |
| FUNC-004 | `dex-trading-session` | 1h | 600 | High-frequency spot + perpetuals trading (500 orders/sec) |
| FUNC-005 | `privacy-operations` | 45m | 400 | Shielded transfers, mixing, address recycling (200 transfers/min) |
| FUNC-006 | `validator-operations` | 4h | 120 | Validator lifecycle: registration, block production, rewards, ejection |
| FUNC-007 | `mempool-saturation` | 15m | 1000 | Fill mempool to 10K capacity (2000 tx/sec inbound) |
| FUNC-008 | `name-service-auction` | 2h | 200 | Vickrey auction testing (commit, reveal, claim phases) |

**File locations:** `scenarios/functional/*.yaml`

### Security Scenarios (8)

Testing attack vectors, Byzantine behavior, and security measures.

| ID | Scenario | Duration | Bots | Description |
|----|----------|----------|------|-------------|
| SEC-001 | `mev-extraction` | 30m | 500 | Front-running, sandwich attacks, arbitrage detection |
| SEC-002 | `byzantine-validators` | 1h | 100 | Double-signing, equivocation, attestation withholding (25% Byzantine) |
| SEC-003 | `governance-manipulation` | 2h | 300 | Sybil wallets, vote buying, proposal spam, DoS attempts |
| SEC-004 | `cross-chain-double-spend` | 30m | 300 | Concurrent locks, replay attacks, race conditions |
| SEC-005 | `privacy-leakage` | 1h | 350 | Timing correlation, amount matching, address clustering |
| SEC-006 | `resource-exhaustion` | 30m | 1200 | Mempool spam, storage bombs, API flooding (5000 spam/sec) |
| SEC-007 | `oracle-manipulation` | 45m | 200 | Price feed manipulation, flash loan attacks, outlier injection |
| SEC-008 | `replay-attack` | 30m | 300 | Transaction/attestation replay via nonce reuse, timestamp manipulation |

**File locations:** `scenarios/security/*.yaml`

### Load/Stress Scenarios (4)

Testing throughput limits, concurrent operations, and sustained load.

| ID | Scenario | Duration | Bots | Description |
|----|----------|----------|------|-------------|
| LOAD-001 | `peak-tps-stress` | 15m | 1000 | Maximum throughput test targeting 10,000+ TPS (ramp-up over 15min) |
| LOAD-002 | `concurrent-governance` | 2h | 200 | 100 simultaneous proposals with voting |
| LOAD-003 | `dex-orderbook-stress` | 30m | 800 | 10,000 active orders, 100 price levels (500 orders/sec) |
| LOAD-004 | `sustained-load-48h` | 48h | 300 | Long-duration stability test (500 TPS avg, 1000 cross-chain ops/hour) |

**File locations:** `scenarios/load/*.yaml`

### Chaos Scenarios (4)

Testing fault tolerance, partition recovery, and Byzantine fault tolerance.

| ID | Scenario | Duration | Bots | Description |
|----|----------|----------|------|-------------|
| CHAOS-001 | `network-partition-recovery` | 1h | 150 | Split 40/40 validators, test state consistency + fork resolution |
| CHAOS-002 | `validator-crash-cascade` | 1h | 100 | Sequential crashes up to 40/80 validators (50% down), recovery testing |
| CHAOS-003 | `ipc-delay-injection` | 45m | 200 | Inject 100ms → 30s latency into cross-chain messaging |
| CHAOS-004 | `byzantine-fault-tolerance` | 2h | 120 | Comprehensive BFT verification with 10% → 33% Byzantine nodes |

**File locations:** `scenarios/chaos/*.yaml`

## CLI Usage

### Basic Execution

```bash
# Run single scenario
adnet-testbots run daily-network-ops

# Run with custom duration
adnet-testbots run peak-tps-stress --duration 30m

# Run in verbose mode
adnet-testbots run mev-extraction --verbose

# Dry run (validate without executing)
adnet-testbots run governance-lifecycle --dry-run
```

### Distributed Execution

```bash
# Run on distributed workers
adnet-testbots run peak-tps-stress --distributed --workers 5

# Specify worker capabilities
adnet-testbots run privacy-operations --distributed --require-gpu
```

### Batch Execution

```bash
# Run category
adnet-testbots run-category functional --duration-limit 1h

# Run all scenarios
adnet-testbots run-all --concurrency 4

# Run CI-critical scenarios
adnet-testbots run-ci-suite
```

### Output Formats

```bash
# Ultra-compact output (default)
adnet-testbots run daily-network-ops
# Output: ✓ daily-network-ops: 500 bots, 300 TPS avg, 0.3% errors (PASSED)

# Detailed output
adnet-testbots run daily-network-ops --format detailed

# JSON output
adnet-testbots run daily-network-ops --format json > results.json

# Export metrics
adnet-testbots run daily-network-ops --export-metrics metrics.json
```

## Scenario Structure

All scenarios follow this YAML structure:

```yaml
scenario:
  metadata:
    id: "CATEGORY-NNN"
    name: "Human-Readable Name"
    type: functional|security|load|chaos
    duration: "time spec"

  description: "Brief description"

  setup:
    network:
      alpha_rest: "http://localhost:3030"
      delta_rest: "http://localhost:3031"
    bots:
      - id: "bot-{1-N}"
        role: "role_type"
        variant: "variant"
        count: N
        wallet_ax: amount
        wallet_dx: amount

  phases:
    - name: "Phase name"
      duration: "time spec"
      bots: "bot-selector"
      behavior: "behavior_name"
      params:
        key: value
      concurrent:
        - bots: "other-bots"
          behavior: "other_behavior"
      assertions:
        - condition: value
      metrics:
        - metric_name: "unit"

  metrics:
    - metric_name: "description"

  success_criteria:
    - criterion: value
```

## Success Criteria

Each scenario includes comprehensive success criteria:

- **Functional:** 100% operation success, expected performance metrics
- **Security:** Attack detection >80%, mitigation effectiveness, zero unauthorized actions
- **Load:** Sustained target TPS, acceptable degradation, no crashes
- **Chaos:** Fault tolerance verified, recovery successful, state consistency maintained

## Metrics Collected

All scenarios collect:
- **Throughput:** TPS, operations/sec, cross-chain ops/hour
- **Latency:** p50, p95, p99 (HDR histogram)
- **Success rates:** % successful operations
- **Resource usage:** CPU, memory, storage, network
- **Security:** Detection rates, attack successes, rejections
- **Consensus:** Block production rate, finality time, validator uptime

## Custom Scenarios

Create custom scenarios by copying a template:

```bash
cp scenarios/functional/daily_network_ops.yaml scenarios/custom/my_scenario.yaml
# Edit my_scenario.yaml
adnet-testbots run my-scenario --config scenarios/custom/my_scenario.yaml
```

## CI Integration

Critical scenarios run on every PR:

```yaml
# .woodpecker.yml
- name: run-critical-scenarios
  commands:
    - adnet-testbots run daily-network-ops --duration 10m
    - adnet-testbots run cross-chain-stress --duration 5m
    - adnet-testbots run mev-extraction --duration 5m
    - adnet-testbots run byzantine-validators --duration 10m
    - adnet-testbots run peak-tps-stress --duration 5m
```

## Performance Targets

| Metric | Target | Critical Threshold |
|--------|--------|-------------------|
| Peak TPS | 10,000+ | 8,000+ |
| Sustained TPS (48h) | 500 | 400+ |
| p95 Latency | <5s | <10s |
| Cross-chain p95 | <30s | <60s |
| MEV Detection | >80% | >60% |
| Byzantine Detection | >90% | >70% |
| Validator Uptime | >99% | >95% |
| Error Rate | <0.5% | <1% |

## Development

### Adding New Scenarios

1. Create YAML file in appropriate category directory
2. Follow the standard structure
3. Add entry to this README
4. Test with `--dry-run` first
5. Verify metrics collection
6. Add to CI if critical

### Debugging Scenarios

```bash
# Verbose logging
adnet-testbots run my-scenario --verbose --log-level debug

# Enable distributed tracing
adnet-testbots run my-scenario --trace

# Dump intermediate state
adnet-testbots run my-scenario --checkpoint-interval 5m
```

## References

- **Design Document:** `/docs/DESIGN.md`
- **API Documentation:** `/docs/API.md`
- **Behavior Catalog:** `/crates/behaviors/README.md`
- **Role Definitions:** `/crates/roles/README.md`
