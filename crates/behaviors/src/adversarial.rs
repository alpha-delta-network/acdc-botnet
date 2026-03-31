pub mod byzantine;
pub mod cross_chain;
/// Adversarial behavior patterns for security testing
pub mod governance;
pub mod mev;
pub mod privacy;
pub mod resource;

// Re-export key attacks (names match actual struct names in each file)
pub use byzantine::{CensorshipAttack, Equivocation, InvalidBlockProposal};
pub use cross_chain::{BridgeMismatchAttack, DoubleSpendAttack, ReplayAttack};
pub use governance::{GrimTriggerAbuse, MaliciousProposal};
pub use mev::{MevFrontRunning, SandwichAttack};
pub use privacy::{LinkabilityAttack, NullifierReuse};
pub use resource::{MempoolFlood, StorageExhaustion};
