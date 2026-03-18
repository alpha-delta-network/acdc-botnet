/// Bot role implementations
///
/// This module provides concrete bot roles that implement the Bot trait.
pub mod general_user;
pub mod governor;
pub mod trader;

// Re-export common types
pub use general_user::GeneralUserBot;
pub use governor::{
    GidStatus, GovernorBot, MultiSigPending, ProposalType, SignedVote, VoteChoice,
    DEFAULT_MULTISIG_THRESHOLD, GID_SIGNATORY_COUNT,
};
pub use trader::TraderBot;
