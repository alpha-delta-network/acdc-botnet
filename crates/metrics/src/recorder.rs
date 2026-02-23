/// Event recorder with thread-safe append-only log
use crate::BotEvent;
use parking_lot::RwLock;
use std::sync::Arc;

/// Thread-safe event recorder
#[derive(Clone)]
pub struct EventRecorder {
    events: Arc<RwLock<Vec<BotEvent>>>,
    max_events: usize,
}

impl EventRecorder {
    /// Create a new event recorder
    pub fn new() -> Self {
        Self::with_capacity(100_000)
    }

    /// Create a recorder with a maximum capacity
    pub fn with_capacity(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            max_events,
        }
    }

    /// Record a single event
    pub fn record(&self, event: BotEvent) {
        let mut events = self.events.write();

        // If at capacity, remove oldest events (sliding window)
        if events.len() >= self.max_events {
            let remove_count = events.len() - self.max_events + 1;
            events.drain(0..remove_count);
        }

        events.push(event);
    }

    /// Record multiple events
    pub fn record_batch(&self, batch: Vec<BotEvent>) {
        let mut events = self.events.write();

        for event in batch {
            if events.len() >= self.max_events {
                events.remove(0);
            }
            events.push(event);
        }
    }

    /// Get all events
    pub fn get_all(&self) -> Vec<BotEvent> {
        self.events.read().clone()
    }

    /// Get events since a timestamp
    pub fn get_since(&self, timestamp_ms: i64) -> Vec<BotEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.timestamp_ms() >= timestamp_ms)
            .cloned()
            .collect()
    }

    /// Get events for a specific bot
    pub fn get_for_bot(&self, bot_id: &str) -> Vec<BotEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.bot_id() == Some(bot_id))
            .cloned()
            .collect()
    }

    /// Get total event count
    pub fn count(&self) -> usize {
        self.events.read().len()
    }

    /// Get error events
    pub fn get_errors(&self) -> Vec<BotEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.is_error())
            .cloned()
            .collect()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.write().clear();
    }
}

impl Default for EventRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_event() {
        let recorder = EventRecorder::new();

        recorder.record(BotEvent::BotStarted {
            bot_id: "bot-1".to_string(),
            role: "trader".to_string(),
            timestamp_ms: 1000,
        });

        assert_eq!(recorder.count(), 1);
    }

    #[test]
    fn test_capacity_limit() {
        let recorder = EventRecorder::with_capacity(10);

        // Record 15 events
        for i in 0..15 {
            recorder.record(BotEvent::BotStarted {
                bot_id: format!("bot-{}", i),
                role: "trader".to_string(),
                timestamp_ms: i as i64,
            });
        }

        // Should only keep 10
        assert_eq!(recorder.count(), 10);

        // Oldest events should be removed
        let events = recorder.get_all();
        assert_eq!(events[0].timestamp_ms(), 5);
    }

    #[test]
    fn test_get_since() {
        let recorder = EventRecorder::new();

        recorder.record(BotEvent::BotStarted {
            bot_id: "bot-1".to_string(),
            role: "trader".to_string(),
            timestamp_ms: 1000,
        });

        recorder.record(BotEvent::BotStarted {
            bot_id: "bot-2".to_string(),
            role: "trader".to_string(),
            timestamp_ms: 2000,
        });

        let recent = recorder.get_since(1500);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_get_for_bot() {
        let recorder = EventRecorder::new();

        recorder.record(BotEvent::BotStarted {
            bot_id: "bot-1".to_string(),
            role: "trader".to_string(),
            timestamp_ms: 1000,
        });

        recorder.record(BotEvent::BotStarted {
            bot_id: "bot-2".to_string(),
            role: "trader".to_string(),
            timestamp_ms: 2000,
        });

        let bot1_events = recorder.get_for_bot("bot-1");
        assert_eq!(bot1_events.len(), 1);
    }
}
