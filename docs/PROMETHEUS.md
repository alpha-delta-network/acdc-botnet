# Prometheus Metrics Export

AdNet Testbots exposes comprehensive metrics via Prometheus HTTP endpoint for real-time monitoring and alerting.

## Quick Start

```rust
use adnet_testbots::metrics::{MetricsAggregator, PrometheusExporter};
use std::sync::Arc;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // Create aggregator
    let aggregator = Arc::new(MetricsAggregator::new());

    // Create Prometheus exporter
    let mut labels = HashMap::new();
    labels.insert("environment".to_string(), "testnet".to_string());
    labels.insert("cluster".to_string(), "alpha-delta".to_string());

    let exporter = Arc::new(
        PrometheusExporter::new(Arc::clone(&aggregator), "0.0.0.0:9090".to_string())
            .with_labels(labels)
    );

    // Start HTTP server
    tokio::spawn(async move {
        exporter.start().await.expect("Failed to start Prometheus exporter");
    });

    // Access metrics at http://localhost:9090/metrics
}
```

## Exposed Metrics

### Bot Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_bots_active` | Gauge | Number of currently active bots |
| `testbots_bots_by_role{role="..."}` | Gauge | Number of bots by role (trader, validator, etc.) |

### Performance Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_tps_current` | Gauge | Current transactions per second |
| `testbots_latency_seconds{quantile="0.5"}` | Summary | p50 latency in seconds |
| `testbots_latency_seconds{quantile="0.95"}` | Summary | p95 latency in seconds |
| `testbots_latency_seconds{quantile="0.99"}` | Summary | p99 latency in seconds |

### Error Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_error_rate` | Gauge | Current error rate (0.0 to 1.0) |
| `testbots_errors_total` | Counter | Total number of errors |
| `testbots_transactions_total` | Counter | Total number of transactions |

### Behavior Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_behavior_success_rate{behavior="..."}` | Gauge | Success rate by behavior type |

### Scenario Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_scenario_active{scenario="..."}` | Gauge | Currently active scenario (1 if active, absent if not) |
| `testbots_scenario_progress{scenario="..."}` | Gauge | Scenario completion progress (0.0 to 1.0) |

### Distributed Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_workers_active` | Gauge | Number of active distributed workers |
| `testbots_worker_bots{worker="..."}` | Gauge | Number of bots per worker |

### System Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `testbots_uptime_seconds` | Counter | Total uptime in seconds |

## Prometheus Configuration

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'adnet-testbots'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

## Grafana Dashboard

### Sample PromQL Queries

**Current TPS:**
```promql
testbots_tps_current
```

**p95 Latency (milliseconds):**
```promql
testbots_latency_seconds{quantile="0.95"} * 1000
```

**Error Rate Percentage:**
```promql
testbots_error_rate * 100
```

**Active Bots by Role:**
```promql
sum by (role) (testbots_bots_by_role)
```

**Behavior Success Rates:**
```promql
testbots_behavior_success_rate * 100
```

**Worker Load Distribution:**
```promql
testbots_worker_bots
```

### Dashboard Panels

**Performance Overview:**
- TPS graph (time series)
- Latency percentiles (multi-line graph)
- Error rate (gauge)
- Active bots (stat)

**Bot Distribution:**
- Bots by role (pie chart)
- Worker distribution (bar chart)

**Behavior Analysis:**
- Behavior success rates (bar chart)
- Behavior execution count (counter)

**Scenario Progress:**
- Active scenario (stat)
- Progress gauge (0-100%)
- Phase breakdown (table)

## Alerting Rules

Add to `alert.rules.yml`:

```yaml
groups:
  - name: testbots
    interval: 30s
    rules:
      - alert: HighErrorRate
        expr: testbots_error_rate > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | humanizePercentage }}"

      - alert: LowTPS
        expr: testbots_tps_current < 100
        for: 5m
        labels:
          severity: info
        annotations:
          summary: "TPS below threshold"
          description: "Current TPS is {{ $value }}"

      - alert: HighLatency
        expr: testbots_latency_seconds{quantile="0.95"} > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High p95 latency"
          description: "p95 latency is {{ $value }}s"

      - alert: WorkerDown
        expr: changes(testbots_workers_active[5m]) < 0
        labels:
          severity: critical
        annotations:
          summary: "Worker count decreased"
          description: "Active workers dropped from {{ $value }}"

      - alert: BehaviorFailures
        expr: testbots_behavior_success_rate < 0.9
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Behavior {{ $labels.behavior }} success rate low"
          description: "Success rate is {{ $value | humanizePercentage }}"
```

## Custom Labels

Add custom labels to all metrics:

```rust
let mut labels = HashMap::new();
labels.insert("environment".to_string(), "production".to_string());
labels.insert("region".to_string(), "us-east-1".to_string());
labels.insert("cluster".to_string(), "alpha-delta-main".to_string());

let exporter = PrometheusExporter::new(aggregator, "0.0.0.0:9090".to_string())
    .with_labels(labels);
```

Labels appear in all metrics:
```
testbots_tps_current{environment="production",region="us-east-1",cluster="alpha-delta-main"} 1234.56
```

## Update Interval

Control metrics update frequency:

```rust
use std::time::Duration;

let exporter = PrometheusExporter::new(aggregator, "0.0.0.0:9090".to_string())
    .with_update_interval(Duration::from_secs(30));  // Update every 30s
```

## Multi-Instance Setup

Run multiple testbot instances with different labels:

**Instance 1 (Functional Tests):**
```rust
let mut labels = HashMap::new();
labels.insert("test_type".to_string(), "functional".to_string());
labels.insert("instance".to_string(), "func-1".to_string());
```

**Instance 2 (Security Tests):**
```rust
let mut labels = HashMap::new();
labels.insert("test_type".to_string(), "security".to_string());
labels.insert("instance".to_string(), "sec-1".to_string());
```

Query by instance:
```promql
testbots_tps_current{test_type="functional"}
testbots_error_rate{test_type="security"}
```

## Troubleshooting

### Metrics Not Updating

Check aggregator is processing events:
```promql
rate(testbots_transactions_total[1m])
```

If zero, verify events are being recorded.

### High Memory Usage

HDR Histogram bounded at 60 seconds max latency. If operations exceed this, histogram will saturate.

### Missing Worker Metrics

Workers only appear after first heartbeat. Check distributed coordinator is running.

## Integration with CI

Export metrics to file for CI verification:

```bash
curl http://localhost:9090/metrics > testbots-metrics.txt

# Verify TPS target met
grep "testbots_tps_current" testbots-metrics.txt | awk '{if ($2 < 1000) exit 1}'

# Verify error rate acceptable
grep "testbots_error_rate" testbots-metrics.txt | awk '{if ($2 > 0.01) exit 1}'
```

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Dashboards](https://grafana.com/grafana/dashboards/)
- [PromQL Tutorial](https://prometheus.io/docs/prometheus/latest/querying/basics/)
