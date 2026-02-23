# AdNet Testbots - Design Document

## Executive Summary

AdNet Testbots is a production-grade bot testing infrastructure for the Alpha/Delta dual-chain protocol. It provides comprehensive functional, security, and chaos testing through autonomous bot orchestration with formal correctness guarantees.

**Key Features:**
- Type-driven architecture with compile-time guarantees
- Distributed execution across 10+ worker nodes
- HDR histogram for accurate latency measurements
- 24 pre-built scenarios covering 90+ REST endpoints
- Prometheus metrics export for real-time monitoring
- MECE (Mutually Exclusive, Collectively Exhaustive) coverage

## Design Philosophy

### Maximum Rigor & Architectural Elegance

Every design decision optimizes for **correctness, elegance, and comprehensiveness** over implementation velocity.

**Guiding Principles:**

1. **Type-Driven Architecture**
   - Zero stringly-typed APIs
   - State machines encoded in types (phantom types)
   - Compile-time guarantees for invalid states
   - Zero-cost abstractions

2. **Formal Correctness**
   - Design-by-contract (pre/post-conditions)
   - Property-based testing with `proptest`
   - HDR histogram for statistical accuracy
   - Zero `unwrap()`/`panic!()` in production code

3. **MECE Coverage**
   - 100% endpoint coverage (90+ REST endpoints)
   - No overlapping tests
   - No gaps in functionality
   - Quantified risk assessment

4. **Observability & Causality**
   - Distributed tracing with causal chains
   - HDR histogram (not naive percentiles)
   - Anomaly detection (3-sigma + MAD)
   - Deterministic replay from seed

5. **Security-First**
   - Assume Byzantine validators
   - 100% attack detection in controlled scenarios
   - Formal threat models
   - CVE/paper citations for attack patterns

6. **Architectural Beauty**
   - Composability: scenarios → behaviors → operations
   - Separation of concerns
   - Dependency inversion
   - Single responsibility principle

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Coordinator (Command & Control)                        │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │  Scenario   │  │   Metrics    │  │   Worker      │ │
│  │  Orchestrator│──│  Aggregator  │──│   Registry    │ │
│  └─────────────┘  └──────────────┘  └───────────────┘ │
│         │                 │                   │         │
└─────────┼─────────────────┼───────────────────┼─────────┘
          │                 │                   │
          │ gRPC (tonic/prost)                 │
          ├─────────────────┼───────────────────┤
          │                 │                   │
     ┌────▼────┐       ┌────▼────┐        ┌────▼────┐
     │ Worker 1│       │ Worker 2│        │ Worker N│
     │ (GPU)   │       │ (CPU)   │        │ (CPU)   │
     ├─────────┤       ├─────────┤        ├─────────┤
     │ Bot Pool│       │ Bot Pool│        │ Bot Pool│
     │  - 50   │       │  - 200  │        │  - 200  │
     └────┬────┘       └────┬────┘        └────┬────┘
          │                 │                   │
          └─────────────────┼───────────────────┘
                            │
                    ┌───────▼────────┐
                    │  Alpha/Delta   │
                    │   Protocol     │
                    │ (Port 3030/3031)│
                    └────────────────┘
```

## Module Structure

### 1. Bot Framework (`crates/bot/`)

**Purpose:** Core bot abstraction and lifecycle management.

```rust
bot/
├── actor.rs          // Bot trait (setup, execute, teardown)
├── identity.rs       // Multi-chain identity (ax1/dx1)
├── wallet.rs         // Balance tracking, signing
├── scheduler.rs      // Tokio task scheduling
├── state.rs          // Type-safe state machine
├── communication.rs  // Inter-bot messaging
└── mod.rs
```

**Key Design:**
- `Bot` trait with async lifecycle
- Identity generation from seed (deterministic)
- Ed25519 signing for both chains
- State machine uses phantom types for compile-time validation

**Type Safety Example:**
```rust
pub struct StateMachine<S> {
    current: BotState,
    _marker: PhantomData<S>,
}

// States
pub struct Idle;
pub struct Running;
pub struct Stopped;

impl StateMachine<Idle> {
    pub fn start(self) -> StateMachine<Running> { /* ... */ }
}

impl StateMachine<Running> {
    pub fn stop(self) -> StateMachine<Stopped> { /* ... */ }
    // Cannot call start() - compile error!
}
```

### 2. Role Implementations (`crates/roles/`)

**Purpose:** Define bot capabilities and behaviors by role.

**9 Role Types:**
1. **GeneralUser** (casual, power, whale)
2. **Trader** (spot, perpetual, arbitrageur, market_maker, mev_searcher)
3. **Validator** (honest, lazy, byzantine, shadow)
4. **Governor** (active, passive, malicious, abstainer)
5. **Prover** (fast, standard, slow, shadow)
6. **LiquidityProvider** (pool_manager, lp_staker)
7. **Treasury** (buyback_bot, reward_distributor)
8. **Fleet** (coordinator, spawner, lifecycle)
9. **Orchestrator** (scenario_runner, metrics_collector)

**Role-Based Affinity:**
- Prover bots → GPU workers
- Validator bots → high-uptime workers
- Byzantine bots → isolated workers

### 3. Behavior System (`crates/behaviors/`)

**Purpose:** Pluggable behaviors with setup/execute/teardown.

```rust
#[async_trait]
pub trait Behavior: Send + Sync {
    fn id(&self) -> &str;
    fn category(&self) -> BehaviorCategory;
    async fn setup(&mut self, context: &BehaviorContext) -> Result<()>;
    async fn execute(&mut self, context: &BehaviorContext) -> Result<BehaviorResult>;
    async fn teardown(&mut self, context: &BehaviorContext) -> Result<()>;
}
```

**3 Behavior Categories:**
1. **Legitimate** (20+ patterns)
   - Governance workflows
   - Cross-chain flows
   - Trading lifecycles
   - Privacy operations

2. **Adversarial** (35+ patterns)
   - Governance manipulation (Sybil, vote buying)
   - Cross-chain exploits (double-spend, replay)
   - MEV extraction (sandwich, front-running)
   - Byzantine behavior (equivocation, censorship)

3. **Anti-Patterns** (15+ patterns)
   - Validation errors
   - State assumptions
   - Timing errors
   - Type confusion

### 4. Scenario Framework (`crates/scenario/`)

**Purpose:** Orchestrate multi-bot coordinated testing.

**5 Scenario Types:**
1. **Unit** (single bot, single operation, 1-10s)
2. **Integration** (multi-bot, coordinated, 1-10min)
3. **Load** (high-volume stress, 10-60min, 100-1000 bots)
4. **Chaos** (fault injection + recovery, 5-30min)
5. **Adversarial** (attack simulations, 5-30min)

**Scenario Lifecycle:**
```yaml
scenario:
  metadata: { id, name, type, duration }
  setup: { network, bots }
  phases:
    - name: "Phase 1"
      duration: "5 min"
      bots: "selector"
      behavior: "behavior_id"
      concurrent: [...]
      assertions: [...]
      metrics: [...]
  success_criteria: [...]
```

### 5. Integration Layer (`crates/integration/`)

**Purpose:** Interface with Alpha/Delta protocol.

```rust
integration/
├── adnet_client.rs      // CLI command execution
├── alphaos_client.rs    // AlphaOS REST (port 3030)
├── deltaos_client.rs    // DeltaOS REST (port 3031)
└── test_harness.rs      // Integration with adnet-e2e
```

**Client Features:**
- HTTP client with retry logic
- Request/response serialization
- Error handling and recovery
- Connection pooling

### 6. Metrics & Observability (`crates/metrics/`)

**Purpose:** Real-time metrics with statistical accuracy.

**Key Components:**
1. **Event Recorder** - Append-only log
2. **Aggregator** - HDR histogram for latency
3. **Prometheus Exporter** - `/metrics` endpoint

**HDR Histogram Benefits:**
- Accurate percentiles (p50, p95, p99)
- No bucket discretization errors
- Bounded memory (60s max latency)
- 3 significant digits precision

**Metrics Collected:**
- TPS (transactions per second)
- Latency distribution (HDR)
- Error rates
- Bot counts by role
- Behavior success rates
- Worker distribution

### 7. Distributed Architecture (`crates/distributed/`)

**Purpose:** Scale to 10+ worker nodes with fault tolerance.

**Components:**

**Coordinator:**
```rust
pub trait Coordinator {
    async fn register_worker(&mut self, worker_id, capacity) -> Result<()>;
    async fn spawn_bot(&mut self, bot_spec) -> Result<BotHandle>;
    async fn distribute_scenario(&mut self, scenario) -> Result<Vec<BotHandle>>;
    async fn collect_metrics(&self) -> Result<AggregatedMetrics>;
}
```

**Worker:**
```rust
pub trait Worker {
    async fn connect_coordinator(&mut self, addr) -> Result<()>;
    async fn spawn_local_bot(&mut self, spec) -> Result<LocalBotHandle>;
    async fn stream_metrics(&self, tx) -> Result<()>;
    async fn health_heartbeat(&self, interval) -> Result<()>;
}
```

**gRPC Protocol:**
```protobuf
service BotOrchestration {
    rpc RegisterWorker(WorkerInfo) returns (WorkerAck);
    rpc SpawnBot(BotSpec) returns (BotHandle);
    rpc StopBot(BotId) returns (StopAck);
    rpc StreamMetrics(stream WorkerMetrics) returns (Empty);
    rpc Heartbeat(WorkerHealth) returns (CoordinatorDirective);
}
```

**Fault Tolerance:**
- Worker failure detection (3 missed heartbeats = 15s)
- Bot migration to healthy workers
- Metrics buffering (60s local)
- Coordinator state checkpointing

**Bot Distribution Strategy:**
1. Capacity-aware scheduling (workers report max_bots)
2. Role-based affinity (GPU for provers)
3. Load balancing (even distribution)
4. Fault isolation (Byzantine → dedicated workers)

## Type Safety & Correctness

### Phantom Types for State Validation

```rust
pub struct Bot<S> {
    identity: Identity,
    state: BotState,
    _marker: PhantomData<S>,
}

// States
pub struct Initialized;
pub struct Running;
pub struct Stopped;

impl Bot<Initialized> {
    pub fn start(self) -> Bot<Running> {
        // Transition allowed
    }
    // Cannot call stop() - compile error!
}

impl Bot<Running> {
    pub fn stop(self) -> Bot<Stopped> {
        // Transition allowed
    }
    // Cannot call start() again - compile error!
}
```

### Zero Unwrap/Panic

All fallible operations return `Result`:
```rust
pub async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
    let behavior = self.behaviors.get(behavior_id)
        .ok_or_else(|| anyhow!("Behavior not found: {}", behavior_id))?;

    behavior.execute(&self.context).await
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_identity_generation_deterministic(seed in any::<[u8; 32]>()) {
        let id1 = Identity::from_seed(seed)?;
        let id2 = Identity::from_seed(seed)?;

        prop_assert_eq!(id1.alpha_address, id2.alpha_address);
        prop_assert_eq!(id1.delta_address, id2.delta_address);
    }
}
```

## Performance Considerations

### Async/Await with Tokio

All I/O operations use Tokio for efficient concurrency:
```rust
#[tokio::main]
async fn main() {
    let mut tasks = vec![];

    for bot_spec in scenario.bots {
        let task = tokio::spawn(async move {
            bot.execute(bot_spec).await
        });
        tasks.push(task);
    }

    // Await all tasks concurrently
    let results = futures::future::join_all(tasks).await;
}
```

### HDR Histogram Performance

- O(1) record operation
- O(log n) percentile query
- Bounded memory (configurable)
- Thread-safe with `parking_lot::RwLock`

### Connection Pooling

HTTP clients use connection pooling:
```rust
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(10)
    .timeout(Duration::from_secs(30))
    .build()?;
```

## Security Considerations

### Byzantine Fault Tolerance

**Threat Model:**
- Up to f < n/3 Byzantine validators
- Arbitrary behavior (equivocation, censorship, invalid blocks)
- Coordination between Byzantine nodes

**Detection Mechanisms:**
1. Signature verification (Ed25519)
2. Equivocation detection (conflicting votes)
3. Censorship monitoring (missing attestations)
4. Invalid block rejection (state root mismatch)

### Attack Surface

**Minimized Attack Surface:**
- No dynamic code execution
- No unsafe Rust
- All inputs validated
- Rate limiting on all endpoints

### Formal Threat Models

Each adversarial pattern has:
- Attacker capabilities (computational, network, stake)
- Attack success criteria
- Detection requirements
- Mitigation strategy

## Testing Strategy

### Unit Tests

Test individual components in isolation:
```bash
cargo test --lib --all-features
```

### Integration Tests

Test multi-component interactions:
```bash
cargo test --test '*' --all-features
```

### Property-Based Tests

Test invariants with randomized inputs:
```rust
proptest! {
    #[test]
    fn test_cross_chain_atomicity(amount in 1u64..1_000_000) {
        let lock_result = alpha.lock(amount)?;
        let mint_result = delta.mint(amount)?;

        prop_assert_eq!(lock_result.amount, mint_result.amount);
    }
}
```

### Scenario Tests

End-to-end tests with full bot orchestration:
```bash
adnet-testbots run cross-chain-stress --duration 5m
```

## Deterministic Replay

All scenarios are reproducible from seed:
```rust
let mut rng = StdRng::from_seed(scenario.seed);
let bot_id = rng.gen::<u64>();
let behavior_delay = rng.gen_range(0..1000);
```

**Benefits:**
- Bug reproduction
- Performance comparison
- Chaos engineering consistency

## Future Enhancements

### Phase 6: Formal Verification

- TLA+ specifications for consensus
- Coq proofs for cryptographic operations
- Model checking for state machines

### Phase 7: Multi-Chain Support

- Generalize beyond Alpha/Delta
- Plugin architecture for new chains
- Cross-chain messaging protocol

### Phase 8: AI-Driven Testing

- Reinforcement learning for attack discovery
- Genetic algorithms for scenario generation
- Anomaly detection with ML

## References

- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [HDR Histogram](http://hdrhistogram.org/)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Byzantine Fault Tolerance](https://en.wikipedia.org/wiki/Byzantine_fault)
- [Property-Based Testing](https://hypothesis.works/articles/what-is-property-based-testing/)
