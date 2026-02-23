/// Bot event definitions for comprehensive observability
use serde::{Deserialize, Serialize};

/// All possible bot events with structured data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BotEvent {
    /// Bot lifecycle events
    BotStarted {
        bot_id: String,
        role: String,
        timestamp_ms: i64,
    },
    BotStopped {
        bot_id: String,
        timestamp_ms: i64,
        reason: String,
    },
    BotError {
        bot_id: String,
        timestamp_ms: i64,
        error: String,
    },

    /// Behavior execution events
    BehaviorStarted {
        bot_id: String,
        behavior_id: String,
        timestamp_ms: i64,
    },
    BehaviorCompleted {
        bot_id: String,
        behavior_id: String,
        timestamp_ms: i64,
        duration_ms: u64,
        success: bool,
    },

    /// Transaction events
    TransactionSubmitted {
        bot_id: String,
        tx_hash: String,
        tx_type: String,
        timestamp_ms: i64,
    },
    TransactionConfirmed {
        bot_id: String,
        tx_hash: String,
        timestamp_ms: i64,
        confirmation_time_ms: u64,
    },
    TransactionFailed {
        bot_id: String,
        tx_hash: String,
        timestamp_ms: i64,
        error: String,
    },

    /// Network events
    NetworkRequest {
        bot_id: String,
        endpoint: String,
        timestamp_ms: i64,
    },
    NetworkResponse {
        bot_id: String,
        endpoint: String,
        timestamp_ms: i64,
        latency_ms: u64,
        status_code: u16,
    },

    /// DEX events
    OrderPlaced {
        bot_id: String,
        order_id: String,
        pair: String,
        side: String,
        timestamp_ms: i64,
    },
    OrderFilled {
        bot_id: String,
        order_id: String,
        timestamp_ms: i64,
        fill_time_ms: u64,
    },
    OrderCanceled {
        bot_id: String,
        order_id: String,
        timestamp_ms: i64,
    },

    /// Governance events
    ProposalCreated {
        bot_id: String,
        proposal_id: String,
        timestamp_ms: i64,
    },
    VoteCast {
        bot_id: String,
        proposal_id: String,
        vote: String,
        timestamp_ms: i64,
    },

    /// Cross-chain events
    CrossChainLock {
        bot_id: String,
        lock_id: String,
        amount: u128,
        timestamp_ms: i64,
    },
    CrossChainMint {
        bot_id: String,
        mint_id: String,
        amount: u128,
        timestamp_ms: i64,
    },

    /// Validator events
    BlockProposed {
        bot_id: String,
        block_height: u32,
        timestamp_ms: i64,
    },
    BlockAttested {
        bot_id: String,
        block_height: u32,
        timestamp_ms: i64,
    },

    /// Scenario events
    ScenarioStarted {
        scenario_id: String,
        timestamp_ms: i64,
    },
    ScenarioCompleted {
        scenario_id: String,
        timestamp_ms: i64,
        duration_ms: u64,
        success: bool,
    },

    /// Metric snapshots
    MetricSnapshot {
        timestamp_ms: i64,
        tps: f64,
        latency_p50_ms: f64,
        latency_p95_ms: f64,
        latency_p99_ms: f64,
        error_rate: f64,
        active_bots: usize,
    },
}

impl BotEvent {
    /// Get the bot ID associated with this event, if any
    pub fn bot_id(&self) -> Option<&str> {
        match self {
            BotEvent::BotStarted { bot_id, .. }
            | BotEvent::BotStopped { bot_id, .. }
            | BotEvent::BotError { bot_id, .. }
            | BotEvent::BehaviorStarted { bot_id, .. }
            | BotEvent::BehaviorCompleted { bot_id, .. }
            | BotEvent::TransactionSubmitted { bot_id, .. }
            | BotEvent::TransactionConfirmed { bot_id, .. }
            | BotEvent::TransactionFailed { bot_id, .. }
            | BotEvent::NetworkRequest { bot_id, .. }
            | BotEvent::NetworkResponse { bot_id, .. }
            | BotEvent::OrderPlaced { bot_id, .. }
            | BotEvent::OrderFilled { bot_id, .. }
            | BotEvent::OrderCanceled { bot_id, .. }
            | BotEvent::ProposalCreated { bot_id, .. }
            | BotEvent::VoteCast { bot_id, .. }
            | BotEvent::CrossChainLock { bot_id, .. }
            | BotEvent::CrossChainMint { bot_id, .. }
            | BotEvent::BlockProposed { bot_id, .. }
            | BotEvent::BlockAttested { bot_id, .. } => Some(bot_id.as_str()),
            _ => None,
        }
    }

    /// Get the timestamp of this event
    pub fn timestamp_ms(&self) -> i64 {
        match self {
            BotEvent::BotStarted { timestamp_ms, .. }
            | BotEvent::BotStopped { timestamp_ms, .. }
            | BotEvent::BotError { timestamp_ms, .. }
            | BotEvent::BehaviorStarted { timestamp_ms, .. }
            | BotEvent::BehaviorCompleted { timestamp_ms, .. }
            | BotEvent::TransactionSubmitted { timestamp_ms, .. }
            | BotEvent::TransactionConfirmed { timestamp_ms, .. }
            | BotEvent::TransactionFailed { timestamp_ms, .. }
            | BotEvent::NetworkRequest { timestamp_ms, .. }
            | BotEvent::NetworkResponse { timestamp_ms, .. }
            | BotEvent::OrderPlaced { timestamp_ms, .. }
            | BotEvent::OrderFilled { timestamp_ms, .. }
            | BotEvent::OrderCanceled { timestamp_ms, .. }
            | BotEvent::ProposalCreated { timestamp_ms, .. }
            | BotEvent::VoteCast { timestamp_ms, .. }
            | BotEvent::CrossChainLock { timestamp_ms, .. }
            | BotEvent::CrossChainMint { timestamp_ms, .. }
            | BotEvent::BlockProposed { timestamp_ms, .. }
            | BotEvent::BlockAttested { timestamp_ms, .. }
            | BotEvent::ScenarioStarted { timestamp_ms, .. }
            | BotEvent::ScenarioCompleted { timestamp_ms, .. }
            | BotEvent::MetricSnapshot { timestamp_ms, .. } => *timestamp_ms,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            BotEvent::BotError { .. } | BotEvent::TransactionFailed { .. }
        )
    }
}
