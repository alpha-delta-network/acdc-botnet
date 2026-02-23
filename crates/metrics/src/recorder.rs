/// Event recorder
// Placeholder for Phase 1 task #5

use crate::BotEvent;

pub struct EventRecorder {
    events: Vec<BotEvent>,
}

impl EventRecorder {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn record(&mut self, event: BotEvent) {
        self.events.push(event);
    }
}

impl Default for EventRecorder {
    fn default() -> Self {
        Self::new()
    }
}
