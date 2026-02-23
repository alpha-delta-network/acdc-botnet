/// Inter-bot communication and message passing
///
/// Provides a lightweight message bus for bots to coordinate actions

use crate::{BotError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::broadcast;

/// Message types for bot communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Coordination message (e.g., "ready", "start", "stop")
    Coordination,
    /// Data sharing
    Data,
    /// Request for action
    Request,
    /// Response to request
    Response,
    /// Event notification
    Event,
}

/// A message that can be sent between bots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Sender bot ID
    pub from: String,

    /// Recipient bot ID (or "broadcast" for all)
    pub to: String,

    /// Message type
    pub msg_type: MessageType,

    /// Message content
    pub content: serde_json::Value,

    /// Timestamp
    pub timestamp_ms: i64,

    /// Optional correlation ID for request/response
    pub correlation_id: Option<String>,
}

impl Message {
    pub fn new(from: String, to: String, msg_type: MessageType, content: serde_json::Value) -> Self {
        Self {
            from,
            to,
            msg_type,
            content,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            correlation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, id: String) -> Self {
        self.correlation_id = Some(id);
        self
    }

    pub fn is_broadcast(&self) -> bool {
        self.to == "broadcast"
    }
}

/// Message channel for a specific bot
type BotChannel = broadcast::Sender<Message>;

/// Communication bus for inter-bot messaging
pub struct MessageBus {
    /// Channels per bot
    channels: Arc<RwLock<HashMap<String, BotChannel>>>,

    /// Broadcast channel for all bots
    broadcast_channel: BotChannel,
}

impl MessageBus {
    /// Create a new message bus
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);

        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            broadcast_channel: tx,
        }
    }

    /// Register a bot to receive messages
    pub fn register_bot(&self, bot_id: String) -> broadcast::Receiver<Message> {
        let (tx, rx) = broadcast::channel(100);

        self.channels.write().insert(bot_id, tx);

        rx
    }

    /// Unregister a bot
    pub fn unregister_bot(&self, bot_id: &str) {
        self.channels.write().remove(bot_id);
    }

    /// Send a message to a specific bot
    pub fn send(&self, message: Message) -> Result<()> {
        if message.is_broadcast() {
            // Send to broadcast channel
            self.broadcast_channel
                .send(message)
                .map_err(|e| BotError::CommunicationError(format!("Broadcast failed: {}", e)))?;
        } else {
            // Send to specific bot
            let channels = self.channels.read();
            let channel = channels
                .get(&message.to)
                .ok_or_else(|| BotError::CommunicationError(format!("Bot {} not found", message.to)))?;

            channel
                .send(message)
                .map_err(|e| BotError::CommunicationError(format!("Send failed: {}", e)))?;
        }

        Ok(())
    }

    /// Get a receiver for broadcast messages
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<Message> {
        self.broadcast_channel.subscribe()
    }

    /// Get number of registered bots
    pub fn bot_count(&self) -> usize {
        self.channels.read().len()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_send() {
        let bus = MessageBus::new();

        // Register two bots
        let mut rx1 = bus.register_bot("bot-1".to_string());
        let mut rx2 = bus.register_bot("bot-2".to_string());

        // Send message from bot-1 to bot-2
        let msg = Message::new(
            "bot-1".to_string(),
            "bot-2".to_string(),
            MessageType::Data,
            serde_json::json!({"value": 42}),
        );

        bus.send(msg.clone()).unwrap();

        // bot-2 should receive the message
        let received = rx2.recv().await.unwrap();
        assert_eq!(received.from, "bot-1");
        assert_eq!(received.content, serde_json::json!({"value": 42}));

        // bot-1 should not receive anything (not a broadcast)
        assert!(rx1.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_broadcast() {
        let bus = MessageBus::new();

        // Register two bots
        let mut rx1 = bus.subscribe_broadcast();
        let mut rx2 = bus.subscribe_broadcast();

        // Send broadcast message
        let msg = Message::new(
            "coordinator".to_string(),
            "broadcast".to_string(),
            MessageType::Coordination,
            serde_json::json!({"command": "start"}),
        );

        bus.send(msg).unwrap();

        // Both bots should receive the message
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1.content, serde_json::json!({"command": "start"}));
        assert_eq!(received2.content, serde_json::json!({"command": "start"}));
    }

    #[test]
    fn test_bot_count() {
        let bus = MessageBus::new();

        assert_eq!(bus.bot_count(), 0);

        bus.register_bot("bot-1".to_string());
        assert_eq!(bus.bot_count(), 1);

        bus.register_bot("bot-2".to_string());
        assert_eq!(bus.bot_count(), 2);

        bus.unregister_bot("bot-1");
        assert_eq!(bus.bot_count(), 1);
    }

    #[test]
    fn test_correlation_id() {
        let msg = Message::new(
            "bot-1".to_string(),
            "bot-2".to_string(),
            MessageType::Request,
            serde_json::json!({"query": "balance"}),
        ).with_correlation_id("req-123".to_string());

        assert_eq!(msg.correlation_id, Some("req-123".to_string()));
    }
}
