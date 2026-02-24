pub mod boundaries;
pub mod prerequisites;
pub mod state;
pub mod timing;
pub mod type_confusion;
/// Developer anti-pattern behaviors for error handling testing
pub mod validation;

// Re-export common anti-patterns
pub use boundaries::{IntegerOverflow, MaxSizeExceeded, ZeroAmount};
pub use prerequisites::{MissingPriorLock, UnregisteredGovernor, UnstakedVoting};
pub use state::{DoubleSpend, InsufficientBalance, StaleNonce};
pub use timing::{ExpiredProof, LateVote, PreTimelockExecution};
pub use type_confusion::{WrongChain, WrongNetwork};
pub use validation::{InvalidFormat, InvalidSignature, MissingFields};
