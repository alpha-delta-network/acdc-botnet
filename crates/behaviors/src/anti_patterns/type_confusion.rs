/// Type confusion anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-030: Wrong Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrongChain;

impl WrongChain {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Wrong chain transaction");

        // Submit Alpha tx to Delta endpoint
        // Expected: "Invalid chain ID" error

        Ok(BehaviorResult::error("Invalid chain ID (expected)"))
    }
}

/// PT-D-031: Wrong Network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrongNetwork;

impl WrongNetwork {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Wrong network (testnet vs mainnet)");

        // Submit testnet tx to mainnet
        // Expected: "Network mismatch" error

        Ok(BehaviorResult::error("Network mismatch (expected)"))
    }
}
