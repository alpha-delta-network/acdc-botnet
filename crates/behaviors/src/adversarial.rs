pub mod byzantine;
pub mod cross_chain;
/// Adversarial behavior patterns for security testing
pub mod governance;
pub mod mev;
pub mod privacy;
pub mod resource;

// Re-export key attacks
pub use byzantine::{CensorshipAttack, Equivocation, InvalidBlockProposal};
pub use cross_chain::{DoubleSpendAttack, FinalityBypass, ReplayAttack};
pub use governance::{FlashLoanGovernance, ProposalSpam, SybilAttack};
pub use mev::{FrontRunning, LiquidationSniping, SandwichAttack};
pub use privacy::{AmountMatching, TimingCorrelation};
pub use resource::{MempoolSpam, StorageBomb};
