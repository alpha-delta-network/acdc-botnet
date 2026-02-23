/// Adnet CLI client
///
/// Wrapper for executing adnet CLI commands

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

/// Adnet CLI client
pub struct AdnetClient {
    binary_path: String,
}

impl AdnetClient {
    /// Create a new Adnet CLI client
    pub fn new(binary_path: String) -> Self {
        Self { binary_path }
    }

    /// Create account
    pub fn create_account(&self, name: &str) -> Result<AccountInfo> {
        let output = self.execute(&["account", "new", "--name", name])?;
        self.parse_json_output(&output)
    }

    /// Get account info
    pub fn get_account(&self, address: &str) -> Result<AccountInfo> {
        let output = self.execute(&["account", "get", address])?;
        self.parse_json_output(&output)
    }

    /// Transfer tokens
    pub fn transfer(&self, from: &str, to: &str, amount: u64, token: &str) -> Result<String> {
        let amount_str = amount.to_string();
        let output = self.execute(&[
            "account",
            "transfer",
            "--from",
            from,
            "--to",
            to,
            "--amount",
            &amount_str,
            "--token",
            token,
        ])?;

        self.extract_transaction_id(&output)
    }

    /// Submit DEX order
    pub fn trade_order(
        &self,
        account: &str,
        pair: &str,
        side: &str,
        amount: &str,
        price: Option<&str>,
    ) -> Result<String> {
        let mut args = vec![
            "trade",
            "order",
            "--account",
            account,
            "--pair",
            pair,
            "--side",
            side,
            "--amount",
            amount,
        ];

        if let Some(p) = price {
            args.extend_from_slice(&["--price", p]);
        }

        let output = self.execute(&args)?;
        self.extract_order_id(&output)
    }

    /// Cancel DEX order
    pub fn trade_cancel(&self, order_id: &str) -> Result<()> {
        self.execute(&["trade", "cancel", "--order", order_id])?;
        Ok(())
    }

    /// Get validator info
    pub fn get_validator(&self, address: &str) -> Result<ValidatorInfo> {
        let output = self.execute(&["validator", "info", address])?;
        self.parse_json_output(&output)
    }

    /// Claim rewards
    pub fn claim_rewards(&self, validator: &str) -> Result<String> {
        let output = self.execute(&["rewards", "claim", "--validator", validator])?;
        self.extract_transaction_id(&output)
    }

    /// Execute a command
    fn execute(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new(&self.binary_path)
            .args(args)
            .output()
            .context("Failed to execute adnet command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Command failed: {}", stderr);
        }

        Ok(output)
    }

    /// Parse JSON output
    fn parse_json_output<T: for<'de> Deserialize<'de>>(&self, output: &Output) -> Result<T> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).context("Failed to parse JSON output")
    }

    /// Extract transaction ID from output
    fn extract_transaction_id(&self, output: &Output) -> Result<String> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Simple extraction - assumes format "Transaction ID: <id>"
        stdout
            .lines()
            .find(|line| line.contains("Transaction ID:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|id| id.trim().to_string())
            .context("Failed to extract transaction ID")
    }

    /// Extract order ID from output
    fn extract_order_id(&self, output: &Output) -> Result<String> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Simple extraction - assumes format "Order ID: <id>"
        stdout
            .lines()
            .find(|line| line.contains("Order ID:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|id| id.trim().to_string())
            .context("Failed to extract order ID")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub address: String,
    pub balance_ax: u64,
    pub balance_sax: u64,
    pub balance_dx: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub address: String,
    pub stake: u64,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = AdnetClient::new("/usr/local/bin/adnet".to_string());
        assert_eq!(client.binary_path, "/usr/local/bin/adnet");
    }
}
