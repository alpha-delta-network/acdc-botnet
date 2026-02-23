/// Prometheus metrics exporter for adnet-testbots
///
/// Exposes metrics via HTTP endpoint at /metrics in Prometheus text format.
/// Provides real-time visibility into bot operations, scenario execution,
/// and system health.

use crate::aggregator::MetricsAggregator;
use crate::event::BotEvent;
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

/// Prometheus metrics exporter
pub struct PrometheusExporter {
    aggregator: Arc<MetricsAggregator>,
    bind_address: String,
    update_interval: Duration,
    custom_labels: HashMap<String, String>,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new(aggregator: Arc<MetricsAggregator>, bind_address: String) -> Self {
        Self {
            aggregator,
            bind_address,
            update_interval: Duration::from_secs(15),
            custom_labels: HashMap::new(),
        }
    }

    /// Set custom labels for all metrics
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.custom_labels = labels;
        self
    }

    /// Set metrics update interval
    pub fn with_update_interval(mut self, interval: Duration) -> Self {
        self.update_interval = interval;
        self
    }

    /// Start the Prometheus HTTP server
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let listener = TcpListener::bind(&self.bind_address).await?;
        tracing::info!("Prometheus exporter listening on {}", self.bind_address);

        loop {
            let (stream, _) = listener.accept().await?;
            let exporter = Arc::clone(&self);

            tokio::spawn(async move {
                if let Err(e) = exporter.handle_connection(stream).await {
                    tracing::error!("Failed to handle Prometheus request: {}", e);
                }
            });
        }
    }

    /// Handle incoming HTTP connection
    async fn handle_connection(&self, stream: tokio::net::TcpStream) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut buffer = [0; 1024];
        let mut stream = stream;

        // Read request (simplified HTTP parser)
        let n = stream.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        // Check if request is for /metrics endpoint
        if request.contains("GET /metrics") {
            let metrics_text = self.generate_metrics_text().await;

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                metrics_text.len(),
                metrics_text
            );

            stream.write_all(response.as_bytes()).await?;
        } else {
            // 404 for other paths
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }

    /// Generate Prometheus metrics text format
    async fn generate_metrics_text(&self) -> String {
        let mut output = String::new();

        // Get current metrics snapshot
        let snapshot = self.aggregator.snapshot();

        // Add custom labels
        let labels = self.format_labels();

        // Bot metrics
        output.push_str(&format!(
            "# HELP testbots_bots_active Number of active bots\n\
             # TYPE testbots_bots_active gauge\n\
             testbots_bots_active{{{}}} {}\n\n",
            labels, snapshot.active_bots
        ));

        // TPS metrics
        output.push_str(&format!(
            "# HELP testbots_tps_current Current transactions per second\n\
             # TYPE testbots_tps_current gauge\n\
             testbots_tps_current{{{}}} {:.2}\n\n",
            labels, snapshot.tps
        ));

        // Latency metrics (p50, p95, p99) - convert ms to seconds
        output.push_str(&format!(
            "# HELP testbots_latency_seconds Transaction latency distribution\n\
             # TYPE testbots_latency_seconds summary\n\
             testbots_latency_seconds{{quantile=\"0.5\",{}}} {:.6}\n\
             testbots_latency_seconds{{quantile=\"0.95\",{}}} {:.6}\n\
             testbots_latency_seconds{{quantile=\"0.99\",{}}} {:.6}\n\n",
            labels,
            snapshot.latency_p50_ms / 1000.0,
            labels,
            snapshot.latency_p95_ms / 1000.0,
            labels,
            snapshot.latency_p99_ms / 1000.0
        ));

        // Error rate
        output.push_str(&format!(
            "# HELP testbots_error_rate Current error rate (0.0 to 1.0)\n\
             # TYPE testbots_error_rate gauge\n\
             testbots_error_rate{{{}}} {:.4}\n\n",
            labels, snapshot.error_rate
        ));

        // Total transactions
        output.push_str(&format!(
            "# HELP testbots_transactions_total Total number of transactions\n\
             # TYPE testbots_transactions_total counter\n\
             testbots_transactions_total{{{}}} {}\n\n",
            labels, snapshot.total_operations
        ));

        // Total errors
        output.push_str(&format!(
            "# HELP testbots_errors_total Total number of errors\n\
             # TYPE testbots_errors_total counter\n\
             testbots_errors_total{{{}}} {}\n\n",
            labels, snapshot.total_errors
        ));

        // Behavior success rates (per behavior type)
        if !snapshot.behavior_success_rates.is_empty() {
            output.push_str(&format!(
                "# HELP testbots_behavior_success_rate Success rate by behavior type\n\
                 # TYPE testbots_behavior_success_rate gauge\n"
            ));
            for (behavior, rate) in &snapshot.behavior_success_rates {
                output.push_str(&format!(
                    "testbots_behavior_success_rate{{behavior=\"{}\",{}}} {:.4}\n",
                    behavior, labels, rate
                ));
            }
            output.push('\n');
        }

        // Role distribution
        if !snapshot.bots_by_role.is_empty() {
            output.push_str(&format!(
                "# HELP testbots_bots_by_role Number of bots by role\n\
                 # TYPE testbots_bots_by_role gauge\n"
            ));
            for (role, count) in &snapshot.bots_by_role {
                output.push_str(&format!(
                    "testbots_bots_by_role{{role=\"{}\",{}}} {}\n",
                    role, labels, count
                ));
            }
            output.push('\n');
        }

        // Scenario status (if running)
        if let Some(scenario_name) = &snapshot.active_scenario {
            output.push_str(&format!(
                "# HELP testbots_scenario_active Currently active scenario\n\
                 # TYPE testbots_scenario_active gauge\n\
                 testbots_scenario_active{{scenario=\"{}\",{}}} 1\n\n",
                scenario_name, labels
            ));

            if let Some(progress) = snapshot.scenario_progress {
                output.push_str(&format!(
                    "# HELP testbots_scenario_progress Scenario completion progress (0.0 to 1.0)\n\
                     # TYPE testbots_scenario_progress gauge\n\
                     testbots_scenario_progress{{scenario=\"{}\",{}}} {:.4}\n\n",
                    scenario_name, labels, progress
                ));
            }
        }

        // Distributed worker metrics (if applicable)
        if !snapshot.workers.is_empty() {
            output.push_str(&format!(
                "# HELP testbots_workers_active Number of active distributed workers\n\
                 # TYPE testbots_workers_active gauge\n\
                 testbots_workers_active{{{}}} {}\n\n",
                labels,
                snapshot.workers.len()
            ));

            output.push_str(&format!(
                "# HELP testbots_worker_bots Number of bots per worker\n\
                 # TYPE testbots_worker_bots gauge\n"
            ));
            for (worker_id, bot_count) in &snapshot.workers {
                output.push_str(&format!(
                    "testbots_worker_bots{{worker=\"{}\",{}}} {}\n",
                    worker_id, labels, bot_count
                ));
            }
            output.push('\n');
        }

        // Uptime metric - calculate from timestamp
        let uptime_secs = (current_time_ms() - snapshot.timestamp_ms) as f64 / 1000.0;
        output.push_str(&format!(
            "# HELP testbots_uptime_seconds Total uptime in seconds\n\
             # TYPE testbots_uptime_seconds counter\n\
             testbots_uptime_seconds{{{}}} {:.0}\n\n",
            labels,
            uptime_secs
        ));

        output
    }

    /// Format custom labels for Prometheus
    fn format_labels(&self) -> String {
        if self.custom_labels.is_empty() {
            return String::new();
        }

        self.custom_labels
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Get current time in milliseconds since UNIX epoch
fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_labels() {
        let aggregator = Arc::new(MetricsAggregator::new());
        let mut exporter = PrometheusExporter::new(aggregator, "127.0.0.1:9090".to_string());

        let mut labels = HashMap::new();
        labels.insert("environment".to_string(), "testnet".to_string());
        labels.insert("region".to_string(), "us-west".to_string());

        exporter.custom_labels = labels;
        let formatted = exporter.format_labels();

        assert!(formatted.contains("environment=\"testnet\""));
        assert!(formatted.contains("region=\"us-west\""));
    }

    #[tokio::test]
    async fn test_prometheus_text_generation() {
        let aggregator = Arc::new(MetricsAggregator::new());
        let exporter = PrometheusExporter::new(aggregator, "127.0.0.1:9091".to_string());

        let metrics_text = exporter.generate_metrics_text().await;

        // Verify Prometheus format
        assert!(metrics_text.contains("# HELP"));
        assert!(metrics_text.contains("# TYPE"));
        assert!(metrics_text.contains("testbots_tps_current"));
        assert!(metrics_text.contains("testbots_latency_seconds"));
        assert!(metrics_text.contains("testbots_error_rate"));
    }

    #[test]
    fn test_metrics_snapshot() {
        let aggregator = MetricsAggregator::new();
        let snapshot = aggregator.snapshot();

        // Verify initial state
        assert_eq!(snapshot.active_bots, 0);
        assert_eq!(snapshot.tps, 0.0);
        assert_eq!(snapshot.total_operations, 0);
        assert_eq!(snapshot.total_errors, 0);
    }
}
