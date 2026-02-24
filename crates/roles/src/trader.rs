/// Trader bot role
///
/// Simulates DEX trading operations
use adnet_testbot::{BehaviorResult, Bot, BotContext, Result};
use async_trait::async_trait;

pub struct TraderBot {
    id: String,
    context: Option<BotContext>,
}

impl TraderBot {
    pub fn new(id: String) -> Self {
        Self { id, context: None }
    }
}

#[async_trait]
impl Bot for TraderBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        self.context = Some(context.clone());
        tracing::info!("TraderBot {} setup complete", self.id);
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        tracing::info!("TraderBot {} executing behavior: {}", self.id, behavior_id);

        // TODO: Implement actual trading behaviors
        Ok(BehaviorResult::success(format!("Executed {}", behavior_id)))
    }

    async fn teardown(&mut self) -> Result<()> {
        tracing::info!("TraderBot {} teardown complete", self.id);
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn role(&self) -> &str {
        "trader"
    }
}
