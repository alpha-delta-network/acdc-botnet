/// Metrics aggregator with HDR histogram for latency tracking
use crate::BotEvent;
use hdrhistogram::Histogram;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Real-time metrics aggregator
#[derive(Clone)]
pub struct MetricsAggregator {
    state: Arc<RwLock<AggregatorState>>,
}

struct AggregatorState {
    /// Latency histogram (in microseconds, up to 1 minute)
    latency_histogram: Histogram<u64>,

    /// Total operations
    total_operations: u64,

    /// Total errors
    total_errors: u64,

    /// Operations per bot
    bot_operations: HashMap<String, u64>,

    /// Start time for TPS calculation
    start_time_ms: i64,

    /// Window start for rolling metrics
    window_start_ms: i64,

    /// Operations in current window
    window_operations: u64,

    /// Active bots
    active_bots: HashMap<String, i64>,  // bot_id -> last_seen_ms
}

impl MetricsAggregator {
    /// Create a new metrics aggregator
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(AggregatorState {
                // HDR Histogram: 1 microsecond to 60 seconds, 3 significant digits
                latency_histogram: Histogram::new_with_bounds(1, 60_000_000, 3)
                    .expect("Failed to create histogram"),
                total_operations: 0,
                total_errors: 0,
                bot_operations: HashMap::new(),
                start_time_ms: current_time_ms(),
                window_start_ms: current_time_ms(),
                window_operations: 0,
                active_bots: HashMap::new(),
            })),
        }
    }

    /// Process a single event
    pub fn process_event(&self, event: &BotEvent) {
        let mut state = self.state.write();

        match event {
            BotEvent::BotStarted { bot_id, timestamp_ms, .. } => {
                state.active_bots.insert(bot_id.clone(), *timestamp_ms);
            }

            BotEvent::BotStopped { bot_id, .. } => {
                state.active_bots.remove(bot_id);
            }

            BotEvent::BehaviorCompleted {
                bot_id,
                duration_ms,
                success,
                ..
            } => {
                state.total_operations += 1;
                state.window_operations += 1;

                *state.bot_operations.entry(bot_id.clone()).or_insert(0) += 1;

                if !success {
                    state.total_errors += 1;
                }

                // Record latency in microseconds
                let _ = state.latency_histogram.record(*duration_ms * 1000);
            }

            BotEvent::TransactionConfirmed {
                confirmation_time_ms,
                ..
            } => {
                state.total_operations += 1;
                state.window_operations += 1;
                let _ = state.latency_histogram.record(*confirmation_time_ms * 1000);
            }

            BotEvent::NetworkResponse { latency_ms, .. } => {
                let _ = state.latency_histogram.record(*latency_ms * 1000);
            }

            BotEvent::BotError { .. } | BotEvent::TransactionFailed { .. } => {
                state.total_errors += 1;
            }

            _ => {}
        }
    }

    /// Process a batch of events
    pub fn process_batch(&self, events: &[BotEvent]) {
        for event in events {
            self.process_event(event);
        }
    }

    /// Get current TPS (transactions per second)
    pub fn tps(&self) -> f64 {
        let state = self.state.read();
        let elapsed_ms = current_time_ms() - state.start_time_ms;

        if elapsed_ms == 0 {
            return 0.0;
        }

        (state.total_operations as f64) / (elapsed_ms as f64 / 1000.0)
    }

    /// Get TPS for a rolling window (last N milliseconds)
    pub fn window_tps(&self, window_ms: i64) -> f64 {
        let mut state = self.state.write();
        let now = current_time_ms();

        // Reset window if expired
        if now - state.window_start_ms > window_ms {
            state.window_start_ms = now;
            state.window_operations = 0;
        }

        let elapsed = now - state.window_start_ms;
        if elapsed == 0 {
            return 0.0;
        }

        (state.window_operations as f64) / (elapsed as f64 / 1000.0)
    }

    /// Get latency percentile (in milliseconds)
    pub fn latency_percentile(&self, percentile: f64) -> f64 {
        let state = self.state.read();

        if state.latency_histogram.len() == 0 {
            return 0.0;
        }

        let value_us = state.latency_histogram.value_at_quantile(percentile);
        value_us as f64 / 1000.0  // Convert to milliseconds
    }

    /// Get p50 latency
    pub fn latency_p50(&self) -> f64 {
        self.latency_percentile(0.50)
    }

    /// Get p95 latency
    pub fn latency_p95(&self) -> f64 {
        self.latency_percentile(0.95)
    }

    /// Get p99 latency
    pub fn latency_p99(&self) -> f64 {
        self.latency_percentile(0.99)
    }

    /// Get error rate (errors / total operations)
    pub fn error_rate(&self) -> f64 {
        let state = self.state.read();

        if state.total_operations == 0 {
            return 0.0;
        }

        (state.total_errors as f64) / (state.total_operations as f64)
    }

    /// Get total operations
    pub fn total_operations(&self) -> u64 {
        self.state.read().total_operations
    }

    /// Get total errors
    pub fn total_errors(&self) -> u64 {
        self.state.read().total_errors
    }

    /// Get active bot count
    pub fn active_bot_count(&self) -> usize {
        self.state.read().active_bots.len()
    }

    /// Get operations per bot
    pub fn bot_operations(&self) -> HashMap<String, u64> {
        self.state.read().bot_operations.clone()
    }

    /// Get full metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp_ms: current_time_ms(),
            tps: self.tps(),
            latency_p50_ms: self.latency_p50(),
            latency_p95_ms: self.latency_p95(),
            latency_p99_ms: self.latency_p99(),
            error_rate: self.error_rate(),
            total_operations: self.total_operations(),
            total_errors: self.total_errors(),
            active_bots: self.active_bot_count(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        let mut state = self.state.write();

        state.latency_histogram.clear();
        state.total_operations = 0;
        state.total_errors = 0;
        state.bot_operations.clear();
        state.start_time_ms = current_time_ms();
        state.window_start_ms = current_time_ms();
        state.window_operations = 0;
    }
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics snapshot at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub timestamp_ms: i64,
    pub tps: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub error_rate: f64,
    pub total_operations: u64,
    pub total_errors: u64,
    pub active_bots: usize,
}

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
    fn test_metrics_aggregation() {
        let aggregator = MetricsAggregator::new();

        // Process some events
        aggregator.process_event(&BotEvent::BehaviorCompleted {
            bot_id: "bot-1".to_string(),
            behavior_id: "test".to_string(),
            timestamp_ms: current_time_ms(),
            duration_ms: 100,
            success: true,
        });

        assert_eq!(aggregator.total_operations(), 1);
        assert_eq!(aggregator.total_errors(), 0);
    }

    #[test]
    fn test_error_rate() {
        let aggregator = MetricsAggregator::new();

        aggregator.process_event(&BotEvent::BehaviorCompleted {
            bot_id: "bot-1".to_string(),
            behavior_id: "test".to_string(),
            timestamp_ms: current_time_ms(),
            duration_ms: 100,
            success: true,
        });

        aggregator.process_event(&BotEvent::BehaviorCompleted {
            bot_id: "bot-1".to_string(),
            behavior_id: "test".to_string(),
            timestamp_ms: current_time_ms(),
            duration_ms: 100,
            success: false,
        });

        assert_eq!(aggregator.error_rate(), 0.5);
    }

    #[test]
    fn test_active_bots() {
        let aggregator = MetricsAggregator::new();

        aggregator.process_event(&BotEvent::BotStarted {
            bot_id: "bot-1".to_string(),
            role: "trader".to_string(),
            timestamp_ms: current_time_ms(),
        });

        aggregator.process_event(&BotEvent::BotStarted {
            bot_id: "bot-2".to_string(),
            role: "trader".to_string(),
            timestamp_ms: current_time_ms(),
        });

        assert_eq!(aggregator.active_bot_count(), 2);

        aggregator.process_event(&BotEvent::BotStopped {
            bot_id: "bot-1".to_string(),
            timestamp_ms: current_time_ms(),
            reason: "test".to_string(),
        });

        assert_eq!(aggregator.active_bot_count(), 1);
    }
}
