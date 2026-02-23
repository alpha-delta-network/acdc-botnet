# AdNet Testbots - API Documentation

Complete API reference for bot framework, behaviors, scenarios, and distributed execution.

## Table of Contents

- [Bot Framework API](#bot-framework-api)
- [Behavior API](#behavior-api)
- [Scenario API](#scenario-api)
- [Metrics API](#metrics-api)
- [Distributed API](#distributed-api)
- [Integration Clients](#integration-clients)

---

## Bot Framework API

### Bot Trait

Core trait for all bot implementations.

```rust
#[async_trait]
pub trait Bot: Send + Sync {
    /// Setup bot (initialize connections, load config)
    async fn setup(&mut self, context: &BotContext) -> Result<()>;

    /// Execute a specific behavior
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult>;

    /// Teardown bot (cleanup resources)
    async fn teardown(&mut self) -> Result<()>;

    /// Get bot ID
    fn id(&self) -> &str;

    /// Get bot role
    fn role(&self) -> &str;
}
```

**Usage Example:**

```rust
use adnet_testbots::bot::{Bot, BotContext};

struct MyBot {
    id: String,
    role: String,
}

#[async_trait]
impl Bot for MyBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        // Initialize connections
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        // Execute behavior
        Ok(BehaviorResult::Success { duration_ms: 100 })
    }

    async fn teardown(&mut self) -> Result<()> {
        // Cleanup
        Ok(())
    }

    fn id(&self) -> &str { &self.id }
    fn role(&self) -> &str { &self.role }
}
```

### Identity

Multi-chain identity with Ed25519 keys.

```rust
pub struct Identity {
    pub id: String,
    pub alpha_address: String,  // ax1...
    pub delta_address: String,  // dx1...
}

impl Identity {
    /// Create identity from seed (deterministic)
    pub fn from_seed(seed: [u8; 32]) -> Result<Self>;

    /// Create identity from random seed
    pub fn random() -> Result<Self>;

    /// Sign message for Alpha chain
    pub fn sign_alpha(&self, message: &[u8]) -> Result<Signature>;

    /// Sign message for Delta chain
    pub fn sign_delta(&self, message: &[u8]) -> Result<Signature>;

    /// Verify signature
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool;
}
```

**Usage Example:**

```rust
use adnet_testbots::bot::Identity;

// Deterministic identity
let seed = [0u8; 32];
let identity = Identity::from_seed(seed)?;
println!("Alpha: {}", identity.alpha_address);
println!("Delta: {}", identity.delta_address);

// Random identity
let identity = Identity::random()?;

// Sign transaction
let tx_data = b"transfer 1000 to ax1...";
let signature = identity.sign_alpha(tx_data)?;
assert!(identity.verify(tx_data, &signature));
```

### Wallet

Track balances and sign transactions.

```rust
pub struct Wallet {
    pub ax_balance: u128,
    pub dx_balance: u128,
    pub sax_balance: u128,  // Synthetic AX on Delta
}

impl Wallet {
    pub fn new(ax_balance: u128, dx_balance: u128) -> Self;
    pub fn debit_ax(&mut self, amount: u128) -> Result<()>;
    pub fn credit_ax(&mut self, amount: u128);
    pub fn debit_dx(&mut self, amount: u128) -> Result<()>;
    pub fn credit_dx(&mut self, amount: u128);
}
```

### BotContext

Context passed to behaviors.

```rust
pub struct BotContext {
    pub bot_id: String,
    pub role: String,
    pub identity: Identity,
    pub wallet: Wallet,
    pub config: BotConfig,
    pub alpha_client: AlphaOSClient,
    pub delta_client: DeltaOSClient,
}
```

---

## Behavior API

### Behavior Trait

Core trait for all behaviors.

```rust
#[async_trait]
pub trait Behavior: Send + Sync {
    /// Behavior identifier
    fn id(&self) -> &str;

    /// Behavior category
    fn category(&self) -> BehaviorCategory;

    /// Setup behavior (pre-execution)
    async fn setup(&mut self, context: &BehaviorContext) -> Result<()>;

    /// Execute behavior
    async fn execute(&mut self, context: &BehaviorContext) -> Result<BehaviorResult>;

    /// Teardown behavior (post-execution)
    async fn teardown(&mut self, context: &BehaviorContext) -> Result<()>;
}
```

**BehaviorCategory:**

```rust
pub enum BehaviorCategory {
    Legitimate,
    Adversarial,
    AntiPattern,
}
```

**BehaviorResult:**

```rust
pub enum BehaviorResult {
    Success { duration_ms: u64 },
    Failure { error: String, duration_ms: u64 },
    Timeout { duration_ms: u64 },
}
```

### Implementing a Custom Behavior

```rust
use adnet_testbots::behaviors::{Behavior, BehaviorCategory, BehaviorResult, BehaviorContext};

pub struct TransferBehavior {
    amount: u128,
    recipient: String,
}

#[async_trait]
impl Behavior for TransferBehavior {
    fn id(&self) -> &str {
        "transfer.simple"
    }

    fn category(&self) -> BehaviorCategory {
        BehaviorCategory::Legitimate
    }

    async fn setup(&mut self, context: &BehaviorContext) -> Result<()> {
        // Pre-execution checks
        if context.wallet.ax_balance < self.amount {
            anyhow::bail!("Insufficient balance");
        }
        Ok(())
    }

    async fn execute(&mut self, context: &BehaviorContext) -> Result<BehaviorResult> {
        let start = std::time::Instant::now();

        // Submit transfer transaction
        let tx = context.alpha_client
            .transfer(self.amount, &self.recipient)
            .await?;

        // Wait for confirmation
        context.alpha_client
            .wait_for_confirmation(&tx.id, Duration::from_secs(30))
            .await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(BehaviorResult::Success { duration_ms })
    }

    async fn teardown(&mut self, context: &BehaviorContext) -> Result<()> {
        // Post-execution cleanup
        Ok(())
    }
}
```

### Pre-Built Behaviors

**Legitimate Behaviors:**

```rust
// Cross-chain
use adnet_testbots::behaviors::legitimate::cross_chain::{LockMintFlow, BurnUnlockFlow};

// Trading
use adnet_testbots::behaviors::legitimate::trading::{SpotTrade, PerpetualTrade};

// Governance
use adnet_testbots::behaviors::legitimate::governance::{ProposeAndVote, ExecuteProposal};

// Privacy
use adnet_testbots::behaviors::legitimate::privacy::{ShieldedTransfer, AddressRecycle};
```

**Adversarial Behaviors:**

```rust
// MEV
use adnet_testbots::behaviors::adversarial::mev::{SandwichAttack, FrontRun};

// Byzantine
use adnet_testbots::behaviors::adversarial::byzantine::{Equivocate, WithholdAttestations};

// Governance
use adnet_testbots::behaviors::adversarial::governance::{SybilAttack, VoteBuying};
```

---

## Scenario API

### Scenario Structure

Scenarios are defined in YAML format:

```yaml
scenario:
  metadata:
    id: "FUNC-001"
    name: "Simple Transfer Test"
    type: functional
    duration: "5 min"

  setup:
    network:
      alpha_rest: "http://localhost:3030"
    bots:
      - id: "user-{1-10}"
        role: "general_user"
        count: 10
        wallet_ax: 1000000

  phases:
    - name: "Execute transfers"
      duration: "3 min"
      bots: "user-*"
      behavior: "transfer.simple"
      params:
        amount: 100000
        rate_per_bot: 1
      assertions:
        - success_rate: ">95%"
      metrics:
        - tps: "rate"
        - latency_p95: "ms"

  success_criteria:
    - total_transactions: ">100"
    - error_rate: "<5%"
```

### Running Scenarios Programmatically

```rust
use adnet_testbots::scenario::{Scenario, ScenarioRunner};

#[tokio::main]
async fn main() -> Result<()> {
    // Load scenario from YAML
    let scenario = Scenario::from_file("scenarios/functional/simple_transfer.yaml")?;

    // Create runner
    let runner = ScenarioRunner::new(scenario);

    // Execute
    let result = runner.run().await?;

    // Check results
    assert!(result.success);
    assert!(result.error_rate < 0.05);

    println!("TPS: {}", result.average_tps);
    println!("Latency p95: {}ms", result.latency_p95_ms);

    Ok(())
}
```

### Scenario CLI

```bash
# Run scenario
adnet-testbots run simple-transfer

# Run with custom duration
adnet-testbots run simple-transfer --duration 10m

# Run in distributed mode
adnet-testbots run simple-transfer --distributed --workers 3

# Dry run (validate without executing)
adnet-testbots run simple-transfer --dry-run

# Export metrics
adnet-testbots run simple-transfer --export-metrics results.json
```

---

## Metrics API

### MetricsAggregator

Real-time metrics aggregation with HDR histogram.

```rust
use adnet_testbots::metrics::{MetricsAggregator, BotEvent};

// Create aggregator
let aggregator = MetricsAggregator::new();

// Process events
aggregator.process_event(&BotEvent::BehaviorCompleted {
    bot_id: "bot-1".to_string(),
    behavior_id: "transfer".to_string(),
    timestamp_ms: 1234567890,
    duration_ms: 150,
    success: true,
});

// Query metrics
let tps = aggregator.tps();
let p95 = aggregator.latency_p95();
let error_rate = aggregator.error_rate();

println!("TPS: {:.2}", tps);
println!("p95 latency: {:.2}ms", p95);
println!("Error rate: {:.2}%", error_rate * 100.0);
```

### BotEvent Types

```rust
pub enum BotEvent {
    BotStarted {
        bot_id: String,
        role: String,
        timestamp_ms: i64,
    },
    BotStopped {
        bot_id: String,
        timestamp_ms: i64,
        reason: String,
    },
    BehaviorStarted {
        bot_id: String,
        behavior_id: String,
        timestamp_ms: i64,
    },
    BehaviorCompleted {
        bot_id: String,
        behavior_id: String,
        timestamp_ms: i64,
        duration_ms: u64,
        success: bool,
    },
    TransactionSubmitted {
        bot_id: String,
        tx_id: String,
        timestamp_ms: i64,
    },
    TransactionConfirmed {
        bot_id: String,
        tx_id: String,
        timestamp_ms: i64,
        confirmation_time_ms: u64,
    },
    TransactionFailed {
        bot_id: String,
        tx_id: String,
        timestamp_ms: i64,
        error: String,
    },
    BotError {
        bot_id: String,
        error: String,
        timestamp_ms: i64,
    },
    NetworkResponse {
        bot_id: String,
        endpoint: String,
        latency_ms: u64,
        status_code: u16,
        timestamp_ms: i64,
    },
}
```

### Prometheus Export

```rust
use adnet_testbots::metrics::{MetricsAggregator, PrometheusExporter};
use std::sync::Arc;

// Create aggregator
let aggregator = Arc::new(MetricsAggregator::new());

// Create Prometheus exporter
let exporter = Arc::new(
    PrometheusExporter::new(Arc::clone(&aggregator), "0.0.0.0:9090".to_string())
);

// Start HTTP server
tokio::spawn(async move {
    exporter.start().await.expect("Failed to start exporter");
});

// Access metrics at http://localhost:9090/metrics
```

---

## Distributed API

### Coordinator

Central command & control for distributed bot execution.

```rust
use adnet_testbots::distributed::{Coordinator, WorkerInfo};

#[tokio::main]
async fn main() -> Result<()> {
    // Create coordinator
    let mut coordinator = Coordinator::new("0.0.0.0:50051");

    // Start server
    coordinator.start().await?;

    // Coordinator automatically handles:
    // - Worker registration
    // - Bot distribution
    // - Metrics aggregation
    // - Health monitoring

    Ok(())
}
```

### Worker

Bot execution worker daemon.

```rust
use adnet_testbots::distributed::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    // Create worker
    let mut worker = Worker::new();

    // Connect to coordinator
    worker.connect_coordinator("coordinator.example.com:50051").await?;

    // Register capabilities
    worker.register(WorkerInfo {
        worker_id: "worker-1".to_string(),
        cpu_cores: 8,
        memory_bytes: 16_000_000_000,
        max_bots: 200,
        capabilities: vec!["trader".to_string(), "user".to_string()],
    }).await?;

    // Start worker loop
    worker.run().await?;

    Ok(())
}
```

### WorkerInfo

```rust
pub struct WorkerInfo {
    pub worker_id: String,
    pub cpu_cores: u32,
    pub memory_bytes: u64,
    pub max_bots: u32,
    pub capabilities: Vec<String>,  // ["trader", "prover", etc.]
}
```

### BotSpec

```rust
pub struct BotSpec {
    pub bot_id: String,
    pub role: String,
    pub behavior: String,
    pub config: BotConfig,
    pub target_network: String,
}
```

### Distributed Execution

```bash
# Terminal 1: Start coordinator
adnet-testbots coordinator start --bind 0.0.0.0:50051

# Terminal 2-4: Start workers
adnet-testbots worker start --coordinator localhost:50051 --max-bots 200

# Terminal 5: Run distributed scenario
adnet-testbots run peak-tps-stress --distributed --workers 3
```

---

## Integration Clients

### AlphaOS Client

```rust
use adnet_testbots::integration::AlphaOSClient;

let client = AlphaOSClient::new("http://localhost:3030")?;

// Query block height
let height = client.block_height().await?;

// Submit transaction
let tx_id = client.submit_transaction(tx_data).await?;

// Wait for confirmation
client.wait_for_confirmation(&tx_id, Duration::from_secs(30)).await?;

// Query balance
let balance = client.balance("ax1...").await?;
```

### DeltaOS Client

```rust
use adnet_testbots::integration::DeltaOSClient;

let client = DeltaOSClient::new("http://localhost:3031")?;

// Submit order
let order_id = client.submit_order(Order {
    pair: "DX/sAX".to_string(),
    side: OrderSide::Buy,
    amount: 1000,
    price: 1.0,
}).await?;

// Query orderbook
let orderbook = client.orderbook("DX/sAX").await?;

// Cancel order
client.cancel_order(&order_id).await?;
```

### Adnet Client

```rust
use adnet_testbots::integration::AdnetClient;

let client = AdnetClient::new()?;

// Execute CLI command
let output = client.execute(&["account", "new"]).await?;

// Parse output
let address = parse_address(&output)?;
```

---

## Error Handling

All APIs use `anyhow::Result` for flexible error handling:

```rust
use anyhow::{Result, Context};

pub async fn transfer(amount: u128) -> Result<String> {
    let client = AlphaOSClient::new("http://localhost:3030")
        .context("Failed to create client")?;

    let tx_id = client.submit_transaction(tx_data).await
        .context("Failed to submit transaction")?;

    Ok(tx_id)
}
```

## Best Practices

1. **Always use async/await** for I/O operations
2. **Return Result** for fallible operations
3. **Use contexts** in error chains
4. **Validate inputs** before execution
5. **Clean up resources** in teardown/drop
6. **Log important events** with `tracing`
7. **Use HDR histogram** for latency measurements
8. **Process events** for observability
9. **Test with proptest** for correctness
10. **Document public APIs** with rustdoc

## Examples

See `examples/` directory for complete examples:

- `simple_bot.rs` - Single bot example
- `bot_fleet.rs` - Multi-bot coordination
- `distributed_fleet.rs` - Distributed execution
- `custom_behavior.rs` - Custom behavior implementation
- `metrics_export.rs` - Prometheus integration

## Further Reading

- [Bot Framework Guide](./DESIGN.md#bot-framework)
- [Behavior System](./DESIGN.md#behavior-system)
- [Distributed Architecture](./DESIGN.md#distributed-architecture)
- [Prometheus Metrics](./PROMETHEUS.md)
- [Scenario Catalog](../scenarios/README.md)
