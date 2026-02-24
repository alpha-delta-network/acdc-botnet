/// Fault tolerance for distributed bot orchestration
use crate::proto::*;
use crate::registry::WorkerRegistry;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

/// Fault detector monitors worker health
pub struct FaultDetector {
    registry: Arc<WorkerRegistry>,
    check_interval: Duration,
    heartbeat_timeout: Duration,
}

impl FaultDetector {
    pub fn new(registry: Arc<WorkerRegistry>) -> Self {
        Self {
            registry,
            check_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(15), // 3 missed heartbeats
        }
    }

    /// Start fault detection loop
    pub async fn run(&self) {
        let mut ticker = interval(self.check_interval);

        loop {
            ticker.tick().await;
            self.check_stale_workers();
        }
    }

    /// Check for stale workers and mark as unhealthy
    fn check_stale_workers(&self) {
        let timeout_ms = self.heartbeat_timeout.as_millis() as i64;
        self.registry.check_stale_workers(timeout_ms);

        // Log any unhealthy workers
        let unhealthy_count = self.registry.worker_count() - self.registry.healthy_worker_count();
        if unhealthy_count > 0 {
            warn!("Detected {} unhealthy workers", unhealthy_count);
        }
    }
}

/// Bot migration handles moving bots from failed workers
pub struct BotMigration {
    registry: Arc<WorkerRegistry>,
}

impl BotMigration {
    pub fn new(registry: Arc<WorkerRegistry>) -> Self {
        Self { registry }
    }

    /// Migrate bots from failed worker to healthy workers
    pub async fn migrate_bots_from_worker(&self, failed_worker_id: &str) -> Result<Vec<BotHandle>> {
        info!("Migrating bots from failed worker: {}", failed_worker_id);

        // TODO: Track which bots were on the failed worker
        // For now, return empty list
        let migrated = Vec::new();

        info!("Migrated {} bots from failed worker", migrated.len());

        Ok(migrated)
    }

    /// Distribute bots across healthy workers
    pub async fn redistribute_bots(&self, bot_specs: Vec<BotSpec>) -> Result<Vec<BotHandle>> {
        let healthy_workers = self.registry.list_workers();

        if healthy_workers.is_empty() {
            anyhow::bail!("No healthy workers available for redistribution");
        }

        info!(
            "Redistributing {} bots across {} healthy workers",
            bot_specs.len(),
            healthy_workers.len()
        );

        let mut handles = Vec::new();
        let mut worker_idx = 0;

        for bot_spec in bot_specs {
            let worker = &healthy_workers[worker_idx % healthy_workers.len()];

            // TODO: Send spawn request to worker
            handles.push(BotHandle {
                bot_id: bot_spec.bot_id.clone(),
                worker_id: worker.worker_id.clone(),
                success: true,
                message: "Bot migrated".to_string(),
            });

            worker_idx += 1;
        }

        Ok(handles)
    }
}

/// Metrics buffering for workers when coordinator is unreachable
pub struct MetricsBuffer {
    buffer: Arc<parking_lot::RwLock<Vec<WorkerMetrics>>>,
    max_buffer_size: usize,
}

impl MetricsBuffer {
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            buffer: Arc::new(parking_lot::RwLock::new(Vec::new())),
            max_buffer_size,
        }
    }

    /// Buffer metrics locally
    pub fn buffer(&self, metrics: WorkerMetrics) {
        let mut buffer = self.buffer.write();

        if buffer.len() >= self.max_buffer_size {
            // Remove oldest
            buffer.remove(0);
        }

        buffer.push(metrics);
    }

    /// Flush buffered metrics to coordinator
    pub fn flush(&self) -> Vec<WorkerMetrics> {
        let mut buffer = self.buffer.write();
        std::mem::take(&mut *buffer)
    }

    /// Get buffer size
    pub fn size(&self) -> usize {
        self.buffer.read().len()
    }
}

/// Coordinator state checkpointing
pub struct CoordinatorCheckpoint {
    checkpoint_path: String,
}

impl CoordinatorCheckpoint {
    pub fn new(checkpoint_path: String) -> Self {
        Self { checkpoint_path }
    }

    /// Save coordinator state to disk
    pub async fn save(&self, state: &CoordinatorState) -> Result<()> {
        info!("Saving coordinator checkpoint to {}", self.checkpoint_path);

        let json = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&self.checkpoint_path, json).await?;

        Ok(())
    }

    /// Load coordinator state from disk
    pub async fn load(&self) -> Result<CoordinatorState> {
        info!(
            "Loading coordinator checkpoint from {}",
            self.checkpoint_path
        );

        let json = tokio::fs::read_to_string(&self.checkpoint_path).await?;
        let state = serde_json::from_str(&json)?;

        Ok(state)
    }
}

/// Coordinator state for checkpointing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoordinatorState {
    pub worker_count: usize,
    pub total_bots: usize,
    pub active_scenarios: Vec<String>,
    pub checkpoint_timestamp: i64,
}

impl CoordinatorState {
    pub fn new() -> Self {
        Self {
            worker_count: 0,
            total_bots: 0,
            active_scenarios: Vec::new(),
            checkpoint_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }
}

impl Default for CoordinatorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_buffer() {
        let buffer = MetricsBuffer::new(5);

        // Buffer some metrics
        for i in 0..7 {
            buffer.buffer(WorkerMetrics {
                worker_id: format!("worker-{}", i),
                active_bots: i as u32,
                bot_metrics: vec![],
                cpu_usage_percent: 50,
                memory_usage_bytes: 1000,
                timestamp_ms: i as i64,
            });
        }

        // Should only keep 5 (max buffer size)
        assert_eq!(buffer.size(), 5);

        // Flush should return all and clear
        let flushed = buffer.flush();
        assert_eq!(flushed.len(), 5);
        assert_eq!(buffer.size(), 0);
    }
}
