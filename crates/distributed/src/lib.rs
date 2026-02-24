/// Distributed bot orchestration
///
/// Provides gRPC-based coordinator/worker architecture
pub mod coordinator;
pub mod fault_tolerance;
pub mod protocol;
pub mod registry;
pub mod worker;

pub use coordinator::Coordinator;
pub use fault_tolerance::{BotMigration, CoordinatorCheckpoint, FaultDetector, MetricsBuffer};
pub use registry::WorkerRegistry;
pub use worker::Worker;

// Include generated protobuf code
pub mod proto {
    tonic::include_proto!("adnet.testbots");
}
