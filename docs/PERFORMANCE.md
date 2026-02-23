# Performance Optimization Guide

This document provides performance profiling strategies, optimization opportunities, and benchmarking guidelines for adnet-testbots.

## Performance Targets

| Metric | Target | Critical Threshold |
|--------|--------|-------------------|
| Bot spawn time | <1ms | <5ms |
| Behavior overhead | <5% | <10% |
| Metrics aggregation | <100µs/event | <1ms/event |
| TPS sustained | 10,000+ | 5,000+ |
| Coordinator overhead | <2% CPU | <10% CPU |
| Worker overhead | <5% CPU/100 bots | <15% CPU/100 bots |
| Memory per bot | <500KB | <2MB |
| gRPC latency | <1ms local | <10ms local |

## Profiling Strategy

### 1. CPU Profiling with Flamegraph

```bash
# Install flamegraph
cargo install flamegraph

# Profile scenario execution
cargo flamegraph --bin adnet-testbots -- run peak-tps-stress --duration 5m

# Output: flamegraph.svg (open in browser)
```

**Hot Paths to Investigate:**
- HDR histogram record operations
- Event serialization/deserialization
- HTTP request handling
- Signature verification
- Bot spawning/teardown

### 2. Memory Profiling

```bash
# Install valgrind/heaptrack
sudo apt install valgrind heaptrack

# Memory profile
heaptrack ./target/release/adnet-testbots run peak-tps-stress --duration 1m

# Analyze
heaptrack_gui heaptrack.adnet-testbots.*.gz
```

**Memory Hotspots:**
- Event buffering (60s buffer per worker)
- HDR histogram storage
- Bot state storage
- HTTP connection pools
- gRPC channel management

### 3. Async Profiling with Tokio Console

```toml
# Cargo.toml
[dependencies]
tokio = { version = "1.35", features = ["full", "tracing"] }
console-subscriber = "0.2"
```

```rust
// main.rs
#[tokio::main]
async fn main() {
    console_subscriber::init();
    // ... rest of code
}
```

```bash
# Terminal 1: Run with console
TOKIO_CONSOLE=1 ./target/release/adnet-testbots run peak-tps-stress

# Terminal 2: Connect tokio-console
tokio-console
```

**Async Bottlenecks:**
- Task starvation
- Lock contention (RwLock in aggregator)
- Channel backpressure
- gRPC stream buffering

## Optimization Opportunities

### 1. Hot Path: Metrics Aggregation

**Current:**
```rust
pub fn process_event(&self, event: &BotEvent) {
    let mut state = self.state.write();  // Lock acquisition
    // ... processing
}
```

**Optimization:**
- Use `crossbeam` channel for lock-free event submission
- Batch process events (100 events/batch)
- Separate hot/cold metrics (fast path for latency, slow path for detailed stats)

**Expected Improvement:** 5x throughput (100µs → 20µs per event)

### 2. Hot Path: HDR Histogram

**Current:**
```rust
let _ = state.latency_histogram.record(*duration_ms * 1000);
```

**Optimization:**
- Pre-allocate histogram with correct bounds
- Use `record_unchecked` if bounds guaranteed
- Consider alternative: [metrics](https://docs.rs/metrics) crate with faster implementation

**Expected Improvement:** 2x throughput

### 3. Hot Path: HTTP Requests

**Current:**
```rust
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(10)
    .build()?;
```

**Optimization:**
- Increase pool size: `.pool_max_idle_per_host(50)`
- Enable HTTP/2: `.http2_prior_knowledge()`
- Reuse single client globally (not per-bot)
- Consider `hyper` directly for lower-level control

**Expected Improvement:** 3x request rate

### 4. Hot Path: Event Serialization

**Current:**
```rust
use serde_json;
let json = serde_json::to_string(&event)?;
```

**Optimization:**
- Use `bincode` instead of JSON (10x faster)
- Pre-allocate serialization buffer
- Use `rkyv` for zero-copy deserialization

**Expected Improvement:** 10x serialization speed

### 5. Bot State Management

**Current:**
```rust
pub struct Bot {
    identity: Identity,
    wallet: Wallet,
    behaviors: HashMap<String, Box<dyn Behavior>>,
    state: StateMachine<S>,
}
```

**Optimization:**
- Use `Arc<Behavior>` to share immutable behaviors
- Use `SmallVec` for behaviors (stack allocation for <8 behaviors)
- Pool bot instances instead of creating/destroying

**Expected Improvement:** 5x bot spawn rate

### 6. gRPC Communication

**Current:**
```rust
let channel = tonic::transport::Channel::from_static("http://localhost:50051")
    .connect()
    .await?;
```

**Optimization:**
- Use connection pooling: `.concurrency_limit(256)`
- Enable keepalive: `.keep_alive_timeout(Duration::from_secs(60))`
- Batch RPC calls (spawn_bot_batch instead of spawn_bot)
- Use unary calls for small requests, streaming for large

**Expected Improvement:** 10x RPC throughput

### 7. Distributed Metrics Streaming

**Current:**
```rust
// Stream metrics every event
async fn stream_metrics(&self, tx: mpsc::Sender<WorkerMetrics>) {
    for event in &self.events {
        tx.send(event).await?;
    }
}
```

**Optimization:**
- Batch metrics (100 events per message)
- Compress with `lz4` before sending
- Use interval-based send (every 5s) instead of per-event
- Aggregate locally, send summaries only

**Expected Improvement:** 100x reduction in network traffic

## Memory Optimization

### 1. Event Buffer Sizing

**Current:**
```rust
// 60s buffer per worker
let buffer_size = events_per_sec * 60;
```

**Optimization:**
- Use ring buffer with fixed size (cap at 10K events)
- Drop oldest events if full (lossy, but prevents OOM)
- Spill to disk for long-duration scenarios

**Expected Improvement:** Bounded memory usage

### 2. HDR Histogram Memory

**Current:**
```rust
latency_histogram: Histogram::new_with_bounds(1, 60_000_000, 3)
```

**Memory:** ~200KB per histogram

**Optimization:**
- Use lower precision: `2` instead of `3` (50% memory reduction)
- Shorter max latency: `30_000_000` (30s instead of 60s)
- Reset histogram periodically for rolling window

**Expected Improvement:** 2x memory efficiency

### 3. Bot Pool Reuse

**Current:**
```rust
// Create new bot every time
let bot = Bot::new(spec)?;
bot.execute().await?;
drop(bot);  // Deallocate
```

**Optimization:**
```rust
// Pool pattern
struct BotPool {
    idle: Vec<Box<dyn Bot>>,
    active: HashMap<String, Box<dyn Bot>>,
}

impl BotPool {
    fn acquire(&mut self) -> Box<dyn Bot> {
        self.idle.pop().unwrap_or_else(|| Box::new(Bot::new()))
    }

    fn release(&mut self, bot: Box<dyn Bot>) {
        bot.reset();
        self.idle.push(bot);
    }
}
```

**Expected Improvement:** 10x bot spawn rate, 5x memory efficiency

## Concurrency Optimization

### 1. Lock Contention Reduction

**Current:**
```rust
pub struct MetricsAggregator {
    state: Arc<RwLock<AggregatorState>>,  // Single lock
}
```

**Optimization:**
```rust
pub struct MetricsAggregator {
    // Sharded locks (reduce contention)
    shards: [Arc<RwLock<AggregatorState>>; 16],
}

impl MetricsAggregator {
    fn shard_for_bot(&self, bot_id: &str) -> usize {
        hash(bot_id) % 16
    }
}
```

**Expected Improvement:** 16x concurrent throughput

### 2. Channel Sizing

**Current:**
```rust
let (tx, rx) = mpsc::channel(100);  // Small buffer
```

**Optimization:**
```rust
let (tx, rx) = mpsc::channel(10000);  // Larger buffer reduces backpressure
```

**Expected Improvement:** Smoother throughput under load

### 3. Parallelization

**Current:**
```rust
// Sequential bot execution
for bot in bots {
    bot.execute().await?;
}
```

**Optimization:**
```rust
// Parallel bot execution
let tasks: Vec<_> = bots.into_iter()
    .map(|bot| tokio::spawn(async move { bot.execute().await }))
    .collect();

futures::future::join_all(tasks).await;
```

**Expected Improvement:** N bots in parallel (linear scaling)

## Benchmarking

### Micro-Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_bot_spawn(c: &mut Criterion) {
    c.bench_function("bot_spawn", |b| {
        b.iter(|| {
            let bot = Bot::new(black_box(BotSpec::default()));
            black_box(bot);
        });
    });
}

fn bench_event_process(c: &mut Criterion) {
    let aggregator = MetricsAggregator::new();
    let event = BotEvent::BehaviorCompleted {
        bot_id: "bot-1".to_string(),
        behavior_id: "test".to_string(),
        timestamp_ms: 123456,
        duration_ms: 100,
        success: true,
    };

    c.bench_function("event_process", |b| {
        b.iter(|| {
            aggregator.process_event(black_box(&event));
        });
    });
}

criterion_group!(benches, bench_bot_spawn, bench_event_process);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

### End-to-End Benchmarks

```bash
# Scenario execution time
time adnet-testbots run peak-tps-stress --duration 5m

# TPS measurement
adnet-testbots run peak-tps-stress --duration 5m --export-metrics metrics.json
jq '.average_tps' metrics.json
```

## Regression Testing

Add performance tests to CI:

```yaml
# .woodpecker.yml
steps:
  - name: performance-regression
    commands:
      # Baseline
      - git checkout main
      - cargo build --release
      - ./target/release/adnet-testbots run peak-tps-stress --duration 1m --export-metrics baseline.json

      # Current
      - git checkout $CI_COMMIT_SHA
      - cargo build --release
      - ./target/release/adnet-testbots run peak-tps-stress --duration 1m --export-metrics current.json

      # Compare
      - python3 scripts/compare_performance.py baseline.json current.json
    when:
      event: pull_request
```

## Profiling Checklist

Before optimization:

- [ ] Run flamegraph to identify CPU hotspots
- [ ] Run heaptrack to identify memory allocations
- [ ] Run tokio-console to identify async bottlenecks
- [ ] Measure baseline TPS with target scenario
- [ ] Measure baseline latency (p50, p95, p99)
- [ ] Measure baseline memory usage
- [ ] Identify top 3 bottlenecks

After optimization:

- [ ] Re-run all profiling tools
- [ ] Measure new TPS (expect >2x improvement)
- [ ] Measure new latency (expect <50% reduction)
- [ ] Measure new memory (expect <30% reduction)
- [ ] Verify correctness (all tests pass)
- [ ] Update benchmarks

## Known Performance Bottlenecks

1. **HDR Histogram Record** - 10% of CPU time
   - Optimization: Switch to faster histogram implementation or batch records

2. **Signature Verification** - 15% of CPU time
   - Optimization: Cache recent signatures, use faster Ed25519 library

3. **HTTP Connection Setup** - 5% of CPU time
   - Optimization: Increase connection pool, enable HTTP/2

4. **Event Serialization** - 8% of CPU time
   - Optimization: Switch from JSON to bincode

5. **Lock Contention in Aggregator** - 12% of CPU time
   - Optimization: Shard locks by bot_id

## Performance Tips

1. **Always measure first** - Don't optimize blindly
2. **Focus on hot paths** - 80/20 rule applies
3. **Use release mode** - `cargo build --release` is 10-100x faster
4. **Profile in production** - Synthetic benchmarks can mislead
5. **Batch operations** - Reduces syscall/lock overhead
6. **Cache aggressively** - Especially cryptographic operations
7. **Avoid allocations** - Use stack allocation where possible
8. **Minimize copying** - Use references and zero-copy
9. **Parallelize smartly** - Too much parallelism increases overhead
10. **Monitor continuously** - Use Prometheus to track performance over time

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Tokio Performance](https://tokio.rs/tokio/topics/performance)
- [HDR Histogram Paper](http://hdrhistogram.org/hdrhistogram.pdf)
- [Flamegraph](https://github.com/flamegraph-rs/flamegraph)
- [Criterion.rs](https://github.com/bheisler/criterion.rs)
