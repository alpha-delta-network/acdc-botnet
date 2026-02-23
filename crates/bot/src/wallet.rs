/// Wallet management for tracking balances across Alpha and Delta chains
///
/// Supports:
/// - AX (Alpha native token)
/// - sAX (Shielded AX on Delta)
/// - DX (Delta native token)

use crate::{BotError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chain identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainId {
    Alpha,
    Delta,
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainId::Alpha => write!(f, "alpha"),
            ChainId::Delta => write!(f, "delta"),
        }
    }
}

/// Token identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Token {
    /// Alpha native token
    AX,
    /// Shielded AX on Delta
    SAX,
    /// Delta native token
    DX,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::AX => write!(f, "AX"),
            Token::SAX => write!(f, "sAX"),
            Token::DX => write!(f, "DX"),
        }
    }
}

/// Balance amount (using u128 for large values)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    amount: u128,
}

impl Balance {
    pub fn new(amount: u128) -> Self {
        Self { amount }
    }

    pub fn zero() -> Self {
        Self { amount: 0 }
    }

    pub fn amount(&self) -> u128 {
        self.amount
    }

    pub fn add(&self, other: Balance) -> Result<Balance> {
        self.amount
            .checked_add(other.amount)
            .map(Balance::new)
            .ok_or_else(|| BotError::WalletError("Balance overflow".to_string()))
    }

    pub fn sub(&self, other: Balance) -> Result<Balance> {
        self.amount
            .checked_sub(other.amount)
            .map(Balance::new)
            .ok_or_else(|| BotError::WalletError("Insufficient balance".to_string()))
    }

    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }
}

impl std::fmt::Display for Balance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.amount)
    }
}

/// Multi-chain wallet for bot accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    /// Owner's bot ID
    pub owner_id: String,

    /// Balances per token
    balances: HashMap<Token, Balance>,

    /// Pending operations (transaction hashes)
    pending_ops: Vec<String>,
}

impl Wallet {
    /// Create a new wallet
    pub fn new(owner_id: String) -> Self {
        let mut balances = HashMap::new();
        balances.insert(Token::AX, Balance::zero());
        balances.insert(Token::SAX, Balance::zero());
        balances.insert(Token::DX, Balance::zero());

        Self {
            owner_id,
            balances,
            pending_ops: Vec::new(),
        }
    }

    /// Create a wallet with initial balances
    pub fn with_balances(owner_id: String, initial: HashMap<Token, Balance>) -> Self {
        let mut wallet = Self::new(owner_id);
        for (token, balance) in initial {
            wallet.balances.insert(token, balance);
        }
        wallet
    }

    /// Get balance for a token
    pub fn balance(&self, token: &Token) -> Balance {
        self.balances.get(token).copied().unwrap_or_else(Balance::zero)
    }

    /// Credit (add) to balance
    pub fn credit(&mut self, token: Token, amount: Balance) -> Result<()> {
        let current = self.balance(&token);
        let new_balance = current.add(amount)?;
        self.balances.insert(token, new_balance);
        Ok(())
    }

    /// Debit (subtract) from balance
    pub fn debit(&mut self, token: Token, amount: Balance) -> Result<()> {
        let current = self.balance(&token);
        let new_balance = current.sub(amount)?;
        self.balances.insert(token, new_balance);
        Ok(())
    }

    /// Check if wallet has sufficient balance
    pub fn has_balance(&self, token: &Token, amount: Balance) -> bool {
        self.balance(token).amount() >= amount.amount()
    }

    /// Add a pending operation
    pub fn add_pending_op(&mut self, tx_hash: String) {
        self.pending_ops.push(tx_hash);
    }

    /// Clear pending operations
    pub fn clear_pending_ops(&mut self) {
        self.pending_ops.clear();
    }

    /// Get all pending operations
    pub fn pending_ops(&self) -> &[String] {
        &self.pending_ops
    }

    /// Get total value across all tokens (simplified)
    /// Returns value in smallest units
    pub fn total_value(&self) -> u128 {
        self.balances.values().map(|b| b.amount()).sum()
    }

    /// Snapshot of all balances
    pub fn snapshot(&self) -> HashMap<Token, Balance> {
        self.balances.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new("test-bot".to_string());
        assert_eq!(wallet.balance(&Token::AX), Balance::zero());
        assert_eq!(wallet.balance(&Token::SAX), Balance::zero());
        assert_eq!(wallet.balance(&Token::DX), Balance::zero());
    }

    #[test]
    fn test_credit_debit() {
        let mut wallet = Wallet::new("test-bot".to_string());

        // Credit 1000 AX
        wallet.credit(Token::AX, Balance::new(1000)).unwrap();
        assert_eq!(wallet.balance(&Token::AX).amount(), 1000);

        // Debit 300 AX
        wallet.debit(Token::AX, Balance::new(300)).unwrap();
        assert_eq!(wallet.balance(&Token::AX).amount(), 700);
    }

    #[test]
    fn test_insufficient_balance() {
        let mut wallet = Wallet::new("test-bot".to_string());
        wallet.credit(Token::AX, Balance::new(100)).unwrap();

        // Try to debit more than available
        let result = wallet.debit(Token::AX, Balance::new(200));
        assert!(result.is_err());
    }

    #[test]
    fn test_has_balance() {
        let mut wallet = Wallet::new("test-bot".to_string());
        wallet.credit(Token::AX, Balance::new(1000)).unwrap();

        assert!(wallet.has_balance(&Token::AX, Balance::new(500)));
        assert!(wallet.has_balance(&Token::AX, Balance::new(1000)));
        assert!(!wallet.has_balance(&Token::AX, Balance::new(1001)));
    }

    #[test]
    fn test_pending_ops() {
        let mut wallet = Wallet::new("test-bot".to_string());

        wallet.add_pending_op("tx_hash_1".to_string());
        wallet.add_pending_op("tx_hash_2".to_string());

        assert_eq!(wallet.pending_ops().len(), 2);

        wallet.clear_pending_ops();
        assert_eq!(wallet.pending_ops().len(), 0);
    }

    #[test]
    fn test_balance_operations() {
        let b1 = Balance::new(100);
        let b2 = Balance::new(50);

        let sum = b1.add(b2).unwrap();
        assert_eq!(sum.amount(), 150);

        let diff = b1.sub(b2).unwrap();
        assert_eq!(diff.amount(), 50);
    }
}
