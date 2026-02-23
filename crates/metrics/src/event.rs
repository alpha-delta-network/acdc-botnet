/// Bot event definitions
// Placeholder for Phase 1 task #5

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BotEvent {
    BotStarted { bot_id: String },
    BotStopped { bot_id: String },
    BehaviorExecuted { bot_id: String, behavior: String },
    TransactionSubmitted { bot_id: String, tx_hash: String },
}
