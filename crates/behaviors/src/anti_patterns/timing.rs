/// Timing/ordering anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-020: Pre-Timelock Execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreTimelockExecution {
    pub proposal_id: String,
}

impl PreTimelockExecution {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Pre-timelock execution attempt");

        // Try to execute before timelock expires
        // Expected: "Timelock not expired" error

        Ok(BehaviorResult::error("Timelock not expired (expected)"))
    }
}

/// PT-D-021: Late Vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LateVote {
    pub proposal_id: String,
}

impl LateVote {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Late vote after deadline");

        // Vote after voting period closed
        // Expected: "Voting period closed" error

        Ok(BehaviorResult::error("Voting period closed (expected)"))
    }
}

/// PT-D-022: Expired Proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiredProof;

impl ExpiredProof {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Expired ZK proof submission");

        // Submit proof after expiration
        // Expected: "Proof expired" error

        Ok(BehaviorResult::error("Proof expired (expected)"))
    }
}
