/// Trader bot role
///
/// Simulates DEX trading operations
use adnet_testbot::{BehaviorResult, Bot, BotContext, BotError, Result};
use adnet_testbot_integration::AdnetClient;
use async_trait::async_trait;
use serde_json::json;

pub struct TraderBot {
    id: String,
    adnet_url: String,
}

impl TraderBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for TraderBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        self.adnet_url = context.execution.network.adnet_unified.clone();
        tracing::info!("TraderBot {} setup complete", self.id);
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        tracing::info!("TraderBot {} executing behavior: {}", self.id, behavior_id);
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "dex.place_limit_order" => {
                let transaction_id = client
                    .execute_delta_transaction(
                        "dex.aleo",
                        "place_limit_order",
                        vec!["1u64".to_string(), "100u64".to_string()],
                        "placeholder_key",
                        1000,
                    )
                    .await?;
                Ok(
                    BehaviorResult::success(format!("order placed: {}", transaction_id))
                        .with_data(json!({"market": "AX/DX", "transaction_id": transaction_id})),
                )
            }
            _ => Err(BotError::NetworkError(format!(
                "TraderBot: unknown behavior {}",
                behavior_id
            ))
            .into()),
        }
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
