/// Bot state management with typestate pattern
///
/// Provides type-safe state transitions using phantom types to encode
/// bot lifecycle states at compile time.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Bot state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotState {
    /// Initial state
    Created,
    /// Setup phase
    Initializing,
    /// Running and executing behaviors
    Running,
    /// Paused (can be resumed)
    Paused,
    /// Shutting down
    Stopping,
    /// Fully stopped
    Stopped,
    /// Error state
    Error,
}

impl std::fmt::Display for BotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotState::Created => write!(f, "created"),
            BotState::Initializing => write!(f, "initializing"),
            BotState::Running => write!(f, "running"),
            BotState::Paused => write!(f, "paused"),
            BotState::Stopping => write!(f, "stopping"),
            BotState::Stopped => write!(f, "stopped"),
            BotState::Error => write!(f, "error"),
        }
    }
}

/// State transition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state
    pub from: BotState,
    /// New state
    pub to: BotState,
    /// Timestamp of transition
    pub timestamp_ms: i64,
    /// Optional message
    pub message: Option<String>,
}

impl StateTransition {
    pub fn new(from: BotState, to: BotState) -> Self {
        Self {
            from,
            to,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            message: None,
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}

/// Type-safe state machine using phantom types
/// This allows compile-time enforcement of valid state transitions
pub struct StateMachine<S> {
    current: BotState,
    history: Vec<StateTransition>,
    _marker: PhantomData<S>,
}

/// Phantom types for different states
pub struct Created;
pub struct Initializing;
pub struct Running;
pub struct Paused;
pub struct Stopping;
pub struct Stopped;
pub struct Error;

impl StateMachine<Created> {
    pub fn new() -> Self {
        Self {
            current: BotState::Created,
            history: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn initialize(mut self) -> StateMachine<Initializing> {
        let transition = StateTransition::new(self.current, BotState::Initializing);
        self.history.push(transition);
        StateMachine {
            current: BotState::Initializing,
            history: self.history,
            _marker: PhantomData,
        }
    }
}

impl StateMachine<Initializing> {
    pub fn start(mut self) -> StateMachine<Running> {
        let transition = StateTransition::new(self.current, BotState::Running);
        self.history.push(transition);
        StateMachine {
            current: BotState::Running,
            history: self.history,
            _marker: PhantomData,
        }
    }

    pub fn fail(mut self, message: String) -> StateMachine<Error> {
        let transition = StateTransition::new(self.current, BotState::Error)
            .with_message(message);
        self.history.push(transition);
        StateMachine {
            current: BotState::Error,
            history: self.history,
            _marker: PhantomData,
        }
    }
}

impl StateMachine<Running> {
    pub fn pause(mut self) -> StateMachine<Paused> {
        let transition = StateTransition::new(self.current, BotState::Paused);
        self.history.push(transition);
        StateMachine {
            current: BotState::Paused,
            history: self.history,
            _marker: PhantomData,
        }
    }

    pub fn stop(mut self) -> StateMachine<Stopping> {
        let transition = StateTransition::new(self.current, BotState::Stopping);
        self.history.push(transition);
        StateMachine {
            current: BotState::Stopping,
            history: self.history,
            _marker: PhantomData,
        }
    }

    pub fn fail(mut self, message: String) -> StateMachine<Error> {
        let transition = StateTransition::new(self.current, BotState::Error)
            .with_message(message);
        self.history.push(transition);
        StateMachine {
            current: BotState::Error,
            history: self.history,
            _marker: PhantomData,
        }
    }
}

impl StateMachine<Paused> {
    pub fn resume(mut self) -> StateMachine<Running> {
        let transition = StateTransition::new(self.current, BotState::Running);
        self.history.push(transition);
        StateMachine {
            current: BotState::Running,
            history: self.history,
            _marker: PhantomData,
        }
    }

    pub fn stop(mut self) -> StateMachine<Stopping> {
        let transition = StateTransition::new(self.current, BotState::Stopping);
        self.history.push(transition);
        StateMachine {
            current: BotState::Stopping,
            history: self.history,
            _marker: PhantomData,
        }
    }
}

impl StateMachine<Stopping> {
    pub fn complete(mut self) -> StateMachine<Stopped> {
        let transition = StateTransition::new(self.current, BotState::Stopped);
        self.history.push(transition);
        StateMachine {
            current: BotState::Stopped,
            history: self.history,
            _marker: PhantomData,
        }
    }
}

impl<S> StateMachine<S> {
    pub fn current_state(&self) -> BotState {
        self.current
    }

    pub fn history(&self) -> &[StateTransition] {
        &self.history
    }
}

impl Default for StateMachine<Created> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_happy_path() {
        // Created -> Initializing -> Running -> Stopping -> Stopped
        let sm = StateMachine::new();
        assert_eq!(sm.current_state(), BotState::Created);

        let sm = sm.initialize();
        assert_eq!(sm.current_state(), BotState::Initializing);

        let sm = sm.start();
        assert_eq!(sm.current_state(), BotState::Running);

        let sm = sm.stop();
        assert_eq!(sm.current_state(), BotState::Stopping);

        let sm = sm.complete();
        assert_eq!(sm.current_state(), BotState::Stopped);

        // Check history
        assert_eq!(sm.history().len(), 4);
    }

    #[test]
    fn test_state_machine_with_pause() {
        let sm = StateMachine::new();
        let sm = sm.initialize();
        let sm = sm.start();

        let sm = sm.pause();
        assert_eq!(sm.current_state(), BotState::Paused);

        let sm = sm.resume();
        assert_eq!(sm.current_state(), BotState::Running);
    }

    #[test]
    fn test_state_machine_error() {
        let sm = StateMachine::new();
        let sm = sm.initialize();
        let sm = sm.fail("Initialization failed".to_string());

        assert_eq!(sm.current_state(), BotState::Error);

        let last_transition = sm.history().last().unwrap();
        assert_eq!(last_transition.message, Some("Initialization failed".to_string()));
    }
}
