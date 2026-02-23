/// Bot role implementations
///
/// This module provides concrete bot roles that implement the Bot trait.

pub mod general_user;
pub mod trader;

// Re-export common types
pub use general_user::GeneralUserBot;
pub use trader::TraderBot;
