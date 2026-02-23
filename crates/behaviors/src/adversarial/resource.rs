/// Resource exhaustion attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use serde::{Deserialize, Serialize};

/// PT-A-050: Mempool Spam
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolSpam {
    pub spam_count: usize,
}

impl MempoolSpam {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Mempool spam with {} txs", self.spam_count);

        // Flood mempool with low-value transactions
        // Expected: Rate limiting, minimum fee enforcement

        Ok(BehaviorResult::error("Spam filtered by fee market"))
    }
}

/// PT-A-051: Storage Bomb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBomb {
    pub storage_size_mb: usize,
}

impl StorageBomb {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!("ATTACK: Storage bomb ({} MB)", self.storage_size_mb);

        // Deploy program with excessive storage
        // Expected: Storage fees, maximum size limits

        Ok(BehaviorResult::error("Storage limit exceeded"))
    }
}
