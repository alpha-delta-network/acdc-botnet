/// General user bot role
///
/// Simulates regular user operations like transfers and balance queries.
///
/// `submit_tx` targets the gauntlet endpoints `/api/v1/{alpha,delta}/tx`
/// (wired to M1 in adnet PR #612). These accept Bearer ak_* auth, accept
/// hex-encoded tx bytes, and queue a TxProofTask in the shared ProofMempool —
/// so a bot population running this behavior at rate N generates ~N tx-proof
/// jobs per second, driving the real Nova IVC prover.
use adnet_testbot::{BehaviorResult, Bot, BotContext, BotError, Result};
use async_trait::async_trait;
use rand::RngCore;
use serde_json::json;

/// Default gauntlet auth token. Any "Bearer ak_*" passes check_ak_auth.
/// Load-test harness auth; real wallet Schnorr is required at /submit/private.
const DEFAULT_AK_TOKEN: &str = "Bearer ak_botnet_general_user_default";

pub struct GeneralUserBot {
    id: String,
    context: Option<BotContext>,
}

impl GeneralUserBot {
    pub fn new(id: String) -> Self {
        Self { id, context: None }
    }

    /// Unified-API base URL (port 8080). The scenarios' alphaos_rest field
    /// historically pointed at the alpha REST port 3030, but the gauntlet
    /// endpoints live on the unified API. We rewrite 3030→8080 if needed.
    fn api_base(&self) -> String {
        let raw = self
            .context
            .as_ref()
            .map(|c| c.execution.network.alphaos_rest.clone())
            .unwrap_or_else(|| "http://localhost:3030".to_string());
        // Canonicalize to unified API port.
        raw.replace(":3030", ":8080").replace(":3031", ":8080")
    }

    /// Generate a 64-hex-char random payload ("encrypted tx" surrogate).
    fn random_hex_payload() -> String {
        let mut b = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut b);
        hex::encode(b)
    }

    /// Alpha private-transfer simulation via gauntlet endpoint.
    async fn submit_alpha_tx(&self) -> Result<BehaviorResult> {
        self.submit_gauntlet_tx("alpha").await
    }

    /// Delta private-transfer simulation via gauntlet endpoint.
    async fn submit_delta_tx(&self) -> Result<BehaviorResult> {
        self.submit_gauntlet_tx("delta").await
    }

    async fn submit_gauntlet_tx(&self, chain: &str) -> Result<BehaviorResult> {
        let api_base = self.api_base();
        let url = format!("{}/api/v1/{}/tx", api_base, chain);
        let payload = json!({"tx": Self::random_hex_payload()});

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| BotError::NetworkError(e.to_string()))?;

        let resp = client
            .post(&url)
            .header("Authorization", DEFAULT_AK_TOKEN)
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
            let tx_id = body.get("tx_id").cloned().unwrap_or(json!("unknown"));
            tracing::info!(
                "GeneralUserBot {} submit_{}_tx OK: {}",
                self.id,
                chain,
                tx_id
            );
            Ok(
                BehaviorResult::success(format!("submit_{}_tx accepted: {}", chain, tx_id))
                    .with_data(body),
            )
        } else {
            tracing::warn!(
                "GeneralUserBot {} submit_{}_tx HTTP {}: {:?}",
                self.id,
                chain,
                status,
                body
            );
            Ok(
                BehaviorResult::error(format!("submit_{}_tx HTTP {}", chain, status))
                    .with_data(body),
            )
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
            // "default" is what scenario runners pass when a scenario file doesn't
            // specify a behavior_id. Historically a no-op; now it actually drives
            // load: alternates alpha/delta tx submissions, generating Nova IVC
            // proof jobs in the shared ProofMempool.
            // "default" / "submit_tx" / "transfer.ax" all alias to alpha submission.
            // Scenario YAML historically uses several of these labels.
            "default" | "submit_tx" | "submit_alpha_tx" | "transfer.ax" => {
                self.submit_alpha_tx().await
            }
            "submit_delta_tx" => self.submit_delta_tx().await,
            "query_balance" => self.query_balance().await,
            "check_status" | "query.block_height" => self.check_status().await,
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
