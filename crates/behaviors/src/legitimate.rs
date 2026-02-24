pub mod cross_chain;
/// Legitimate user behavior patterns
///
/// Implements real-world user workflows for testing
pub mod governance;
pub mod privacy;
pub mod trading;
pub mod validator;

// Re-export common behaviors
pub use cross_chain::{BurnUnlockFlow, LockMintFlow};
pub use governance::{BasicProposalVoting, JointGovernance};
pub use privacy::ShieldedTransfer;
pub use trading::{LimitOrderLifecycle, SpotMarketOrder};
pub use validator::{BlockAttestation, BlockProposal, RewardsClaim};
