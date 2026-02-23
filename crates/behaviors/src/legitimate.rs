/// Legitimate user behavior patterns
///
/// Implements real-world user workflows for testing

pub mod governance;
pub mod cross_chain;
pub mod trading;
pub mod privacy;
pub mod validator;

// Re-export common behaviors
pub use governance::{BasicProposalVoting, JointGovernance};
pub use cross_chain::{LockMintFlow, BurnUnlockFlow};
pub use trading::{SpotMarketOrder, LimitOrderLifecycle};
pub use privacy::ShieldedTransfer;
pub use validator::{BlockProposal, BlockAttestation, RewardsClaim};
