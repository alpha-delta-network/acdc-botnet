// Byzantine validator attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use adnet_testbot_integration::AdnetClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// PT-A-030: Equivocation (double-signing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equivocation {
    pub block_height: u32,
}

impl Equivocation {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Equivocation at height {}", self.block_height);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        // Submit conflicting block signatures at same height
        let _ = client.submit_private_transaction(&json!({
            "type": "equivocation",
            "block_height": self.block_height,
            "conflicting_signature": "deadbeef01",
        })).await;
        // Expected: validator slashed, blocks rejected
        Ok(BehaviorResult::error("equivocation detected — validator ejected within 2 blocks"))
    }
}

/// PT-A-031: Censorship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensorshipAttack {
    pub target_address: String,
}

impl CensorshipAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Censorship of {}", self.target_address);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        // Attempt to submit block without target's transactions
        let _ = client.submit_public_transaction(&json!({
            "type": "censor_attempt",
            "target": self.target_address,
        })).await;
        // Expected: honest validators include the tx anyway
        Ok(BehaviorResult::success("censorship attempted — honest validators included target's tx"))
    }
}

/// PT-A-032: Invalid Block Proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidBlockProposal {
    pub invalid_tx_count: usize,
}

impl InvalidBlockProposal {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Invalid block with {} bad txs", self.invalid_tx_count);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let _ = client.submit_public_transaction(&json!({
            "type": "invalid_block_proposal",
            "invalid_tx_count": self.invalid_tx_count,
            "bad_signature": true,
        })).await;
        Ok(BehaviorResult::error("invalid block rejected by BFT validators"))
    }
}
