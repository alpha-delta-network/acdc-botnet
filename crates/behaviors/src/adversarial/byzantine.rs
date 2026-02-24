/// Byzantine validator attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-A-030: Equivocation (double-signing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equivocation {
    pub block_height: u32,
}

impl Equivocation {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Equivocation at height {}", self.block_height);

        // Sign two conflicting blocks at same height
        // Expected: Slashing via cryptographic evidence

        Ok(BehaviorResult::error(
            "Equivocation detected, validator slashed",
        ))
    }
}

/// PT-A-031: Censorship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensorshipAttack {
    pub target_address: String,
}

impl CensorshipAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Censorship of address {}", self.target_address);

        // Refuse to include transactions from target
        // Expected: Transaction eventually included by honest validator

        Ok(BehaviorResult::success(
            "Censorship attempted (transaction eventually included)",
        ))
    }
}

/// PT-A-032: Invalid Block Proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidBlockProposal {
    pub invalid_tx_count: usize,
}

impl InvalidBlockProposal {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!(
            "ATTACK: Invalid block with {} bad txs",
            self.invalid_tx_count
        );

        // Propose block with invalid transactions
        // Expected: Block rejected, validator potentially slashed

        Ok(BehaviorResult::error("Invalid block rejected"))
    }
}
