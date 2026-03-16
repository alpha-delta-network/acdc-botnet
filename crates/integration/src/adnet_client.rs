/// AdnetClient — unified adnet API client for acdc-botnet.
///
/// Routes all requests through adnet (Alpha: port 3030, Delta: port 4030).
/// This replaces the direct AlphaOS/DeltaOS REST clients.
use anyhow::{Context, Result};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unified adnet REST API client
pub struct AdnetClient {
    base_url: String,
    client: Client,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StateRoot {
    pub state_root: Option<String>,
    pub root: Option<String>,
    pub hash: Option<String>,
    pub height: Option<u64>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ValidatorListResponse {
    pub validators: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MempoolResponse {
    pub size: Option<u64>,
    pub pending_count: Option<u64>,
    pub total: Option<u64>,
    pub transactions: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VersionResponse {
    pub version: Option<String>,
    pub adnet_version: Option<String>,
    pub chain: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GovernanceProposalsResponse {
    pub proposals: Option<Vec<serde_json::Value>>,
    pub total: Option<u64>,
}

impl AdnetClient {
    /// Create a new adnet client.
    ///
    /// `base_url` should be the adnet API URL, e.g. `https://testnet.ac-dc.network:3030`
    pub fn new(base_url: String) -> Result<Self> {
        Self::with_api_key(base_url, None)
    }

    pub fn with_api_key(base_url: String, api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self {
            base_url,
            client,
            api_key,
        })
    }

    fn auth_header(&self) -> Vec<(String, String)> {
        if let Some(key) = &self.api_key {
            vec![("X-Api-Key".to_string(), key.clone())]
        } else {
            vec![]
        }
    }

    async fn get_json<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);
        for (k, v) in self.auth_header() {
            req = req.header(k, v);
        }
        let response = req.send().await.context(format!("GET {}", path))?;
        if !response.status().is_success() {
            anyhow::bail!("GET {} returned {}", path, response.status());
        }
        response
            .json::<T>()
            .await
            .context(format!("parse response from {}", path))
    }

    async fn post_json<T: for<'de> serde::Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url).json(body);
        for (k, v) in self.auth_header() {
            req = req.header(k, v);
        }
        let response = req.send().await.context(format!("POST {}", path))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "POST {} returned {}: {}",
                path,
                status,
                &body[..body.len().min(200)]
            );
        }
        response
            .json::<T>()
            .await
            .context(format!("parse response from {}", path))
    }

    // ── Chain state ────────────────────────────────────────────────────────

    /// Get the current state root (maps to GET /api/v1/state/root)
    pub async fn get_state_root(&self) -> Result<StateRoot> {
        self.get_json("/api/v1/state/root").await
    }

    /// Get current block height
    pub async fn get_latest_block_height(&self) -> Result<u64> {
        let root = self.get_state_root().await?;
        Ok(root.height.or(root.block_height).unwrap_or(0))
    }

    // ── Validators ─────────────────────────────────────────────────────────

    /// List all validators (GET /validators)
    pub async fn get_validators(&self) -> Result<Vec<serde_json::Value>> {
        let raw: serde_json::Value = self.get_json("/validators").await?;
        if let Some(arr) = raw.as_array() {
            return Ok(arr.clone());
        }
        if let Some(obj) = raw.as_object() {
            if let Some(arr) = obj.get("validators").and_then(|v| v.as_array()) {
                return Ok(arr.clone());
            }
        }
        Ok(vec![])
    }

    // ── Mempool ────────────────────────────────────────────────────────────

    /// Get mempool info (GET /api/v1/mempool)
    pub async fn get_mempool(&self) -> Result<MempoolResponse> {
        self.get_json("/api/v1/mempool").await
    }

    // ── Transactions ───────────────────────────────────────────────────────

    /// Submit a private transaction (POST /api/v1/transactions/submit/private)
    pub async fn submit_private_transaction(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post_json("/api/v1/transactions/submit/private", body)
            .await
    }

    /// Submit a public DEX transaction (POST /api/v1/transactions/submit/public)
    pub async fn submit_public_transaction(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post_json("/api/v1/transactions/submit/public", body)
            .await
    }

    // ── Prover pool ────────────────────────────────────────────────────────

    /// Get prover pool status (GET /api/v1/pool/status)
    pub async fn get_pool_status(&self) -> Result<serde_json::Value> {
        self.get_json("/api/v1/pool/status").await
    }

    /// Register a prover (POST /api/v1/prover/register)
    pub async fn register_prover(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        self.post_json("/api/v1/prover/register", body).await
    }

    // ── Governance ─────────────────────────────────────────────────────────

    /// List governance proposals (GET /api/v1/governance/proposals)
    pub async fn get_governance_proposals(&self) -> Result<GovernanceProposalsResponse> {
        self.get_json("/api/v1/governance/proposals").await
    }

    /// Get a specific governance proposal (GET /api/v1/governance/proposals/:id)
    pub async fn get_governance_proposal(&self, id: u64) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/v1/governance/proposals/{}", id))
            .await
    }

    /// Submit a governance vote (POST /api/v1/governance/proposals/:id/vote)
    pub async fn submit_governance_vote(
        &self,
        proposal_id: u64,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post_json(
            &format!("/api/v1/governance/proposals/{}/vote", proposal_id),
            body,
        )
        .await
    }

    // ── Slash evidence ─────────────────────────────────────────────────────

    /// Submit slash evidence (POST /api/v1/validator/slash-evidence)
    pub async fn submit_slash_evidence(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post_json("/api/v1/validator/slash-evidence", body)
            .await
    }

    // ── Version / health ───────────────────────────────────────────────────

    /// Get version info (GET /api/v1/version)
    pub async fn get_version(&self) -> Result<VersionResponse> {
        self.get_json("/api/v1/version").await
    }

    /// Check if adnet is reachable
    pub async fn is_alive(&self) -> bool {
        self.get_version().await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_constructs() {
        let c = AdnetClient::new("http://localhost:3030".to_string());
        assert!(c.is_ok());
    }

    #[test]
    fn test_client_with_api_key() {
        let c = AdnetClient::with_api_key(
            "http://localhost:3030".to_string(),
            Some("test-key".to_string()),
        );
        assert!(c.is_ok());
    }
}
