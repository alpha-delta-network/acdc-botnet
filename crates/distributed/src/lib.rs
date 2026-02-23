/// Distributed bot orchestration
///
/// Provides gRPC-based coordinator/worker architecture

pub mod coordinator;
pub mod worker;
pub mod protocol;
pub mod registry;

pub use coordinator::Coordinator;
pub use worker::Worker;
pub use registry::WorkerRegistry;

// Include generated protobuf code
pub mod proto {
    tonic::include_proto!("adnet.testbots");
}
