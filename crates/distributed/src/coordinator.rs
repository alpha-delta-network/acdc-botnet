/// Coordinator server for distributed bot orchestration
///
/// Manages worker registration, bot distribution, and metrics aggregation
use crate::proto::{
    bot_orchestration_server::{BotOrchestration, BotOrchestrationServer},
    *,
};
use crate::registry::WorkerRegistry;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, warn};

/// Aggregated metrics across all workers
#[derive(Debug, Default, Clone)]
struct AggregatedMetrics {
    total_active_bots: u32,
    per_worker: HashMap<String, WorkerMetrics>,
}

/// Coordinator for distributed bot orchestration
pub struct Coordinator {
    registry: Arc<WorkerRegistry>,
    metrics_tx: mpsc::Sender<WorkerMetrics>,
    /// Aggregated metrics storage
    aggregated_metrics: Arc<RwLock<AggregatedMetrics>>,
    /// Pending directives for each worker: worker_id -> queue of directives
    pending_directives: Arc<RwLock<HashMap<String, Vec<CoordinatorDirective>>>>,
    /// Bot-to-worker mapping: bot_id -> worker_id
    bot_assignments: Arc<RwLock<HashMap<String, String>>>,
}

impl Coordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        let (metrics_tx, mut metrics_rx) = mpsc::channel::<WorkerMetrics>(1000);

        let aggregated_metrics = Arc::new(RwLock::new(AggregatedMetrics::default()));
        let agg_clone = Arc::clone(&aggregated_metrics);

        // Spawn metrics processor — aggregates and stores metrics from all workers
        tokio::spawn(async move {
            while let Some(metrics) = metrics_rx.recv().await {
                let worker_id = metrics.worker_id.clone();
                let active_bots = metrics.active_bots;
                tracing::debug!("Received metrics from worker {}: {} active bots", worker_id, active_bots);

                let mut agg = agg_clone.write().await;
                // Update per-worker entry
                let old_bots = agg.per_worker.get(&worker_id).map(|m| m.active_bots).unwrap_or(0);
                agg.per_worker.insert(worker_id, metrics);
                // Adjust total
                agg.total_active_bots = agg.total_active_bots.saturating_sub(old_bots) + active_bots;
            }
        });

        Self {
            registry: Arc::new(WorkerRegistry::new()),
            metrics_tx,
            aggregated_metrics,
            pending_directives: Arc::new(RwLock::new(HashMap::new())),
            bot_assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the coordinator server
    pub async fn serve(self, addr: String) -> Result<()> {
        let addr = addr.parse()?;

        info!("Starting coordinator on {}", addr);

        let orchestration_service = BotOrchestrationServer::new(self);

        Server::builder()
            .add_service(orchestration_service)
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Get worker count
    pub fn worker_count(&self) -> usize {
        self.registry.worker_count()
    }

    /// Get total bot capacity
    pub fn total_capacity(&self) -> u32 {
        self.registry.total_capacity()
    }

    /// Queue a directive for a specific worker (delivered on next heartbeat)
    async fn queue_directive(&self, worker_id: &str, directive: CoordinatorDirective) {
        let mut directives = self.pending_directives.write().await;
        directives
            .entry(worker_id.to_string())
            .or_default()
            .push(directive);
    }

    /// Pop the next pending directive for a worker (called during heartbeat)
    async fn pop_directive(&self, worker_id: &str) -> CoordinatorDirective {
        let mut directives = self.pending_directives.write().await;
        if let Some(queue) = directives.get_mut(worker_id) {
            if !queue.is_empty() {
                return queue.remove(0);
            }
        }
        // Default: continue
        CoordinatorDirective {
            action: coordinator_directive::Action::Continue as i32,
            spawn_specs: vec![],
            stop_bot_ids: vec![],
        }
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl BotOrchestration for Coordinator {
    async fn register_worker(
        &self,
        request: Request<WorkerInfo>,
    ) -> Result<Response<WorkerAck>, Status> {
        let worker_info = request.into_inner();

        info!(
            "Registering worker: {} (capacity: {})",
            worker_info.worker_id, worker_info.max_bots
        );

        self.registry.register(worker_info).map_err(|e| {
            warn!("Failed to register worker: {}", e);
            Status::internal(format!("Registration failed: {}", e))
        })?;

        Ok(Response::new(WorkerAck {
            success: true,
            message: "Worker registered successfully".to_string(),
            coordinator_id: "coordinator-1".to_string(),
        }))
    }

    async fn spawn_bot(&self, request: Request<BotSpec>) -> Result<Response<BotHandle>, Status> {
        let bot_spec = request.into_inner();

        info!("Spawning bot: {}", bot_spec.bot_id);

        // Find suitable worker
        let worker_id = self
            .registry
            .find_available_worker()
            .ok_or_else(|| Status::resource_exhausted("No available workers"))?;

        // Record bot->worker assignment
        self.bot_assignments
            .write()
            .await
            .insert(bot_spec.bot_id.clone(), worker_id.clone());

        // Queue a SpawnBots directive for the chosen worker (delivered on next heartbeat)
        self.queue_directive(
            &worker_id,
            CoordinatorDirective {
                action: coordinator_directive::Action::SpawnBots as i32,
                spawn_specs: vec![bot_spec.clone()],
                stop_bot_ids: vec![],
            },
        )
        .await;

        info!("Queued spawn directive for bot {} on worker {}", bot_spec.bot_id, worker_id);

        Ok(Response::new(BotHandle {
            bot_id: bot_spec.bot_id,
            worker_id,
            success: true,
            message: "Bot spawn queued".to_string(),
        }))
    }

    async fn stop_bot(&self, request: Request<BotId>) -> Result<Response<StopAck>, Status> {
        let bot_id = request.into_inner();

        info!("Stopping bot: {}", bot_id.bot_id);

        // Look up which worker hosts this bot
        let assignments = self.bot_assignments.read().await;
        if let Some(worker_id) = assignments.get(&bot_id.bot_id) {
            let worker_id = worker_id.clone();
            drop(assignments);

            // Queue a StopBots directive for the worker
            self.queue_directive(
                &worker_id,
                CoordinatorDirective {
                    action: coordinator_directive::Action::StopBots as i32,
                    spawn_specs: vec![],
                    stop_bot_ids: vec![bot_id.bot_id.clone()],
                },
            )
            .await;

            info!("Queued stop directive for bot {} on worker {}", bot_id.bot_id, worker_id);
        } else {
            warn!("stop_bot: bot {} not found in assignments", bot_id.bot_id);
        }

        Ok(Response::new(StopAck {
            success: true,
            message: "Bot stop queued".to_string(),
        }))
    }

    async fn get_bot_status(&self, request: Request<BotId>) -> Result<Response<BotStatus>, Status> {
        let bot_id = request.into_inner();

        let assignments = self.bot_assignments.read().await;
        let worker_id = assignments
            .get(&bot_id.bot_id)
            .cloned()
            .unwrap_or_default();

        Ok(Response::new(BotStatus {
            bot_id: bot_id.bot_id,
            status: if worker_id.is_empty() { "unknown".to_string() } else { "running".to_string() },
            message: if worker_id.is_empty() { "Bot not found".to_string() } else { format!("On worker {}", worker_id) },
            uptime_ms: 0,
            operations_count: 0,
        }))
    }

    async fn stream_metrics(
        &self,
        request: Request<tonic::Streaming<WorkerMetrics>>,
    ) -> Result<Response<Empty>, Status> {
        let mut stream = request.into_inner();
        let metrics_tx = self.metrics_tx.clone();

        tokio::spawn(async move {
            while let Ok(Some(metrics)) = stream.message().await {
                if metrics_tx.send(metrics).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(Empty {}))
    }

    async fn heartbeat(
        &self,
        request: Request<WorkerHealth>,
    ) -> Result<Response<CoordinatorDirective>, Status> {
        let health = request.into_inner();

        self.registry
            .update_heartbeat(&health.worker_id, health.healthy)
            .map_err(|e| Status::internal(format!("Heartbeat failed: {}", e)))?;

        // Forward metrics to aggregator
        let metrics = WorkerMetrics {
            worker_id: health.worker_id.clone(),
            active_bots: health.active_bots,
            bot_metrics: vec![],
            cpu_usage_percent: 0,
            memory_usage_bytes: 0,
            timestamp_ms: health.timestamp_ms,
        };
        let _ = self.metrics_tx.try_send(metrics);

        // Return the next queued directive (or CONTINUE if none pending)
        let directive = self.pop_directive(&health.worker_id).await;
        Ok(Response::new(directive))
    }

    async fn distribute_scenario(
        &self,
        request: Request<ScenarioSpec>,
    ) -> Result<Response<DistributionPlan>, Status> {
        let scenario = request.into_inner();

        info!("Distributing scenario: {}", scenario.scenario_id);

        // Get available workers
        let workers = self.registry.list_workers();

        if workers.is_empty() {
            return Err(Status::unavailable("No workers available"));
        }

        // Simple distribution: round-robin
        let mut assignments: Vec<WorkerAssignment> = Vec::new();
        let mut worker_idx = 0;

        for bot_spec in scenario.bot_specs {
            let worker = &workers[worker_idx % workers.len()];

            // Track bot->worker assignment
            self.bot_assignments
                .write()
                .await
                .insert(bot_spec.bot_id.clone(), worker.worker_id.clone());

            // Find or create assignment for this worker
            if let Some(assignment) = assignments
                .iter_mut()
                .find(|a| a.worker_id == worker.worker_id)
            {
                assignment.bot_specs.push(bot_spec);
            } else {
                assignments.push(WorkerAssignment {
                    worker_id: worker.worker_id.clone(),
                    bot_specs: vec![bot_spec],
                });
            }

            worker_idx += 1;
        }

        // Queue SpawnBots directives for each worker
        for assignment in &assignments {
            self.queue_directive(
                &assignment.worker_id,
                CoordinatorDirective {
                    action: coordinator_directive::Action::SpawnBots as i32,
                    spawn_specs: assignment.bot_specs.clone(),
                    stop_bot_ids: vec![],
                },
            )
            .await;
        }

        let worker_count = assignments.len();
        Ok(Response::new(DistributionPlan {
            scenario_id: scenario.scenario_id,
            assignments,
            success: true,
            message: format!("Distributed to {} workers", worker_count),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = Coordinator::new();
        assert_eq!(coordinator.worker_count(), 0);
    }

    #[tokio::test]
    async fn test_pending_directives() {
        let coordinator = Coordinator::new();

        // No pending directives → Continue
        let d = coordinator.pop_directive("worker-1").await;
        assert_eq!(d.action, coordinator_directive::Action::Continue as i32);

        // Queue a directive
        coordinator
            .queue_directive(
                "worker-1",
                CoordinatorDirective {
                    action: coordinator_directive::Action::Shutdown as i32,
                    spawn_specs: vec![],
                    stop_bot_ids: vec![],
                },
            )
            .await;

        let d = coordinator.pop_directive("worker-1").await;
        assert_eq!(d.action, coordinator_directive::Action::Shutdown as i32);

        // Back to Continue
        let d = coordinator.pop_directive("worker-1").await;
        assert_eq!(d.action, coordinator_directive::Action::Continue as i32);
    }
}
