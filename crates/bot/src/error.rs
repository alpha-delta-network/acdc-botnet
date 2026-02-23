/// Error types for the bot framework
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BotError>;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Identity generation failed: {0}")]
    IdentityError(String),

    #[error("Wallet operation failed: {0}")]
    WalletError(String),

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    #[error("State transition error: {0}")]
    StateError(String),

    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Behavior execution failed: {0}")]
    BehaviorError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
