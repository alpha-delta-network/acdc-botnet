/// Developer anti-pattern behaviors for error handling testing
pub mod validation;
pub mod state;
pub mod timing;
pub mod type_confusion;
pub mod prerequisites;
pub mod boundaries;

// Re-export common anti-patterns
pub use validation::{InvalidSignature, InvalidFormat, MissingFields};
pub use state::{InsufficientBalance, DoubleSpend, StaleNonce};
pub use timing::{PreTimelockExecution, LateVote, ExpiredProof};
pub use type_confusion::{WrongChain, WrongNetwork};
pub use prerequisites::{UnstakedVoting, UnregisteredGovernor, MissingPriorLock};
pub use boundaries::{IntegerOverflow, ZeroAmount, MaxSizeExceeded};
