/// General user bot role
///
/// Simulates regular user operations like transfers and balance queries
use adnet_testbot::{BehaviorResult, Bot, BotContext, BotError, Result};
use async_trait::async_trait;
use serde_json::json;

pub struct GeneralUserBot {
    id: String,
    context: Option<BotContext>,
}

impl GeneralUserBot {
    pub fn new(id: String) -> Self {
        Self { id, context: None }
    }

    fn api_base(&self) -> String {
        self.context
            .as_ref()
            .map(|c| c.execution.network.alphaos_rest.clone())
            .unwrap_or_else(|| "http://localhost:3030".to_string())
    }

    async fn submit_tx(&self) -> Result<BehaviorResult> {
        let api_base = self.api_base();
        let url = format!("{}/api/v1/transactions", api_base);

        let payload = json!({
            "from": format!("bot_{}", self.id),
            "to": "0x0000000000000000000000000000000000000001",
            "amount": "1",
            "nonce": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| json!({"raw": "non-json response"}));

        if status.is_success() {
            let tx_hash = body.get("tx_hash").cloned().unwrap_or(json!("unknown"));
            tracing::info!("GeneralUserBot {} submit_tx OK: {}", self.id, tx_hash);
            Ok(BehaviorResult::success(format!("submit_tx accepted: {}", tx_hash)).with_data(body))
        } else {
            tracing::warn!(
                "GeneralUserBot {} submit_tx HTTP {}: {:?}",
                self.id,
                status,
                body
            );
            Ok(BehaviorResult::error(format!(
                "submit_tx HTTP {}: {}",
                status,
                body.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
            ))
            .with_data(body))
        }
    }

    async fn query_balance(&self) -> Result<BehaviorResult> {
        let api_base = self.api_base();
        let account = format!("bot_{}", self.id);
        let url = format!("{}/api/v1/accounts/{}/balance", api_base, account);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| json!({"raw": "non-json response"}));

        if status.is_success() {
            let balance = body.get("balance").cloned().unwrap_or(json!("unknown"));
            tracing::info!("GeneralUserBot {} query_balance: {}", self.id, balance);
            Ok(BehaviorResult::success(format!("balance: {}", balance)).with_data(body))
        } else {
            tracing::warn!(
                "GeneralUserBot {} query_balance HTTP {}: {:?}",
                self.id,
                status,
                body
            );
            Ok(BehaviorResult::error(format!("query_balance HTTP {}", status)).with_data(body))
        }
    }

    async fn check_status(&self) -> Result<BehaviorResult> {
        let api_base = self.api_base();
        let url = format!("{}/health", api_base);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| json!({"status": status.as_u16()}));

        if status.is_success() {
            tracing::info!("GeneralUserBot {} check_status: node healthy", self.id);
            Ok(BehaviorResult::success("node healthy").with_data(body))
        } else {
            tracing::warn!(
                "GeneralUserBot {} check_status: node unhealthy HTTP {}",
                self.id,
                status
            );
            Ok(BehaviorResult::error(format!("node unhealthy: HTTP {}", status)).with_data(body))
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
        tracing::info!(
            "GeneralUserBot {} executing behavior: {}",
            self.id,
            behavior_id
        );

        match behavior_id {
            "submit_tx" => self.submit_tx().await,
            "query_balance" => self.query_balance().await,
            "check_status" => self.check_status().await,
            other => Err(BotError::BehaviorError(format!("Unknown behavior_id: {}", other)).into()),
        }
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
