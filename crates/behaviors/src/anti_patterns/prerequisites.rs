/// Missing prerequisites anti-patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-D-040: Unstaked Voting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnstakedVoting;

impl UnstakedVoting {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Voting without stake");

        // Attempt to vote with 0 stake
        // Expected: "Insufficient stake" error

        Ok(BehaviorResult::error(
            "Insufficient stake for voting (expected)",
        ))
    }
}

/// PT-D-041: Unregistered Governor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnregisteredGovernor;

impl UnregisteredGovernor {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Proposal from non-governor");

        // Non-governor tries to create proposal
        // Expected: "Not a registered governor" error

        Ok(BehaviorResult::error(
            "Not a registered governor (expected)",
        ))
    }
}

/// PT-D-042: Missing Prior Lock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingPriorLock;

impl MissingPriorLock {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!("TEST: Unlock without prior lock");

        // Try to unlock with invalid unlock_id
        // Expected: "No corresponding lock found" error

        Ok(BehaviorResult::error(
            "No corresponding lock found (expected)",
        ))
    }
}
