/// Adversarial behavior patterns for security testing
pub mod governance;
pub mod cross_chain;
pub mod mev;
pub mod byzantine;
pub mod privacy;
pub mod resource;

// Re-export key attacks
pub use governance::{SybilAttack, FlashLoanGovernance, ProposalSpam};
pub use cross_chain::{DoubleSpendAttack, FinalityBypass, ReplayAttack};
pub use mev::{FrontRunning, SandwichAttack, LiquidationSniping};
pub use byzantine::{Equivocation, CensorshipAttack, InvalidBlockProposal};
pub use privacy::{TimingCorrelation, AmountMatching};
pub use resource::{MempoolSpam, StorageBomb};
