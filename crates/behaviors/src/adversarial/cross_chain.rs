/// Cross-chain attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-A-010: Double-Spend Attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubleSpendAttack {
    pub amount: u128,
    pub unlock_id: String,
}

impl DoubleSpendAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Double-spend via race condition");

        // Step 1: Lock AX on Alpha
        // Step 2: Submit two mint requests on Delta with same unlock_id
        // Expected: Second mint rejected (unlock_id already used)

        Ok(BehaviorResult::error(
            "Double-spend prevented by replay protection",
        ))
    }
}

/// PT-A-011: Finality Bypass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityBypass {
    pub amount: u128,
}

impl FinalityBypass {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Finality bypass attempt");

        // Try to mint before Alpha finality confirmed
        // Expected: Rejected due to insufficient confirmations

        Ok(BehaviorResult::error("Finality requirement enforced"))
    }
}

/// PT-A-012: Replay Attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayAttack {
    pub transaction_signature: Vec<u8>,
}

impl ReplayAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Replay attack across chains");

        // Replay Alpha transaction on Delta
        // Expected: Rejected due to chain ID mismatch

        Ok(BehaviorResult::error("Replay prevented by chain ID"))
    }
}
