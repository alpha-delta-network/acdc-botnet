/// Distributed bot orchestration
///
/// Provides gRPC-based coordinator/worker architecture

pub mod coordinator;
pub mod worker;
pub mod protocol;
pub mod registry;
pub mod fault_tolerance;

pub use coordinator::Coordinator;
pub use worker::Worker;
pub use registry::WorkerRegistry;
pub use fault_tolerance::{FaultDetector, BotMigration, MetricsBuffer, CoordinatorCheckpoint};

// Include generated protobuf code
pub mod proto {
    tonic::include_proto!("adnet.testbots");
}
