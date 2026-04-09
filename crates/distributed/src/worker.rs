/// Worker daemon for distributed bot orchestration
///
/// Connects to coordinator, spawns bots locally, reports metrics
use crate::proto::{bot_orchestration_client::BotOrchestrationClient, *};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{info, warn};

/// Handle to a running bot task
struct BotTaskHandle {
    bot_id: String,
    behavior: String,
    cancel_tx: mpsc::Sender<()>,
}

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
    pub fn new(worker_id: String, coordinator_addr: String, max_bots: u32) -> Self {
        Self {
            worker_id,
            coordinator_addr,
            max_bots,
            cpu_cores: num_cpus::get() as u32,
            memory_bytes: get_memory_bytes(),
            capabilities: vec![
                "trader".to_string(),
                "user".to_string(),
                "governor".to_string(),
            ],
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

        // Shared state
        let active_bots = Arc::new(AtomicUsize::new(0));
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        // Map from bot_id -> cancel sender
        let bot_handles: Arc<RwLock<HashMap<String, mpsc::Sender<()>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Start heartbeat loop
        let mut heartbeat_interval = interval(Duration::from_secs(5));

        loop {
            if shutdown_flag.load(Ordering::Relaxed) {
                info!("Worker {} shutting down — waiting for {} active bots", self.worker_id, active_bots.load(Ordering::Relaxed));
                // Wait for running bots to complete (poll with backoff)
                let mut waited = 0u32;
                while active_bots.load(Ordering::Relaxed) > 0 && waited < 30 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    waited += 1;
                }
                info!("Worker {} shutdown complete", self.worker_id);
                return Ok(());
            }

            heartbeat_interval.tick().await;

            let current_active = active_bots.load(Ordering::Relaxed) as u32;

            match self.send_heartbeat(&mut client, current_active).await {
                Ok(directive) => {
                    self.handle_directive(
                        directive,
                        Arc::clone(&active_bots),
                        Arc::clone(&shutdown_flag),
                        Arc::clone(&bot_handles),
                    )
                    .await;
                }
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

    /// Send heartbeat to coordinator, returns the directive received
    async fn send_heartbeat(
        &self,
        client: &mut BotOrchestrationClient<tonic::transport::Channel>,
        active_bots: u32,
    ) -> Result<CoordinatorDirective> {
        let request = tonic::Request::new(WorkerHealth {
            worker_id: self.worker_id.clone(),
            healthy: true,
            active_bots,
            timestamp_ms: current_time_ms(),
        });

        let response = client.heartbeat(request).await?;
        Ok(response.into_inner())
    }

    /// Handle a directive from the coordinator
    async fn handle_directive(
        &self,
        directive: CoordinatorDirective,
        active_bots: Arc<AtomicUsize>,
        shutdown_flag: Arc<AtomicBool>,
        bot_handles: Arc<RwLock<HashMap<String, mpsc::Sender<()>>>>,
    ) {
        match coordinator_directive::Action::try_from(directive.action) {
            Ok(coordinator_directive::Action::Continue) => {}
            Ok(coordinator_directive::Action::Shutdown) => {
                info!(
                    "Worker {} received Shutdown directive — draining bots",
                    self.worker_id
                );
                // Signal all running bots to stop
                let handles = bot_handles.read().await;
                for (bot_id, cancel_tx) in handles.iter() {
                    if let Err(e) = cancel_tx.try_send(()) {
                        warn!("Could not cancel bot {}: {}", bot_id, e);
                    }
                }
                // Set shutdown flag — run() loop will drain and exit
                shutdown_flag.store(true, Ordering::Relaxed);
            }
            Ok(coordinator_directive::Action::SpawnBots) => {
                info!(
                    "Worker {} received SpawnBots directive: {} specs",
                    self.worker_id,
                    directive.spawn_specs.len()
                );
                for spec in directive.spawn_specs {
                    let bot_id = spec.bot_id.clone();
                    let behavior = spec.behavior.clone();
                    let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);

                    let active_bots_clone = Arc::clone(&active_bots);
                    let bot_handles_clone = Arc::clone(&bot_handles);
                    let worker_id = self.worker_id.clone();

                    bot_handles.write().await.insert(bot_id.clone(), cancel_tx);
                    active_bots.fetch_add(1, Ordering::Relaxed);

                    tokio::spawn(async move {
                        info!(
                            "Worker {} spawned bot {} behavior={}",
                            worker_id, bot_id, behavior
                        );
                        // Run bot until cancelled or a simulated completion
                        tokio::select! {
                            _ = cancel_rx.recv() => {
                                info!("Bot {} cancelled by worker {}", bot_id, worker_id);
                            }
                            _ = tokio::time::sleep(Duration::from_secs(300)) => {
                                info!("Bot {} completed (timeout) on worker {}", bot_id, worker_id);
                            }
                        }
                        active_bots_clone.fetch_sub(1, Ordering::Relaxed);
                        bot_handles_clone.write().await.remove(&bot_id);
                    });
                }
            }
            Ok(coordinator_directive::Action::StopBots) => {
                info!(
                    "Worker {} received StopBots directive: {} bot ids",
                    self.worker_id,
                    directive.stop_bot_ids.len()
                );
                let handles = bot_handles.read().await;
                for bot_id in &directive.stop_bot_ids {
                    if let Some(cancel_tx) = handles.get(bot_id) {
                        if let Err(e) = cancel_tx.try_send(()) {
                            warn!("Could not stop bot {}: {}", bot_id, e);
                        } else {
                            info!("Sent stop signal to bot {}", bot_id);
                        }
                    } else {
                        warn!("StopBots: bot {} not found on this worker", bot_id);
                    }
                }
            }
            Err(_) => {
                warn!("Unknown directive action: {}", directive.action);
            }
        }
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
