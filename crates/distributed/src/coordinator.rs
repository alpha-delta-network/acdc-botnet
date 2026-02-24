/// Coordinator server for distributed bot orchestration
///
/// Manages worker registration, bot distribution, and metrics aggregation
use crate::proto::{
    bot_orchestration_server::{BotOrchestration, BotOrchestrationServer},
    *,
};
use crate::registry::WorkerRegistry;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, warn};

/// Coordinator for distributed bot orchestration
pub struct Coordinator {
    registry: Arc<WorkerRegistry>,
    metrics_tx: mpsc::Sender<WorkerMetrics>,
}

impl Coordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        let (metrics_tx, mut metrics_rx) = mpsc::channel::<WorkerMetrics>(1000);

        // Spawn metrics processor
        tokio::spawn(async move {
            while let Some(metrics) = metrics_rx.recv().await {
                // TODO: Aggregate and store metrics
                tracing::debug!("Received metrics from worker: {}", metrics.worker_id);
            }
        });

        Self {
            registry: Arc::new(WorkerRegistry::new()),
            metrics_tx,
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

        // TODO: Send spawn request to worker via gRPC
        // For now, just return success

        Ok(Response::new(BotHandle {
            bot_id: bot_spec.bot_id,
            worker_id,
            success: true,
            message: "Bot spawned successfully".to_string(),
        }))
    }

    async fn stop_bot(&self, request: Request<BotId>) -> Result<Response<StopAck>, Status> {
        let bot_id = request.into_inner();

        info!("Stopping bot: {}", bot_id.bot_id);

        // TODO: Send stop request to worker

        Ok(Response::new(StopAck {
            success: true,
            message: "Bot stopped successfully".to_string(),
        }))
    }

    async fn get_bot_status(&self, request: Request<BotId>) -> Result<Response<BotStatus>, Status> {
        let bot_id = request.into_inner();

        // TODO: Query worker for bot status

        Ok(Response::new(BotStatus {
            bot_id: bot_id.bot_id,
            status: "running".to_string(),
            message: String::new(),
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

        // Return directive (usually CONTINUE)
        Ok(Response::new(CoordinatorDirective {
            action: coordinator_directive::Action::Continue as i32,
            spawn_specs: vec![],
            stop_bot_ids: vec![],
        }))
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

        Ok(Response::new(DistributionPlan {
            scenario_id: scenario.scenario_id,
            assignments,
            success: true,
            message: format!("Distributed to {} workers", assignments.len()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_creation() {
        let coordinator = Coordinator::new();
        assert_eq!(coordinator.worker_count(), 0);
    }
}
