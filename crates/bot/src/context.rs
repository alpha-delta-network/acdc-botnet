/// Execution context for bot operations
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Context provided to bots during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique bot identifier
    pub bot_id: String,

    /// Role of the bot
    pub role: String,

    /// Network endpoints
    pub network: NetworkEndpoints,

    /// Configuration parameters
    pub config: HashMap<String, serde_json::Value>,

    /// Execution metadata
    pub metadata: ContextMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEndpoints {
    pub alphaos_rest: String,
    pub deltaos_rest: String,
    pub adnet_unified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// Scenario ID (if part of a scenario)
    pub scenario_id: Option<String>,

    /// Phase within scenario
    pub phase: Option<String>,

    /// Start timestamp
    pub start_time_ms: i64,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ExecutionContext {
    pub fn new(bot_id: String, role: String, network: NetworkEndpoints) -> Self {
        Self {
            bot_id,
            role,
            network,
            config: HashMap::new(),
            metadata: ContextMetadata {
                scenario_id: None,
                phase: None,
                start_time_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
                tags: Vec::new(),
            },
        }
    }

    pub fn with_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.config = config;
        self
    }

    pub fn with_scenario(mut self, scenario_id: String, phase: Option<String>) -> Self {
        self.metadata.scenario_id = Some(scenario_id);
        self.metadata.phase = phase;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags = tags;
        self
    }

    pub fn get_config<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.config
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}
