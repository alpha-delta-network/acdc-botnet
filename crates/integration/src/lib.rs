/// Integration with external systems
///
/// Provides clients for AlphaOS, DeltaOS, and Adnet unified API
pub mod adnet_client;
pub mod alphaos_client;
pub mod deltaos_client;

pub use adnet_client::AdnetClient;
pub use alphaos_client::AlphaOSClient;
pub use deltaos_client::DeltaOSClient;
