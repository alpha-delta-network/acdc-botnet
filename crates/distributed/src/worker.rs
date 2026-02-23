/// Worker daemon for distributed bot orchestration
///
/// Connects to coordinator, spawns bots locally, reports metrics

use crate::proto::{bot_orchestration_client::BotOrchestrationClient, *};
use anyhow::Result;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

/// Worker daemon
pub struct Worker {
    worker_id: String,
    coordinator_addr: String,
    max_bots: u32,
    cpu_cores: u32,
    memory_bytes: u64,
    capabilities: Vec<String>,
}

impl Worker {
    /// Create a new worker
    pub fn new(
        worker_id: String,
        coordinator_addr: String,
        max_bots: u32,
    ) -> Self {
        Self {
            worker_id,
            coordinator_addr,
            max_bots,
            cpu_cores: num_cpus::get() as u32,
            memory_bytes: get_memory_bytes(),
            capabilities: vec!["trader".to_string(), "user".to_string(), "governor".to_string()],
        }
    }

    /// Add a capability
    pub fn with_capability(mut self, capability: String) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Run the worker
    pub async fn run(self) -> Result<()> {
        info!(
            "Starting worker {} (capacity: {} bots)",
            self.worker_id, self.max_bots
        );

        // Connect to coordinator
        let mut client = BotOrchestrationClient::connect(self.coordinator_addr.clone()).await?;

        // Register with coordinator
        self.register(&mut client).await?;

        // Start heartbeat loop
        let mut heartbeat_interval = interval(Duration::from_secs(5));

        loop {
            heartbeat_interval.tick().await;

            match self.send_heartbeat(&mut client).await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Heartbeat failed: {}", e);
                    // Try to reconnect
                    if let Ok(new_client) =
                        BotOrchestrationClient::connect(self.coordinator_addr.clone()).await
                    {
                        client = new_client;
                        let _ = self.register(&mut client).await;
                    }
                }
            }
        }
    }

    /// Register with coordinator
    async fn register(
        &self,
        client: &mut BotOrchestrationClient<tonic::transport::Channel>,
    ) -> Result<()> {
        let request = tonic::Request::new(WorkerInfo {
            worker_id: self.worker_id.clone(),
            cpu_cores: self.cpu_cores,
            memory_bytes: self.memory_bytes,
            max_bots: self.max_bots,
            capabilities: self.capabilities.clone(),
            address: String::new(),
        });

        let response = client.register_worker(request).await?;
        let ack = response.into_inner();

        if ack.success {
            info!("Registered with coordinator: {}", ack.coordinator_id);
        } else {
            warn!("Registration failed: {}", ack.message);
        }

        Ok(())
    }

    /// Send heartbeat to coordinator
    async fn send_heartbeat(
        &self,
        client: &mut BotOrchestrationClient<tonic::transport::Channel>,
    ) -> Result<()> {
        let request = tonic::Request::new(WorkerHealth {
            worker_id: self.worker_id.clone(),
            healthy: true,
            active_bots: 0,  // TODO: Track actual bot count
            timestamp_ms: current_time_ms(),
        });

        let response = client.heartbeat(request).await?;
        let directive = response.into_inner();

        // Handle directive
        match coordinator_directive::Action::try_from(directive.action) {
            Ok(coordinator_directive::Action::Continue) => {}
            Ok(coordinator_directive::Action::Shutdown) => {
                info!("Received shutdown directive from coordinator");
                // TODO: Graceful shutdown
            }
            Ok(coordinator_directive::Action::SpawnBots) => {
                info!("Received spawn directive for {} bots", directive.spawn_specs.len());
                // TODO: Spawn bots
            }
            Ok(coordinator_directive::Action::StopBots) => {
                info!("Received stop directive for {} bots", directive.stop_bot_ids.len());
                // TODO: Stop bots
            }
            Err(_) => {
                warn!("Unknown directive action: {}", directive.action);
            }
        }

        Ok(())
    }
}

fn get_memory_bytes() -> u64 {
    // Simplified: return 8GB
    8 * 1024 * 1024 * 1024
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
    fn test_worker_creation() {
        let worker = Worker::new(
            "worker-1".to_string(),
            "http://localhost:50051".to_string(),
            100,
        );

        assert_eq!(worker.worker_id, "worker-1");
        assert_eq!(worker.max_bots, 100);
    }
}
