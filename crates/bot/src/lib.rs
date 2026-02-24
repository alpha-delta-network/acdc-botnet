/// Core bot framework for adnet-testbots
///
/// This module provides the foundational types and traits for building
/// autonomous bot actors that interact with the Alpha/Delta protocol.
pub mod actor;
pub mod communication;
pub mod context;
pub mod error;
pub mod identity;
pub mod scheduler;
pub mod state;
pub mod wallet;

// Re-export key types
pub use actor::{BehaviorResult, Bot, BotContext};
pub use context::ExecutionContext;
pub use error::{BotError, Result};
pub use identity::{Identity, IdentityGenerator};
pub use scheduler::{Scheduler, Task};
pub use state::{BotState, StateTransition};
pub use wallet::{Balance, ChainId, Token, Wallet};
