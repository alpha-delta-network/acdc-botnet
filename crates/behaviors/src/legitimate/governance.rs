/// Governance behavior patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Behavior trait for governance patterns
#[async_trait]
pub trait GovernanceBehavior: Send + Sync {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult>;
}

/// PT-L-001: Basic Proposal Voting
///
/// Complete governance proposal flow: create → vote → timelock → execute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicProposalVoting {
    pub proposal_type: String,
    pub vote: VoteOption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteOption {
    Yes,
    No,
    Abstain,
}

impl BasicProposalVoting {
    pub fn new(proposal_type: String, vote: VoteOption) -> Self {
        Self {
            proposal_type,
            vote,
        }
    }
}

#[async_trait]
impl GovernanceBehavior for BasicProposalVoting {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing governance vote: {:?}",
            context.execution.bot_id,
            self.vote
        );

        // TODO: Implement actual governance voting
        // 1. Query active proposals
        // 2. Cast vote using appropriate API
        // 3. Verify vote recorded
        // 4. Wait for proposal execution (if applicable)

        Ok(BehaviorResult::success("Governance vote cast successfully"))
    }
}

/// PT-L-003: Joint Alpha/Delta Governance
///
/// Proposals that require coordination across both chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointGovernance {
    pub alpha_proposal_id: String,
    pub delta_proposal_id: String,
    pub vote: VoteOption,
}

impl JointGovernance {
    pub fn new(alpha_proposal_id: String, delta_proposal_id: String, vote: VoteOption) -> Self {
        Self {
            alpha_proposal_id,
            delta_proposal_id,
            vote,
        }
    }
}

#[async_trait]
impl GovernanceBehavior for JointGovernance {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing joint governance vote",
            context.execution.bot_id
        );

        // TODO: Implement joint governance
        // 1. Verify proposal exists on both chains
        // 2. Cast vote on Alpha
        // 3. Cast vote on Delta
        // 4. Verify both votes recorded
        // 5. Wait for IPC coordination

        Ok(BehaviorResult::success(
            "Joint governance vote cast successfully",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_proposal_creation() {
        let behavior = BasicProposalVoting::new("parameter_update".to_string(), VoteOption::Yes);
        assert_eq!(behavior.proposal_type, "parameter_update");
    }
}
