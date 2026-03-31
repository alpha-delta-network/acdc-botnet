pub mod gauntlet_bots;
/// Bot role implementations
///
/// This module provides concrete bot roles that implement the Bot trait.
pub mod general_user;
pub mod governor;
pub mod trader;

// Re-export common types
pub use gauntlet_bots::{LightFleet, 
    AdversarialAttack, AdversarialBot, AtomicSwapBot, BridgeBot, DeadWalletBot, DeltaVoterBot,
    EarnInBot, GauntletFleet, GauntletGovernorBot, MessengerBot, OracleBot, ProverBot, ScannerBot,
    TechRepBot, UserTransactorBot, ValidatorBot,
};
pub use general_user::GeneralUserBot;
pub use governor::{
    GidStatus, GovernorBot, MultiSigPending, ProposalType, SignedVote, VoteChoice,
    DEFAULT_MULTISIG_THRESHOLD, GID_SIGNATORY_COUNT,
};
pub use trader::TraderBot;
