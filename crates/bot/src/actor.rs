/// Bot actor trait and types
///
/// Defines the core Bot trait that all bot implementations must follow.
/// Uses async trait for lifecycle methods: setup, execute, teardown.
use crate::{ExecutionContext, Identity, Result, Wallet};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Context provided to bots during their lifecycle
#[derive(Debug, Clone)]
pub struct BotContext {
    pub execution: ExecutionContext,
    pub identity: Identity,
    pub wallet: Arc<RwLock<Wallet>>,
}

impl BotContext {
    pub fn new(execution: ExecutionContext, identity: Identity, wallet: Wallet) -> Self {
        Self {
            execution,
            identity,
            wallet: Arc::new(RwLock::new(wallet)),
        }
    }
}

/// Result of a behavior execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorResult {
    /// Whether the behavior succeeded
    pub success: bool,

    /// Human-readable message
    pub message: String,

    /// Structured data (e.g., transaction hashes, block numbers)
    pub data: serde_json::Value,

    /// Performance metrics
    pub metrics: BehaviorMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorMetrics {
    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Number of operations performed
    pub operations_count: u64,

    /// Number of errors encountered
    pub errors_count: u64,
}

impl BehaviorResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: serde_json::Value::Null,
            metrics: BehaviorMetrics {
                duration_ms: 0,
                operations_count: 1,
                errors_count: 0,
            },
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: serde_json::Value::Null,
            metrics: BehaviorMetrics {
                duration_ms: 0,
                operations_count: 0,
                errors_count: 1,
            },
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    pub fn with_metrics(mut self, metrics: BehaviorMetrics) -> Self {
        self.metrics = metrics;
        self
    }
}

/// Core Bot trait that all bot implementations must follow
///
/// Bots have a three-phase lifecycle:
/// 1. setup() - Initialize state, register with network
/// 2. execute_behavior() - Perform the bot's intended actions
/// 3. teardown() - Clean up resources, unregister
#[async_trait]
pub trait Bot: Send + Sync {
    /// Initialize the bot
    ///
    /// This is called once when the bot is spawned. Use this to:
    /// - Establish network connections
    /// - Register with services
    /// - Load initial state
    ///
    /// # Errors
    /// Returns an error if initialization fails
    async fn setup(&mut self, context: &BotContext) -> Result<()>;

    /// Execute a behavior pattern
    ///
    /// This is the main action method. It executes a specific behavior
    /// pattern identified by behavior_id.
    ///
    /// # Arguments
    /// - `behavior_id`: Identifier of the behavior to execute
    ///
    /// # Errors
    /// Returns an error if behavior execution fails
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult>;

    /// Clean up the bot
    ///
    /// This is called when the bot is stopped. Use this to:
    /// - Close network connections
    /// - Unregister from services
    /// - Flush pending operations
    ///
    /// # Errors
    /// Returns an error if teardown fails
    async fn teardown(&mut self) -> Result<()>;

    /// Get the bot's unique identifier
    fn id(&self) -> &str;

    /// Get the bot's role
    fn role(&self) -> &str;

    /// Get the bot's current state
    fn state(&self) -> String {
        "running".to_string()
    }
}

/// A builder for creating bot instances
pub struct BotBuilder {
    bot_id: String,
    role: String,
    execution_context: ExecutionContext,
}

impl BotBuilder {
    pub fn new(bot_id: String, role: String, execution_context: ExecutionContext) -> Self {
        Self {
            bot_id,
            role,
            execution_context,
        }
    }

    pub fn build_context(self, identity: Identity, wallet: Wallet) -> BotContext {
        BotContext::new(self.execution_context, identity, wallet)
    }
}
