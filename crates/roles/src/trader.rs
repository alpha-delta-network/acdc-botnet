/// Trader bot role
///
/// Simulates DEX trading operations via the Delta execute endpoint.
use adnet_testbot::{BehaviorResult, Bot, BotContext, BotError, Result};
use adnet_testbot_integration::AdnetClient;
use async_trait::async_trait;
use bech32::{ToBase32, Variant};
use serde_json::json;
use sha2::{Digest, Sha256};

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

/// Derive a deterministic Delta private key (dp1... bech32m) from a bot ID.
///
/// Encoding: bech32m(HRP="dp", sk_sig_bytes_le || r_sig_bytes_le) -- 64 bytes total.
/// Both halves are derived deterministically via SHA-256 with distinct domain tags.
fn derive_trader_key(bot_id: &str) -> String {
    let sk_hash = Sha256::digest(format!("delta_sk:{}", bot_id).as_bytes());
    let r_hash = Sha256::digest(format!("delta_r:{}", bot_id).as_bytes());
    let mut payload = [0u8; 64];
    payload[..32].copy_from_slice(&sk_hash);
    payload[32..].copy_from_slice(&r_hash);
    bech32::encode("dp", payload.to_base32(), Variant::Bech32m)
        .expect("bech32m encoding of trader key failed")
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
                let private_key = derive_trader_key(&self.id);
                let transaction_id = client
                    .execute_delta_transaction(
                        "dex.delta",
                        "place_order",
                        vec![
                            "AX/DX".to_string(),
                            "buy".to_string(),
                            "market".to_string(),
                            "100".to_string(),
                        ],
                        &private_key,
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
