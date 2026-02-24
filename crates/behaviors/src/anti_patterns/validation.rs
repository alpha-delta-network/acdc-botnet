/// Parameter validation anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-001: Invalid Signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidSignature;

impl InvalidSignature {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Invalid signature submission");

        // Submit transaction with malformed signature
        // Expected: "Invalid signature" error

        Ok(BehaviorResult::error("Invalid signature (expected)"))
    }
}

/// PT-D-002: Invalid Format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidFormat;

impl InvalidFormat {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Invalid transaction format");

        // Submit malformed transaction
        // Expected: "Parse error" with field details

        Ok(BehaviorResult::error(
            "Parse error: missing field 'to' (expected)",
        ))
    }
}

/// PT-D-003: Missing Fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingFields;

impl MissingFields {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Missing required fields");

        // Submit transaction missing required fields
        // Expected: "Validation error" listing missing fields

        Ok(BehaviorResult::error(
            "Missing required fields: amount, nonce (expected)",
        ))
    }
}
