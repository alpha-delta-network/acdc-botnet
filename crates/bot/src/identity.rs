/// Multi-chain identity generation for Alpha and Delta chains
///
/// Generates cryptographic identities (keypairs and addresses) for both
/// Alpha (ax1 bech32 addresses) and Delta (dx1 bech32 addresses) chains.

use crate::{BotError, Result};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use blake2::{Blake2s256, Digest};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// A multi-chain identity for Alpha and Delta protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Unique identifier
    pub id: String,

    /// Alpha chain address (ax1...)
    pub alpha_address: String,

    /// Delta chain address (dx1...)
    pub delta_address: String,

    /// Private key (stored as bytes, careful with serialization)
    #[serde(skip)]
    signing_key: Option<SigningKey>,
}

impl Identity {
    /// Create a new identity from a signing key
    pub fn from_signing_key(id: String, signing_key: SigningKey) -> Result<Self> {
        let verifying_key = signing_key.verifying_key();

        // Generate addresses for both chains
        let alpha_address = Self::generate_address(&verifying_key, "ax")?;
        let delta_address = Self::generate_address(&verifying_key, "dx")?;

        Ok(Self {
            id,
            alpha_address,
            delta_address,
            signing_key: Some(signing_key),
        })
    }

    /// Generate a bech32 address from a verifying key
    fn generate_address(verifying_key: &VerifyingKey, prefix: &str) -> Result<String> {
        // Hash the public key
        let mut hasher = Blake2s256::new();
        hasher.update(verifying_key.as_bytes());
        let hash = hasher.finalize();

        // Take first 32 bytes (full hash for Blake2s256)
        let address_bytes = &hash[..];

        // Convert to bech32
        // Note: For production, use proper bech32 encoding
        // This is a simplified version for the testbot framework
        let encoded = bech32::encode(
            prefix,
            address_bytes.to_vec(),
            bech32::Variant::Bech32,
        ).map_err(|e| BotError::IdentityError(format!("Bech32 encoding failed: {}", e)))?;

        Ok(encoded)
    }

    /// Sign a message with this identity
    pub fn sign(&self, message: &[u8]) -> Result<Signature> {
        let signing_key = self.signing_key.as_ref()
            .ok_or_else(|| BotError::IdentityError("No signing key available".to_string()))?;

        Ok(signing_key.sign(message))
    }

    /// Get the verifying key
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let signing_key = self.signing_key.as_ref()
            .ok_or_else(|| BotError::IdentityError("No signing key available".to_string()))?;

        Ok(signing_key.verifying_key())
    }

    /// Create a view-only identity (no signing capability)
    pub fn view_only(id: String, alpha_address: String, delta_address: String) -> Self {
        Self {
            id,
            alpha_address,
            delta_address,
            signing_key: None,
        }
    }

    /// Check if this identity can sign
    pub fn can_sign(&self) -> bool {
        self.signing_key.is_some()
    }
}

/// Generator for creating new identities
pub struct IdentityGenerator {
    /// Deterministic seed for reproducible bot generation
    seed: Option<u64>,
}

impl IdentityGenerator {
    /// Create a new identity generator
    pub fn new() -> Self {
        Self { seed: None }
    }

    /// Create a generator with a deterministic seed
    pub fn with_seed(seed: u64) -> Self {
        Self { seed: Some(seed) }
    }

    /// Generate a new identity
    pub fn generate(&self, bot_id: String) -> Result<Identity> {
        // For now, use OS random source
        // TODO: Add deterministic generation from seed for reproducibility
        let signing_key = SigningKey::generate(&mut OsRng);

        Identity::from_signing_key(bot_id, signing_key)
    }

    /// Generate multiple identities
    pub fn generate_batch(&self, prefix: &str, count: usize) -> Result<Vec<Identity>> {
        let mut identities = Vec::with_capacity(count);

        for i in 0..count {
            let bot_id = format!("{}-{}", prefix, i);
            identities.push(self.generate(bot_id)?);
        }

        Ok(identities)
    }
}

impl Default for IdentityGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let generator = IdentityGenerator::new();
        let identity = generator.generate("test-bot-1".to_string())
            .expect("Failed to generate identity");

        assert!(identity.alpha_address.starts_with("ax"));
        assert!(identity.delta_address.starts_with("dx"));
        assert!(identity.can_sign());
    }

    #[test]
    fn test_batch_generation() {
        let generator = IdentityGenerator::new();
        let identities = generator.generate_batch("bot", 10)
            .expect("Failed to generate batch");

        assert_eq!(identities.len(), 10);

        for (i, identity) in identities.iter().enumerate() {
            assert_eq!(identity.id, format!("bot-{}", i));
            assert!(identity.can_sign());
        }
    }

    #[test]
    fn test_view_only_identity() {
        let identity = Identity::view_only(
            "view-bot".to_string(),
            "ax1test123".to_string(),
            "dx1test456".to_string(),
        );

        assert!(!identity.can_sign());
        assert!(identity.sign(&[1, 2, 3]).is_err());
    }

    #[test]
    fn test_signing() {
        let generator = IdentityGenerator::new();
        let identity = generator.generate("signer-bot".to_string())
            .expect("Failed to generate identity");

        let message = b"test message";
        let signature = identity.sign(message)
            .expect("Failed to sign message");

        // Verify the signature
        let verifying_key = identity.verifying_key()
            .expect("Failed to get verifying key");

        assert!(verifying_key.verify(message, &signature).is_ok());
    }
}
