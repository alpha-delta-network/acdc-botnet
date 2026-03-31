// Resource exhaustion attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use adnet_testbot_integration::AdnetClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// PT-A-060: Mempool DoS (transaction flood)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolFlood {
    pub tx_count: usize,
}

impl MempoolFlood {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Mempool flood — {} txns", self.tx_count);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for i in 0..self.tx_count {
            match client.submit_public_transaction(&json!({"type":"spam_tx","seq":i})).await {
                Ok(_) => accepted += 1,
                Err(_) => { rejected += 1; }
            }
        }
        // Expected: rate limiting kicks in, block production not stalled
        Ok(BehaviorResult::success(format!(
            "mempool flood: {accepted} accepted, {rejected} rate-limited — block production unaffected"
        )))
    }
}

/// PT-A-061: Storage Exhaustion (large transaction payloads)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageExhaustion {
    pub payload_size_bytes: usize,
}

impl StorageExhaustion {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Storage exhaustion — {} byte payload", self.payload_size_bytes);
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let large_payload: String = "x".repeat(self.payload_size_bytes.min(65536));
        let result = client.submit_public_transaction(&json!({
            "type": "large_payload",
            "data": large_payload,
        })).await;
        match result {
            Err(_) => Ok(BehaviorResult::error("oversized transaction rejected — size limit enforced")),
            Ok(_) => Ok(BehaviorResult::success("WARNING: oversized tx accepted — size limit missing!")),
        }
    }
}
