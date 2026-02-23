/// Integration with external systems
///
/// Provides clients for AlphaOS, DeltaOS, and Adnet

pub mod alphaos_client;
pub mod deltaos_client;
pub mod adnet_client;

pub use alphaos_client::AlphaOSClient;
pub use deltaos_client::DeltaOSClient;
pub use adnet_client::AdnetClient;
