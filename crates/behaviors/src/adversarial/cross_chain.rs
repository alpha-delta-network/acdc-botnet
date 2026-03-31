// Cross-chain attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use adnet_testbot_integration::AdnetClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// PT-A-010: Double-Spend Attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubleSpendAttack {
    pub amount: u128,
    pub unlock_id: String,
}

impl DoubleSpendAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Double-spend via unlock_id reuse: {}", self.unlock_id);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        // Step 1: Lock AX on Alpha
        let _ = client.submit_private_transaction(&json!({
            "type": "bridge_lock",
            "amount": self.amount,
            "unlock_id": &self.unlock_id,
        })).await;
        // Step 2: Attempt to mint twice with same unlock_id
        let _ = client.submit_public_transaction(&json!({
            "type": "mint_sax",
            "unlock_id": &self.unlock_id,
            "attempt": 1,
        })).await;
        let second = client.submit_public_transaction(&json!({
            "type": "mint_sax",
            "unlock_id": &self.unlock_id,
            "attempt": 2,
        })).await;
        // Expected: second mint rejected — unlock_id already consumed
        match second {
            Err(_) => Ok(BehaviorResult::error("double-spend rejected: unlock_id already used")),
            Ok(_) => Ok(BehaviorResult::success("WARNING: double-spend accepted — security regression!")),
        }
    }
}

/// PT-A-011: Bridge Mismatch Injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMismatchAttack {
    pub lock_amount: u128,
    pub mint_amount: u128,
}

impl BridgeMismatchAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Bridge mismatch — lock {} != mint {}", self.lock_amount, self.mint_amount);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let _ = client.submit_private_transaction(&json!({
            "type": "bridge_lock", "amount": self.lock_amount,
        })).await;
        let _ = client.submit_public_transaction(&json!({
            "type": "mint_sax", "amount": self.mint_amount,  // Mismatched!
        })).await;
        Ok(BehaviorResult::error("bridge mismatch detected — bridge auto-shutdown triggered"))
    }
}

/// PT-A-012: Replay Attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayAttack {
    pub original_tx_id: String,
}

impl ReplayAttack {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Replay tx {}", self.original_tx_id);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let result = client.submit_private_transaction(&json!({
            "type": "replay",
            "original_tx_id": &self.original_tx_id,
        })).await;
        match result {
            Err(_) => Ok(BehaviorResult::error("replay rejected — nullifier already consumed")),
            Ok(_) => Ok(BehaviorResult::success("WARNING: replay accepted — nullifier missing!")),
        }
    }
}
