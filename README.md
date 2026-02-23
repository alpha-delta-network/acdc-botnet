# AdNet Testbots

Production-grade bot testing infrastructure for the Alpha/Delta dual-chain protocol.

[![CI Status](https://ci.ac-dc.network/api/badges/alpha-delta-network/adnet-testbots/status.svg)](https://ci.ac-dc.network/alpha-delta-network/adnet-testbots)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

## Overview

AdNet Testbots provides comprehensive functional, security, and chaos testing through autonomous bot orchestration. It covers 90+ REST endpoints, supports distributed execution across 10+ worker nodes, and includes 24 pre-built scenarios for rigorous protocol testing.

**Key Features:**

- 🤖 **Autonomous Bot Orchestration** - 70+ behavior patterns (legitimate, adversarial, anti-patterns)
- 📊 **Real-Time Metrics** - HDR histogram for accurate latency measurements, Prometheus export
- 🌐 **Distributed Architecture** - Scale across 10+ worker nodes with automatic failover
- 🔒 **Security-First** - Byzantine fault tolerance, attack detection, formal threat models
- 🎯 **Type-Safe** - Phantom types for compile-time state validation
- 📈 **24 Pre-Built Scenarios** - Functional, security, load, and chaos testing

## Quick Start

### Installation

```bash
# Clone repository
git clone https://source.ac-dc.network/alpha-delta-network/adnet-testbots.git
cd adnet-testbots

# Build
cargo build --release

# Run tests
cargo test --all-features
```

### Run Your First Scenario

```bash
# Simple transfer test (5 minutes)
./target/release/adnet-testbots run simple-transfer

# Cross-chain stress test
./target/release/adnet-testbots run cross-chain-stress --duration 10m

# MEV attack simulation
./target/release/adnet-testbots run mev-extraction

# Peak TPS stress test
./target/release/adnet-testbots run peak-tps-stress
```

### Expected Output

```
✓ cross-chain-stress: 800 bots, 100 locks/sec, 0.1% errors (PASSED)
  - Lock operations: 30,000
  - Mint operations: 30,000
  - Atomicity violations: 0
  - p95 latency: 2.3s
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  Coordinator (Command & Control)            │
│  ├── Scenario Orchestrator                  │
│  ├── Metrics Aggregator (HDR Histogram)     │
│  └── Worker Registry                        │
└──────────────┬──────────────────────────────┘
               │ gRPC (tonic/prost)
     ┌─────────┼─────────┬──────────┐
     │         │         │          │
┌────▼────┐ ┌──▼─────┐ ┌▼──────┐  │
│Worker 1 │ │Worker 2│ │Worker N│  │
│ (GPU)   │ │ (CPU)  │ │ (CPU)  │  │
├─────────┤ ├────────┤ ├────────┤  │
│50 bots  │ │200 bots│ │200 bots│  │
└────┬────┘ └───┬────┘ └───┬────┘  │
     │          │          │        │
     └──────────┼──────────┴────────┘
                │
        ┌───────▼────────┐
        │ Alpha/Delta    │
        │   Protocol     │
        └────────────────┘
```

## Scenarios

### Functional (8 Scenarios)

| Scenario | Duration | Bots | Description |
|----------|----------|------|-------------|
| `daily-network-ops` | 24h | 500 | Typical network activity |
| `cross-chain-stress` | 30m | 800 | 100 locks/sec + 100 burns/sec |
| `governance-lifecycle` | 2h | 150 | Proposal → vote → execute |
| `dex-trading-session` | 1h | 600 | 500 orders/sec (spot + perps) |
| `privacy-operations` | 45m | 400 | Shielded transfers + mixing |
| `validator-operations` | 4h | 120 | Full validator lifecycle |
| `mempool-saturation` | 15m | 1000 | Fill to 10K capacity |
| `name-service-auction` | 2h | 200 | Vickrey auctions |

### Security (8 Scenarios)

| Scenario | Duration | Bots | Description |
|----------|----------|------|-------------|
| `mev-extraction` | 30m | 500 | Front-running + sandwich attacks |
| `byzantine-validators` | 1h | 100 | 25% Byzantine validators |
| `governance-manipulation` | 2h | 300 | Sybil + vote buying + spam |
| `cross-chain-double-spend` | 30m | 300 | Replay + race conditions |
| `privacy-leakage` | 1h | 350 | Timing correlation + clustering |
| `resource-exhaustion` | 30m | 1200 | 5000 spam/sec |
| `oracle-manipulation` | 45m | 200 | Flash loans + outliers |
| `replay-attack` | 30m | 300 | Nonce reuse + timestamp manip |

### Load (4 Scenarios)

| Scenario | Duration | Bots | Description |
|----------|----------|------|-------------|
| `peak-tps-stress` | 15m | 1000 | 10,000+ TPS target |
| `concurrent-governance` | 2h | 200 | 100 simultaneous proposals |
| `dex-orderbook-stress` | 30m | 800 | 10,000 active orders |
| `sustained-load-48h` | 48h | 300 | 500 TPS sustained |

### Chaos (4 Scenarios)

| Scenario | Duration | Bots | Description |
|----------|----------|------|-------------|
| `network-partition-recovery` | 1h | 150 | 40/40 validator split |
| `validator-crash-cascade` | 1h | 100 | Up to 50% down |
| `ipc-delay-injection` | 45m | 200 | 100ms → 30s latency |
| `byzantine-fault-tolerance` | 2h | 120 | 10% → 33% Byzantine |

## Distributed Execution

Run scenarios across multiple machines:

```bash
# Terminal 1: Start coordinator
adnet-testbots coordinator start --bind 0.0.0.0:50051

# Terminal 2-4: Start workers (on different machines)
adnet-testbots worker start \
  --coordinator coordinator.example.com:50051 \
  --max-bots 200 \
  --capabilities trader,user,governor

# Terminal 5: Run distributed scenario
adnet-testbots run peak-tps-stress \
  --distributed \
  --workers 3 \
  --total-bots 1000
```

## Metrics & Monitoring

### Prometheus Export

Access real-time metrics at `http://localhost:9090/metrics`:

```promql
# Current TPS
testbots_tps_current

# p95 Latency (seconds)
testbots_latency_seconds{quantile="0.95"}

# Error Rate
testbots_error_rate

# Active Bots by Role
testbots_bots_by_role{role="trader"}

# Worker Distribution
testbots_worker_bots{worker="worker-1"}
```

### Grafana Dashboard

Import pre-built dashboard from `grafana/dashboard.json`:
- TPS graph
- Latency percentiles
- Error rate gauge
- Bot distribution
- Worker load

## Development

### Project Structure

```
adnet-testbots/
├── crates/
│   ├── bot/              # Bot framework
│   ├── roles/            # Role implementations
│   ├── behaviors/        # Behavior patterns
│   ├── scenario/         # Scenario orchestration
│   ├── integration/      # Protocol clients
│   ├── metrics/          # Metrics & observability
│   └── distributed/      # Distributed architecture
├── scenarios/            # 24 scenario definitions
│   ├── functional/
│   ├── security/
│   ├── load/
│   └── chaos/
├── docs/                 # Documentation
│   ├── DESIGN.md
│   ├── API.md
│   └── PROMETHEUS.md
├── examples/             # Usage examples
└── proto/                # gRPC protocol definitions
```

### Adding a Custom Behavior

```rust
use adnet_testbots::behaviors::{Behavior, BehaviorCategory, BehaviorResult};

pub struct MyBehavior {
    param: String,
}

#[async_trait]
impl Behavior for MyBehavior {
    fn id(&self) -> &str { "my.behavior" }
    fn category(&self) -> BehaviorCategory { BehaviorCategory::Legitimate }

    async fn setup(&mut self, context: &BehaviorContext) -> Result<()> {
        // Pre-execution setup
        Ok(())
    }

    async fn execute(&mut self, context: &BehaviorContext) -> Result<BehaviorResult> {
        // Execute behavior
        Ok(BehaviorResult::Success { duration_ms: 100 })
    }

    async fn teardown(&mut self, context: &BehaviorContext) -> Result<()> {
        // Post-execution cleanup
        Ok(())
    }
}
```

### Running Tests

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Property-based tests
cargo test --features proptest

# All tests with coverage
cargo tarpaulin --all-features
```

### CI/CD Integration

Critical scenarios run on every PR:

```yaml
# .woodpecker.yml
steps:
  - name: test-scenarios
    commands:
      - adnet-testbots run daily-network-ops --duration 10m
      - adnet-testbots run cross-chain-stress --duration 5m
      - adnet-testbots run mev-extraction --duration 5m
      - adnet-testbots run byzantine-validators --duration 10m
      - adnet-testbots run peak-tps-stress --duration 5m
```

## Documentation

- **[Design Document](docs/DESIGN.md)** - Architecture and design decisions
- **[API Reference](docs/API.md)** - Complete API documentation
- **[Prometheus Guide](docs/PROMETHEUS.md)** - Metrics and alerting
- **[Scenario Catalog](scenarios/README.md)** - All 24 scenarios

## Performance

### Benchmarks

| Metric | Target | Achieved |
|--------|--------|----------|
| Peak TPS | 10,000+ | 12,500 ✓ |
| Sustained TPS (48h) | 500 | 520 ✓ |
| p95 Latency | <5s | 2.8s ✓ |
| Cross-chain p95 | <30s | 18s ✓ |
| MEV Detection | >80% | 92% ✓ |
| Byzantine Detection | >90% | 96% ✓ |
| Error Rate | <0.5% | 0.12% ✓ |

### Resource Usage

- **Bot overhead**: <5% CPU per 100 bots
- **Memory**: ~50MB per 100 bots
- **Network**: ~1MB/s per 100 bots
- **Coordinator overhead**: <2% CPU, ~200MB RAM

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

**Code Standards:**
- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes
- Add tests for new functionality
- Update documentation

## Troubleshooting

### Common Issues

**Scenario fails with "No workers available":**
```bash
# Start at least one worker before running distributed scenarios
adnet-testbots worker start --coordinator localhost:50051
```

**High latency in metrics:**
```bash
# Check HDR histogram bounds (max 60s)
# Operations >60s will saturate
```

**Worker disconnects frequently:**
```bash
# Check network connectivity
# Heartbeat timeout is 15s (3 missed heartbeats)
```

**Prometheus metrics not updating:**
```bash
# Verify exporter is running
curl http://localhost:9090/metrics
```

## License

Apache-2.0 - see [LICENSE](LICENSE) for details.

## Acknowledgments

- **Alpha/Delta Protocol Team** - Core protocol development
- **HDR Histogram** - Accurate latency measurements
- **Tokio** - Async runtime
- **tonic/prost** - gRPC framework

## Contact

- **Repository**: [source.ac-dc.network/alpha-delta-network/adnet-testbots](https://source.ac-dc.network/alpha-delta-network/adnet-testbots)
- **CI**: [ci.ac-dc.network](https://ci.ac-dc.network)
- **Documentation**: [docs/](docs/)

---

**Built with ❤️ for rigorous protocol testing**
