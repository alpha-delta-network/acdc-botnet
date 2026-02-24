/// DeltaOS REST API client
///
/// Provides HTTP client for DeltaOS DEX, perpetuals, and oracle endpoints
use anyhow::{Context, Result};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// DeltaOS REST API client
pub struct DeltaOSClient {
    base_url: String,
    client: Client,
}

impl DeltaOSClient {
    /// Create a new DeltaOS client
    pub fn new(base_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { base_url, client })
    }

    /// Get the latest block height
    pub async fn get_latest_block_height(&self) -> Result<u32> {
        let url = format!("{}/mainnet/block/height/latest", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response::<u32>(response).await
    }

    /// Get block by height
    pub async fn get_block(&self, height: u32) -> Result<serde_json::Value> {
        let url = format!("{}/mainnet/block/{}", self.base_url, height);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Broadcast a transaction
    pub async fn broadcast_transaction(&self, tx_bytes: &[u8]) -> Result<String> {
        let url = format!("{}/mainnet/transaction/broadcast", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(tx_bytes.to_vec())
            .send()
            .await?;

        self.handle_response::<BroadcastResponse>(response)
            .await
            .map(|r| r.transaction_id)
    }

    /// Get orderbook for a trading pair
    pub async fn get_orderbook(&self, pair: &str) -> Result<Orderbook> {
        let url = format!("{}/mainnet/dex/orderbook/{}", self.base_url, pair);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Submit a DEX order
    pub async fn submit_order(&self, order: &Order) -> Result<String> {
        let url = format!("{}/mainnet/dex/order", self.base_url);
        let response = self.client.post(&url).json(order).send().await?;

        self.handle_response::<OrderResponse>(response)
            .await
            .map(|r| r.order_id)
    }

    /// Cancel a DEX order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/mainnet/dex/order/{}/cancel", self.base_url, order_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response::<()>(response).await
    }

    /// Get open positions for an address
    pub async fn get_positions(&self, address: &str) -> Result<Vec<Position>> {
        let url = format!("{}/mainnet/perpetuals/positions/{}", self.base_url, address);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Open a perpetual position
    pub async fn open_position(&self, position: &PositionRequest) -> Result<String> {
        let url = format!("{}/mainnet/perpetuals/open", self.base_url);
        let response = self.client.post(&url).json(position).send().await?;

        self.handle_response::<PositionResponse>(response)
            .await
            .map(|r| r.position_id)
    }

    /// Close a perpetual position
    pub async fn close_position(&self, position_id: &str) -> Result<()> {
        let url = format!("{}/mainnet/perpetuals/close/{}", self.base_url, position_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response::<()>(response).await
    }

    /// Get oracle price for an asset
    pub async fn get_oracle_price(&self, asset: &str) -> Result<OraclePrice> {
        let url = format!("{}/mainnet/oracle/price/{}", self.base_url, asset);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get mempool transactions
    pub async fn get_mempool_transactions(&self) -> Result<Vec<String>> {
        let url = format!("{}/mainnet/memoryPool/transactions", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Handle response and deserialize
    async fn handle_response<T: for<'de> Deserialize<'de>>(&self, response: Response) -> Result<T> {
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("HTTP error {}: {}", status, error_text);
        }

        let body = response
            .text()
            .await
            .context("Failed to read response body")?;
        serde_json::from_str(&body).context("Failed to deserialize response")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastResponse {
    transaction_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    pub pair: String,
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub pair: String,
    pub side: String,       // "buy" or "sell"
    pub order_type: String, // "limit" or "market"
    pub price: Option<String>,
    pub quantity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderResponse {
    order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub position_id: String,
    pub pair: String,
    pub side: String, // "long" or "short"
    pub size: String,
    pub entry_price: String,
    pub liquidation_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRequest {
    pub pair: String,
    pub side: String,
    pub size: String,
    pub leverage: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PositionResponse {
    position_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePrice {
    pub asset: String,
    pub price: String,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = DeltaOSClient::new("http://localhost:3031".to_string());
        assert!(client.is_ok());
    }
}
