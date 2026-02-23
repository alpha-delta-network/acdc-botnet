/// State assumption anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-010: Insufficient Balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsufficientBalance {
    pub attempt_amount: u128,
    pub actual_balance: u128,
}

impl InsufficientBalance {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "TEST: Insufficient balance ({} available, {} required)",
            self.actual_balance,
            self.attempt_amount
        );

        // Try to transfer more than available
        // Expected: "Insufficient balance" error

        Ok(BehaviorResult::error(format!(
            "Insufficient balance: available {} AX, required {} AX (expected)",
            self.actual_balance, self.attempt_amount
        )))
    }
}

/// PT-D-011: Double-Spend Attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubleSpend {
    pub nonce: u64,
}

impl DoubleSpend {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Double-spend with duplicate nonce {}", self.nonce);

        // Submit two transactions with same nonce
        // Expected: Second rejected with "Nonce already used"

        Ok(BehaviorResult::error("Nonce already used (expected)"))
    }
}

/// PT-D-012: Stale Nonce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleNonce {
    pub submitted_nonce: u64,
    pub expected_nonce: u64,
}

impl StaleNonce {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "TEST: Stale nonce (submitted {}, expected {})",
            self.submitted_nonce,
            self.expected_nonce
        );

        // Use outdated nonce
        // Expected: "Invalid nonce" with expected value

        Ok(BehaviorResult::error(format!(
            "Invalid nonce: submitted {}, expected {} (expected)",
            self.submitted_nonce, self.expected_nonce
        )))
    }
}
