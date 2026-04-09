/// Fault tolerance for distributed bot orchestration
use crate::proto::*;
use crate::registry::WorkerRegistry;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
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

/// Tracks which bots are assigned to which workers for fault recovery
pub struct BotAssignmentTracker {
    /// Maps worker_id -> list of (bot_id, bot_spec)
    assignments: Arc<RwLock<HashMap<String, Vec<BotSpec>>>>,
}

impl BotAssignmentTracker {
    pub fn new() -> Self {
        Self {
            assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record that a bot has been assigned to a worker
    pub async fn record_assignment(&self, worker_id: &str, bot_spec: BotSpec) {
        let mut assignments = self.assignments.write().await;
        assignments
            .entry(worker_id.to_string())
            .or_default()
            .push(bot_spec);
        tracing::debug!("Tracked bot {} on worker {}", {
            let specs = assignments.get(worker_id).unwrap();
            specs.last().map(|s| s.bot_id.clone()).unwrap_or_default()
        }, worker_id);
    }

    /// Remove a bot assignment (called when a bot completes or stops)
    pub async fn remove_assignment(&self, worker_id: &str, bot_id: &str) {
        let mut assignments = self.assignments.write().await;
        if let Some(bots) = assignments.get_mut(worker_id) {
            bots.retain(|s| s.bot_id != bot_id);
        }
    }

    /// Get all bot specs assigned to a worker (for recovery)
    pub async fn get_worker_bots(&self, worker_id: &str) -> Vec<BotSpec> {
        let assignments = self.assignments.read().await;
        assignments.get(worker_id).cloned().unwrap_or_default()
    }

    /// Remove all assignments for a worker (called after migration)
    pub async fn clear_worker(&self, worker_id: &str) {
        let mut assignments = self.assignments.write().await;
        assignments.remove(worker_id);
    }

    /// Get total tracked bot count
    pub async fn total_bots(&self) -> usize {
        let assignments = self.assignments.read().await;
        assignments.values().map(|v| v.len()).sum()
    }
}

impl Default for BotAssignmentTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Bot migration handles moving bots from failed workers
pub struct BotMigration {
    registry: Arc<WorkerRegistry>,
    tracker: Arc<BotAssignmentTracker>,
}

impl BotMigration {
    pub fn new(registry: Arc<WorkerRegistry>) -> Self {
        Self {
            registry,
            tracker: Arc::new(BotAssignmentTracker::new()),
        }
    }

    pub fn with_tracker(registry: Arc<WorkerRegistry>, tracker: Arc<BotAssignmentTracker>) -> Self {
        Self { registry, tracker }
    }

    /// Get access to the assignment tracker (for recording new assignments)
    pub fn tracker(&self) -> Arc<BotAssignmentTracker> {
        Arc::clone(&self.tracker)
    }

    /// Migrate bots from failed worker to healthy workers
    pub async fn migrate_bots_from_worker(&self, failed_worker_id: &str) -> Result<Vec<BotHandle>> {
        info!("Migrating bots from failed worker: {}", failed_worker_id);

        // Retrieve which bots were on the failed worker
        let bot_specs = self.tracker.get_worker_bots(failed_worker_id).await;
        if bot_specs.is_empty() {
            info!("No bots tracked on failed worker {}", failed_worker_id);
            return Ok(vec![]);
        }

        info!(
            "Found {} bots to migrate from worker {}",
            bot_specs.len(),
            failed_worker_id
        );

        // Clear tracking for failed worker
        self.tracker.clear_worker(failed_worker_id).await;

        // Redistribute to healthy workers
        let migrated = self.redistribute_bots(bot_specs).await?;

        info!(
            "Migrated {} bots from failed worker {}",
            migrated.len(),
            failed_worker_id
        );

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

            // Track the new assignment
            self.tracker
                .record_assignment(&worker.worker_id, bot_spec.clone())
                .await;

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

    #[tokio::test]
    async fn test_bot_assignment_tracker() {
        let tracker = BotAssignmentTracker::new();

        let spec = BotSpec {
            bot_id: "bot-1".to_string(),
            role: "user".to_string(),
            role_variant: String::new(),
            behavior: "submit_tx".to_string(),
            config: vec![],
            target_network: String::new(),
            tags: vec![],
        };

        tracker.record_assignment("worker-1", spec.clone()).await;
        assert_eq!(tracker.total_bots().await, 1);

        let bots = tracker.get_worker_bots("worker-1").await;
        assert_eq!(bots.len(), 1);
        assert_eq!(bots[0].bot_id, "bot-1");

        tracker.remove_assignment("worker-1", "bot-1").await;
        assert_eq!(tracker.total_bots().await, 0);
    }
}
