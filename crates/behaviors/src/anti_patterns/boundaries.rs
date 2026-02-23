/// Boundary condition anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-050: Integer Overflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerOverflow;

impl IntegerOverflow {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Integer overflow attempt");

        // Try to transfer 2^128 tokens
        // Expected: "Amount exceeds maximum" error

        Ok(BehaviorResult::error("Amount exceeds maximum (expected)"))
    }
}

/// PT-D-051: Zero Amount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroAmount;

impl ZeroAmount {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Zero amount transfer");

        // Transfer 0 tokens
        // Expected: "Amount must be positive" error

        Ok(BehaviorResult::error("Amount must be positive (expected)"))
    }
}

/// PT-D-052: Maximum Size Exceeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxSizeExceeded;

impl MaxSizeExceeded {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Maximum transaction size exceeded");

        // Submit oversized transaction
        // Expected: "Transaction too large" error

        Ok(BehaviorResult::error("Transaction too large (expected)"))
    }
}
