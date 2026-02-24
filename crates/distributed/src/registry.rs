/// Worker registry with health tracking
use crate::proto::WorkerInfo;
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Worker registration entry
#[derive(Debug, Clone)]
pub struct WorkerEntry {
    pub info: WorkerInfo,
    pub last_heartbeat_ms: i64,
    pub healthy: bool,
}

/// Worker registry
pub struct WorkerRegistry {
    workers: Arc<RwLock<HashMap<String, WorkerEntry>>>,
}

impl WorkerRegistry {
    /// Create a new worker registry
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a worker
    pub fn register(&self, info: WorkerInfo) -> Result<()> {
        let worker_id = info.worker_id.clone();

        let entry = WorkerEntry {
            info,
            last_heartbeat_ms: current_time_ms(),
            healthy: true,
        };

        self.workers.write().insert(worker_id, entry);

        Ok(())
    }

    /// Update worker heartbeat
    pub fn update_heartbeat(&self, worker_id: &str, healthy: bool) -> Result<()> {
        let mut workers = self.workers.write();

        let entry = workers.get_mut(worker_id).context("Worker not found")?;

        entry.last_heartbeat_ms = current_time_ms();
        entry.healthy = healthy;

        Ok(())
    }

    /// Check for stale workers and mark as unhealthy
    pub fn check_stale_workers(&self, timeout_ms: i64) {
        let now = current_time_ms();
        let mut workers = self.workers.write();

        for entry in workers.values_mut() {
            if now - entry.last_heartbeat_ms > timeout_ms {
                entry.healthy = false;
            }
        }
    }

    /// Get worker count
    pub fn worker_count(&self) -> usize {
        self.workers.read().len()
    }

    /// Get healthy worker count
    pub fn healthy_worker_count(&self) -> usize {
        self.workers.read().values().filter(|w| w.healthy).count()
    }

    /// Get total capacity across all workers
    pub fn total_capacity(&self) -> u32 {
        self.workers
            .read()
            .values()
            .filter(|w| w.healthy)
            .map(|w| w.info.max_bots)
            .sum()
    }

    /// Find an available worker
    pub fn find_available_worker(&self) -> Option<String> {
        self.workers
            .read()
            .iter()
            .find(|(_, entry)| entry.healthy)
            .map(|(id, _)| id.clone())
    }

    /// List all workers
    pub fn list_workers(&self) -> Vec<WorkerInfo> {
        self.workers
            .read()
            .values()
            .filter(|w| w.healthy)
            .map(|w| w.info.clone())
            .collect()
    }

    /// Get worker by ID
    pub fn get_worker(&self, worker_id: &str) -> Option<WorkerEntry> {
        self.workers.read().get(worker_id).cloned()
    }

    /// Remove a worker
    pub fn remove_worker(&self, worker_id: &str) -> Result<()> {
        self.workers
            .write()
            .remove(worker_id)
            .context("Worker not found")?;
        Ok(())
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    fn create_worker_info(id: &str, max_bots: u32) -> WorkerInfo {
        WorkerInfo {
            worker_id: id.to_string(),
            cpu_cores: 8,
            memory_bytes: 8 * 1024 * 1024 * 1024,
            max_bots,
            capabilities: vec!["trader".to_string()],
            address: "localhost:50051".to_string(),
        }
    }

    #[test]
    fn test_worker_registration() {
        let registry = WorkerRegistry::new();

        let worker = create_worker_info("worker-1", 100);
        registry.register(worker).unwrap();

        assert_eq!(registry.worker_count(), 1);
        assert_eq!(registry.healthy_worker_count(), 1);
        assert_eq!(registry.total_capacity(), 100);
    }

    #[test]
    fn test_heartbeat_update() {
        let registry = WorkerRegistry::new();

        let worker = create_worker_info("worker-1", 100);
        registry.register(worker).unwrap();

        registry.update_heartbeat("worker-1", true).unwrap();

        let entry = registry.get_worker("worker-1").unwrap();
        assert!(entry.healthy);
    }

    #[test]
    fn test_find_available_worker() {
        let registry = WorkerRegistry::new();

        let worker1 = create_worker_info("worker-1", 100);
        let worker2 = create_worker_info("worker-2", 100);

        registry.register(worker1).unwrap();
        registry.register(worker2).unwrap();

        let available = registry.find_available_worker();
        assert!(available.is_some());
    }

    #[test]
    fn test_remove_worker() {
        let registry = WorkerRegistry::new();

        let worker = create_worker_info("worker-1", 100);
        registry.register(worker).unwrap();

        assert_eq!(registry.worker_count(), 1);

        registry.remove_worker("worker-1").unwrap();

        assert_eq!(registry.worker_count(), 0);
    }
}
