/// General user bot role
///
/// Simulates regular user operations like transfers and balance queries

use adnet_testbot::{Bot, BotContext, BehaviorResult, Result};
use async_trait::async_trait;

pub struct GeneralUserBot {
    id: String,
    context: Option<BotContext>,
}

impl GeneralUserBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            context: None,
        }
    }
}

#[async_trait]
impl Bot for GeneralUserBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        self.context = Some(context.clone());
        tracing::info!("GeneralUserBot {} setup complete", self.id);
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        tracing::info!("GeneralUserBot {} executing behavior: {}", self.id, behavior_id);

        // TODO: Implement actual behaviors
        Ok(BehaviorResult::success(format!("Executed {}", behavior_id)))
    }

    async fn teardown(&mut self) -> Result<()> {
        tracing::info!("GeneralUserBot {} teardown complete", self.id);
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn role(&self) -> &str {
        "general_user"
    }
}
