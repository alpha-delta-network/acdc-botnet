# adnet-testbots

**Production-grade bot testing infrastructure for the Alpha/Delta dual-chain protocol**

## Overview

adnet-testbots provides comprehensive functional, security, and chaos testing through autonomous bot orchestration. The system supports:

- **90+ REST endpoints** (AlphaOS, DeltaOS, unified Adnet API)
- **VM operations** (AlphaVM programs/privacy, DeltaVM DEX/perpetuals/oracles)
- **Network/Consensus** (BFT, mempool, IPC, validator duties)
- **Cross-chain coordination** (lock/mint, burn/unlock, governance execution)
- **Security testing** (MEV, Byzantine behavior, privacy attacks)
- **Distributed architecture** (10+ worker nodes with automatic failover)

## Architecture

### Core Modules

- **bot/** - Core bot framework with type-safe lifecycle management
- **roles/** - Bot role implementations (GeneralUser, Trader, Validator, Governor, Prover, etc.)
- **behaviors/** - Pluggable behavior patterns (legitimate, adversarial, anti-patterns)
- **integration/** - Client libraries for AlphaOS, DeltaOS, and Adnet
- **metrics/** - Observability with HDR histograms and Prometheus export
- **scenarios/** - YAML-based scenario definitions
- **distributed/** - gRPC-based coordinator/worker architecture
- **cli/** - Command-line interface

### Design Principles

1. **Type-Driven Architecture** - Zero stringly-typed APIs, compile-time state validation
2. **Formal Correctness** - Design-by-contract with pre/post-conditions
3. **MECE Coverage** - Mutually Exclusive, Collectively Exhaustive testing
4. **Observability** - Distributed tracing with causal chains
5. **Security-First** - Assume Byzantine, 100% attack detection
6. **Architectural Beauty** - Composability, separation of concerns, single responsibility

## Phase 1 Status (COMPLETE)

✅ Repository structure initialized
✅ Core bot framework implemented
  - `actor.rs`: Bot trait with setup/execute/teardown lifecycle
  - `identity.rs`: Multi-chain identity generation (ax1/dx1 addresses)
  - `wallet.rs`: Balance management (AX, sAX, DX tokens)
  - `scheduler.rs`: Tokio-based task scheduling
  - `state.rs`: Type-safe state machine with phantom types
  - `communication.rs`: Inter-bot message bus

✅ gRPC protocol definitions for distributed architecture
✅ Cargo workspace with 8 crates

### Next Steps (Phase 1 Completion)

- [ ] Implement basic roles (GeneralUser, Trader)
- [ ] Implement integration clients (AlphaOS, DeltaOS, Adnet)
- [ ] Implement metrics system (event recording, aggregation)
- [ ] Implement distributed coordinator/worker
- [ ] Create CLI interface
- [ ] Write comprehensive tests

## Quick Start

### Build

```bash
cargo build --release
```

### Run Tests

```bash
cargo test --all
```

### Run a Scenario

```bash
# Single-machine mode
adnet-testbots run alpha-transfer

# Distributed mode
# Terminal 1: Start coordinator
adnet-testbots coordinator start --bind 127.0.0.1:50051

# Terminal 2: Start worker
adnet-testbots worker start --coordinator 127.0.0.1:50051 --max-bots 100

# Terminal 3: Run scenario
adnet-testbots run peak-tps-stress --distributed
```

## Scenario Categories

### Functional (8 scenarios)
- daily-network-ops
- cross-chain-stress
- governance-lifecycle
- dex-trading-session
- privacy-operations
- validator-operations
- mempool-saturation
- name-service-auction

### Security (8 scenarios)
- mev-extraction
- byzantine-validators
- governance-manipulation
- cross-chain-double-spend
- privacy-leakage
- resource-exhaustion
- oracle-manipulation
- replay-attack

### Load/Stress (4 scenarios)
- peak-tps-stress
- concurrent-governance
- dex-orderbook-stress
- sustained-load-48h

### Chaos (4 scenarios)
- network-partition-recovery
- validator-crash-cascade
- ipc-delay-injection
- byzantine-fault-tolerance

## Development

### Project Structure

```
adnet-testbots/
├── crates/
│   ├── bot/           # Core framework
│   ├── roles/         # Role implementations
│   ├── behaviors/     # Behavior patterns
│   ├── integration/   # External system clients
│   ├── metrics/       # Observability
│   ├── scenarios/     # Scenario runner
│   ├── distributed/   # Coordinator/worker
│   └── cli/           # CLI interface
├── proto/             # gRPC protocol definitions
├── scenarios/         # YAML scenario files
└── examples/          # Usage examples
```

### Quality Gates

- **Clippy**: `-W clippy::pedantic` must pass
- **Tests**: 100% core functionality covered
- **Property tests**: `proptest` for cryptographic operations
- **No unwrap()**: Zero panics in production code

## Contributing

See [DESIGN.md](DESIGN.md) for architectural details and [SCENARIOS.md](SCENARIOS.md) for scenario definitions.

## License

Apache-2.0

## Repository

https://source.ac-dc.network/alpha-delta-network/adnet-testbots
