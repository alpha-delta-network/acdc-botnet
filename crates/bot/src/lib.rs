/// Core bot framework for adnet-testbots
///
/// This module provides the foundational types and traits for building
/// autonomous bot actors that interact with the Alpha/Delta protocol.

pub mod actor;
pub mod identity;
pub mod wallet;
pub mod scheduler;
pub mod state;
pub mod communication;
pub mod error;
pub mod context;

// Re-export key types
pub use actor::{Bot, BotContext, BehaviorResult};
pub use identity::{Identity, IdentityGenerator};
pub use wallet::{Wallet, Balance, ChainId};
pub use scheduler::{Scheduler, Task};
pub use state::{BotState, StateTransition};
pub use error::{BotError, Result};
pub use context::ExecutionContext;
