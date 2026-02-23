/// AlphaOS REST API client
///
/// Provides a comprehensive HTTP client for all AlphaOS endpoints

use anyhow::{Context, Result};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// AlphaOS REST API client
pub struct AlphaOSClient {
    base_url: String,
    client: Client,
}

impl AlphaOSClient {
    /// Create a new AlphaOS client
    pub fn new(base_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { base_url, client })
    }

    /// Get the latest block height
    pub async fn get_latest_block_height(&self) -> Result<u32> {
        let url = format!("{}/testnet/block/height/latest", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response::<u32>(response).await
    }

    /// Get block by height
    pub async fn get_block(&self, height: u32) -> Result<serde_json::Value> {
        let url = format!("{}/testnet/block/{}", self.base_url, height);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Broadcast a transaction
    pub async fn broadcast_transaction(&self, tx_bytes: &[u8]) -> Result<String> {
        let url = format!("{}/testnet/transaction/broadcast", self.base_url);
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(tx_bytes.to_vec())
            .send()
            .await?;

        self.handle_response::<BroadcastResponse>(response)
            .await
            .map(|r| r.transaction_id)
    }

    /// Get transaction by ID
    pub async fn get_transaction(&self, tx_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/testnet/transaction/{}", self.base_url, tx_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get mempool transactions
    pub async fn get_mempool_transactions(&self) -> Result<Vec<String>> {
        let url = format!("{}/testnet/memoryPool/transactions", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get program by ID
    pub async fn get_program(&self, program_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/testnet/program/{}", self.base_url, program_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get state root
    pub async fn get_state_root(&self) -> Result<String> {
        let url = format!("{}/testnet/stateRoot/latest", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get committee at round
    pub async fn get_committee(&self, round: u64) -> Result<serde_json::Value> {
        let url = format!("{}/testnet/committee/{}", self.base_url, round);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get governors
    pub async fn get_governors(&self) -> Result<Vec<String>> {
        let url = format!("{}/testnet/governors", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get peers
    pub async fn get_peers(&self) -> Result<Vec<String>> {
        let url = format!("{}/testnet/peers/all", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> Result<SyncStatus> {
        let url = format!("{}/testnet/sync_status", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Handle response and deserialize
    async fn handle_response<T: for<'de> Deserialize<'de>>(&self, response: Response) -> Result<T> {
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("HTTP error {}: {}", status, error_text);
        }

        let body = response.text().await.context("Failed to read response body")?;
        serde_json::from_str(&body).context("Failed to deserialize response")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastResponse {
    transaction_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub current_height: u32,
    pub target_height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = AlphaOSClient::new("http://localhost:3030".to_string());
        assert!(client.is_ok());
    }
}
