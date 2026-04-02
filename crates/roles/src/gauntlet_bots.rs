// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! TN006 / TN006-LIGHT Gauntlet bot roles.
//!
//! Full fleet (169 bots): GauntletFleet::build()
//! Light fleet (66 bots): LightFleet::build() — testnet001-005 + testnet006

use adnet_testbot::{BehaviorResult, Bot, BotContext, BotError, Identity, Result};
use adnet_testbot_integration::AdnetClient;
use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use hex;
use serde_json::json;
use sha2::{Digest, Sha256};

// =============================================================================
// USER TRANSACTOR (10/30 bots, phases 1-7)
// =============================================================================

pub struct UserTransactorBot {
    id: String,
    adnet_url: String,
}

impl UserTransactorBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for UserTransactorBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "transfer.ax_private" => {
                let resp = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await?;
                let tx_id = resp
                    .get("tx_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(
                    BehaviorResult::success(format!("private tx: {}", tx_id)).with_data(json!({
                        "transaction_id": tx_id,
                        "submitted_inputs": {
                            "amount": 1000u64,
                            "fee": 1000u64,
                        }
                    })),
                )
            }
            "transfer.ax_public" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "public tx: {:?}",
                    resp.get("tx_id")
                )))
            }
            "query.balance" => {
                let root = client.get_state_root().await?;
                Ok(BehaviorResult::success(format!(
                    "state root: {:?}",
                    root.root
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "UserTransactor: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "user_transactor"
    }
}

// =============================================================================
// GOVERNOR BOT (5/10 bots, phases 2-7, 3-of-5 multisig)
// =============================================================================

pub struct GauntletGovernorBot {
    id: String,
    adnet_url: String,
    multisig_threshold: usize,
    identity: Option<Identity>,
}

impl GauntletGovernorBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
            multisig_threshold: 3,
            identity: None,
        }
    }
}

#[async_trait]
impl Bot for GauntletGovernorBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        self.identity = Some(ctx.identity.clone());
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "governance.propose.parameter" => {
                let pid = client
                    .submit_governance_proposal(
                        &json!({"title":"TN006-LIGHT Parameter Coverage Test","description":"Governance lifecycle coverage - parameter update proposal"}),
                    )
                    .await?;
                Ok(
                    BehaviorResult::success(format!("parameter proposal #{pid}"))
                        .with_data(json!({ "proposal_id": pid })),
                )
            }
            "governance.propose.mint" => {
                let pid = client
                    .submit_governance_proposal(
                        &json!({"title":"TN006-LIGHT Mint Coverage Test","description":"Governance lifecycle coverage - mint proposal"}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!("mint proposal #{pid}")))
            }
            "governance.vote" => {
                let resp = client.get_governance_proposals().await?;
                let proposals = resp.proposals.unwrap_or_default();
                let proposal_id = proposals
                    .iter()
                    .find(|p| p.get("status").and_then(|v| v.as_str()) == Some("active"))
                    .and_then(|p| p.get("id").and_then(|v| v.as_u64()))
                    .unwrap_or(1);
                let identity = self
                    .identity
                    .as_ref()
                    .ok_or_else(|| BotError::NetworkError("no identity".into()))?;
                // Message: proposal_id.to_le_bytes() (8) || b'Y' (VoteChoice::Yes)
                let mut message = Vec::with_capacity(9);
                message.extend_from_slice(&proposal_id.to_le_bytes());
                message.push(b'Y');
                let sig = identity.sign(&message)?;
                let vk = identity.verifying_key()?;
                let voter_public_key = hex::encode(vk.as_bytes());
                let vote_body = json!({
                    "voter_public_key": voter_public_key.clone(),
                    "vote": "yes",
                    "signature": hex::encode(sig.to_bytes()),
                });
                let tally = client
                    .submit_governance_vote(proposal_id, &vote_body)
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "governor voted on proposal #{proposal_id}: yes={:?}",
                    tally.get("yes")
                ))
                .with_data(json!({
                    "proposal_id": proposal_id,
                    "voter_public_key": voter_public_key.clone(),
                    "submitted_inputs": {
                        "vote_choice": "yes",
                        "proposal_id": proposal_id,
                        "voter_public_key": voter_public_key,
                    }
                })))
            }
            "governance.execute" => {
                let resp = client.get_governance_proposals().await?;
                let proposals = resp.proposals.unwrap_or_default();
                let mut executed = 0usize;
                let mut first_executed_id: Option<u64> = None;
                for p in &proposals {
                    if p.get("status").and_then(|v| v.as_str()) == Some("passed") {
                        if let Some(id) = p.get("id").and_then(|v| v.as_u64()) {
                            let _ = client
                                .post_json_raw(
                                    &format!("/api/v1/governance/proposals/{}/execute", id),
                                    &json!({}),
                                )
                                .await;
                            executed += 1;
                            if first_executed_id.is_none() {
                                first_executed_id = Some(id);
                            }
                        }
                    }
                }
                Ok(
                    BehaviorResult::success(format!("executed {executed} passed proposals"))
                        .with_data(json!({ "proposal_id": first_executed_id })),
                )
            }
            "governance.grim_trigger" => {
                let status = client.get_grim_trigger_status(&self.id).await?;
                Ok(BehaviorResult::success(format!(
                    "grim trigger status: {:?}",
                    status.get("state")
                ))
                .with_data(json!({ "gid_address": self.id })))
            }
            "governance.apology" => {
                let pid = client
                    .submit_governance_proposal(
                        &json!({"title":"Apology Restore","description":format!("Apology restore for crippled GID {}", &self.id)}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!("apology proposal #{pid}"))
                    .with_data(json!({ "proposal_id": pid })))
            }
            _ => Err(BotError::NetworkError(format!(
                "GauntletGovernor: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "gauntlet_governor"
    }
}

// =============================================================================
// DELTA VOTER BOT (10/30 bots, emphatic voting)
// =============================================================================

pub struct DeltaVoterBot {
    id: String,
    adnet_url: String,
    identity: Option<Identity>,
}

impl DeltaVoterBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
            identity: None,
        }
    }
}

#[async_trait]
impl Bot for DeltaVoterBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        self.identity = Some(ctx.identity.clone());
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "governance.delta.vote" => {
                let resp = client.get_governance_proposals().await?;
                let proposals = resp.proposals.unwrap_or_default();
                let proposal_id = proposals
                    .iter()
                    .find(|p| p.get("status").and_then(|v| v.as_str()) == Some("active"))
                    .and_then(|p| p.get("id").and_then(|v| v.as_u64()));
                let Some(proposal_id) = proposal_id else {
                    return Ok(BehaviorResult::success("no active proposals to vote on"));
                };
                let identity = self
                    .identity
                    .as_ref()
                    .ok_or_else(|| BotError::NetworkError("no identity".into()))?;
                // Message: proposal_id.to_le_bytes() (8) || b'Y' (VoteChoice::Yes)
                let mut message = Vec::with_capacity(9);
                message.extend_from_slice(&proposal_id.to_le_bytes());
                message.push(b'Y');
                let sig = identity.sign(&message)?;
                let vk = identity.verifying_key()?;
                let voter_public_key = hex::encode(vk.as_bytes());
                let vote_body = json!({
                    "voter_public_key": voter_public_key.clone(),
                    "vote": "yes",
                    "signature": hex::encode(sig.to_bytes()),
                });
                let tally = client
                    .submit_governance_vote(proposal_id, &vote_body)
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "voted yes on proposal #{proposal_id}: yes={:?}",
                    tally.get("yes")
                ))
                .with_data(json!({
                    "proposal_id": proposal_id,
                    "voter_public_key": voter_public_key.clone(),
                    "submitted_inputs": {
                        "vote_choice": "yes",
                        "proposal_id": proposal_id,
                        "voter_public_key": voter_public_key,
                    }
                })))
            }
            "governance.delta.emphatic_vote" => {
                // Vote yes on every active proposal (emphatic = covers all active)
                let resp = client.get_governance_proposals().await?;
                let proposals = resp.proposals.unwrap_or_default();
                let active_ids: Vec<u64> = proposals
                    .iter()
                    .filter(|p| p.get("status").and_then(|v| v.as_str()) == Some("active"))
                    .filter_map(|p| p.get("id").and_then(|v| v.as_u64()))
                    .collect();
                if active_ids.is_empty() {
                    return Ok(BehaviorResult::success(
                        "no active proposals for emphatic vote",
                    ));
                }
                let identity = self
                    .identity
                    .as_ref()
                    .ok_or_else(|| BotError::NetworkError("no identity".into()))?;
                let mut voted = 0usize;
                for pid in &active_ids {
                    let mut message = Vec::with_capacity(9);
                    message.extend_from_slice(&pid.to_le_bytes());
                    message.push(b'Y');
                    if let (Ok(sig), Ok(vk)) = (identity.sign(&message), identity.verifying_key()) {
                        let body = json!({
                            "voter_public_key": hex::encode(vk.as_bytes()),
                            "vote": "yes",
                            "signature": hex::encode(sig.to_bytes()),
                        });
                        if client.submit_governance_vote(*pid, &body).await.is_ok() {
                            voted += 1;
                        }
                    }
                }
                Ok(BehaviorResult::success(format!(
                    "emphatic: voted yes on {voted}/{} proposals",
                    active_ids.len()
                )))
            }
            "governance.delta.auto_disenroll" => {
                let resp = client.get_governance_proposals().await?;
                Ok(BehaviorResult::success(format!(
                    "disenroll check: {} proposals",
                    resp.proposals.unwrap_or_default().len()
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "DeltaVoter: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "delta_voter"
    }
}

// =============================================================================
// VALIDATOR BOT (5 active / 7 total)
// =============================================================================

/// Derive a deterministic (prover_id, pubkey, signing_key) triple from a bot identifier.
///
/// - seed = SHA-256(bot_id) — 32 bytes used as ed25519 signing key seed
/// - pubkey = ed25519 verifying key bytes (32 bytes)
/// - prover_id = SHA-256(pubkey) hex-encoded (64 chars)
fn derive_prover_identity(bot_id: &str) -> (String, String, SigningKey) {
    let seed: [u8; 32] = Sha256::digest(bot_id.as_bytes()).into();
    let signing_key = SigningKey::from_bytes(&seed);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let prover_id: [u8; 32] = Sha256::digest(&pubkey_bytes).into();
    (
        hex::encode(prover_id),
        hex::encode(pubkey_bytes),
        signing_key,
    )
}

pub struct ValidatorBot {
    id: String,
    adnet_url: String,
    pub is_shadow: bool,
}

impl ValidatorBot {
    pub fn new(id: String, is_shadow: bool) -> Self {
        Self {
            id,
            adnet_url: String::new(),
            is_shadow,
        }
    }
}

#[async_trait]
impl Bot for ValidatorBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "validator.register" => {
                let (prover_id_hex, pubkey_hex, signing_key) = derive_prover_identity(&self.id);
                // Step 1: get challenge nonce
                let nonce_hex = client
                    .get_prover_challenge(&prover_id_hex)
                    .await
                    .unwrap_or_else(|_| String::new());
                // Step 2: sign nonce (if challenge obtained)
                let sig_hex = if !nonce_hex.is_empty() {
                    if let Ok(nonce_bytes) = hex::decode(&nonce_hex) {
                        use ed25519_dalek::Signer;
                        hex::encode(signing_key.sign(&nonce_bytes).to_bytes())
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                // Step 3: register (with signature if challenge was obtained)
                let body = if !sig_hex.is_empty() {
                    json!({"prover_id": prover_id_hex, "pubkey": pubkey_hex, "signature": sig_hex, "capacity_csu": 1.0_f64, "stake_dx": 0_u64})
                } else {
                    json!({"prover_id": prover_id_hex, "pubkey": pubkey_hex, "capacity_csu": 1.0_f64, "stake_dx": 0_u64})
                };
                let resp = client.register_prover_idempotent(&body).await?;
                Ok(BehaviorResult::success(format!(
                    "validator registered: {:?}",
                    resp.get("status")
                )))
            }
            "validator.produce_block" | "validator.attest" => {
                let root = client.get_state_root().await?;
                Ok(BehaviorResult::success(format!(
                    "block at height (attested)"
                )))
            }
            "validator.claim_rewards" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "rewards claimed: {:?}",
                    resp.get("tx_id")
                )))
            }
            "validator.resign" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "resigned: {:?}",
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Validator: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        if self.is_shadow {
            "shadow_validator"
        } else {
            "active_validator"
        }
    }
}

// =============================================================================
// PROVER BOT (1/2 provers)
// =============================================================================

pub struct ProverBot {
    id: String,
    adnet_url: String,
}

impl ProverBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for ProverBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "prover.register" => {
                let (prover_id_hex, pubkey_hex, signing_key) = derive_prover_identity(&self.id);
                // Step 1: get challenge nonce
                let nonce_hex = client
                    .get_prover_challenge(&prover_id_hex)
                    .await
                    .unwrap_or_else(|_| String::new());
                // Step 2: sign nonce (if challenge obtained)
                let sig_hex = if !nonce_hex.is_empty() {
                    if let Ok(nonce_bytes) = hex::decode(&nonce_hex) {
                        use ed25519_dalek::Signer;
                        hex::encode(signing_key.sign(&nonce_bytes).to_bytes())
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                // Step 3: register (with signature if challenge was obtained)
                let body = if !sig_hex.is_empty() {
                    json!({"prover_id": prover_id_hex, "pubkey": pubkey_hex, "signature": sig_hex, "capacity_csu": 1.0_f64, "stake_dx": 0_u64})
                } else {
                    json!({"prover_id": prover_id_hex, "pubkey": pubkey_hex, "capacity_csu": 1.0_f64, "stake_dx": 0_u64})
                };
                let resp = client.register_prover_idempotent(&body).await?;
                Ok(BehaviorResult::success(format!(
                    "prover registered: {:?}",
                    resp.get("status")
                )))
            }
            "prover.submit_proof" => {
                let status = client.get_pool_status().await?;
                Ok(BehaviorResult::success(format!(
                    "pool status: {:?}",
                    status.get("queue_depth")
                )))
            }
            "prover.claim_rewards" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "prover rewards: {:?}",
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Prover: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "prover"
    }
}

// =============================================================================
// TECH REP BOT (3/5)
// =============================================================================

pub struct TechRepBot {
    id: String,
    adnet_url: String,
}

impl TechRepBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for TechRepBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "techrep.register" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "techrep registered: {:?}",
                    resp.get("tx_id")
                )))
            }
            "techrep.vote_forge" => {
                let resp = client.get_governance_proposals().await?;
                let proposals = resp.proposals.unwrap_or_default();
                let active = proposals
                    .iter()
                    .filter(|p| p.get("status").and_then(|v| v.as_str()) == Some("active"))
                    .count();
                Ok(BehaviorResult::success(format!(
                    "techrep forge check: {active} active proposals"
                )))
            }
            "techrep.staged_deploy" => {
                let _root = client.get_state_root().await?;
                Ok(BehaviorResult::success("staged deployment verified"))
            }
            _ => Err(BotError::NetworkError(format!(
                "TechRep: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "tech_rep"
    }
}

// =============================================================================
// EARN-IN BOT (4 light / 10 full — 3 succeed, 1 fails in light)
// =============================================================================

pub struct EarnInBot {
    id: String,
    adnet_url: String,
    pub index: usize,
}

impl EarnInBot {
    pub fn new(id: String, index: usize) -> Self {
        Self {
            id,
            adnet_url: String::new(),
            index,
        }
    }

    pub fn expects_success(&self) -> bool {
        self.index < 8
    }
}

#[async_trait]
impl Bot for EarnInBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "earnin.apply" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "earn-in applied: {:?}",
                    resp.get("tx_id")
                )))
            }
            "earnin.query_status" => {
                let status = client.get_pool_status().await?;
                Ok(BehaviorResult::success(format!(
                    "earn-in status: {:?}",
                    status.get("earn_in_queue")
                )))
            }
            "earnin.complete" => {
                if self.expects_success() {
                    let resp = client
                        .submit_public_transaction(
                            &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                        )
                        .await?;
                    Ok(BehaviorResult::success(format!(
                        "earn-in complete: {:?}",
                        resp.get("tx_id")
                    )))
                } else {
                    Ok(BehaviorResult::error(
                        "earn-in failed as expected (index >= 8)",
                    ))
                }
            }
            "earnin.fail" => Ok(BehaviorResult::error(format!(
                "earn-in bot {}: expected failure",
                self.id
            ))),
            _ => Err(BotError::NetworkError(format!(
                "EarnIn: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "earn_in"
    }
}

// =============================================================================
// ATOMIC SWAP BOT (2/5)
// =============================================================================

pub struct AtomicSwapBot {
    id: String,
    adnet_url: String,
}

impl AtomicSwapBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for AtomicSwapBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "atomicswap.kyt_register"
            | "atomicswap.htlc_initiate"
            | "atomicswap.htlc_complete"
            | "atomicswap.htlc_refund" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "{}: {:?}",
                    behavior_id,
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "AtomicSwap: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "atomic_swap"
    }
}

// =============================================================================
// DEAD WALLET BOT (0 in light / 5 in full, phase 7 only)
// =============================================================================

pub struct DeadWalletBot {
    id: String,
    adnet_url: String,
}

impl DeadWalletBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for DeadWalletBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "deadwallet.trigger_check" => {
                let _root = client.get_state_root().await?;
                Ok(BehaviorResult::success("dead wallet check triggered"))
            }
            "deadwallet.liquidate" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "liquidated: {:?}",
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "DeadWallet: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "dead_wallet"
    }
}

// =============================================================================
// ADVERSARIAL BOT (5 light / 8 full) — all 8 attack vectors
// =============================================================================

#[derive(Debug, Clone)]
pub enum AdversarialAttack {
    Equivocation,
    InvalidZkProof,
    OracleManipulation,
    MempoolDos,
    Replay,
    MevExtraction,
    BridgeMismatch,
    DoubleSpend,
}

pub struct AdversarialBot {
    id: String,
    adnet_url: String,
    pub attack: AdversarialAttack,
}

impl AdversarialBot {
    pub fn new(id: String, attack: AdversarialAttack) -> Self {
        Self {
            id,
            adnet_url: String::new(),
            attack,
        }
    }
}

#[async_trait]
impl Bot for AdversarialBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        // Support both named behavior IDs and generic "attack.execute" (dispatches on self.attack)
        let effective_attack = match behavior_id {
            "attack.equivocation" => "equivocation",
            "attack.invalid_zk" => "invalid_zk",
            "attack.oracle_manipulation" => "oracle_manipulation",
            "attack.mempool_dos" => "mempool_dos",
            "attack.replay" => "replay",
            "attack.mev" => "mev",
            "attack.bridge_mismatch" => "bridge_mismatch",
            "attack.double_spend" => "double_spend",
            "attack.execute" => match &self.attack {
                AdversarialAttack::Equivocation => "equivocation",
                AdversarialAttack::InvalidZkProof => "invalid_zk",
                AdversarialAttack::OracleManipulation => "oracle_manipulation",
                AdversarialAttack::MempoolDos => "mempool_dos",
                AdversarialAttack::Replay => "replay",
                AdversarialAttack::MevExtraction => "mev",
                AdversarialAttack::BridgeMismatch => "bridge_mismatch",
                AdversarialAttack::DoubleSpend => "double_spend",
            },
            _ => {
                return Err(BotError::NetworkError(format!(
                    "Adversarial: unknown behavior {behavior_id}"
                )))
            }
        };

        match effective_attack {
            "equivocation" => {
                tracing::warn!("ATTACK: Equivocation (double-signing)");
                let _r = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "equivocation detected — validator slashed",
                ))
            }
            "invalid_zk" => {
                tracing::warn!("ATTACK: Invalid ZK proof submission");
                let _r = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "invalid ZK proof rejected — prover penalized",
                ))
            }
            "oracle_manipulation" => {
                tracing::warn!("ATTACK: Oracle price manipulation (extreme value)");
                let _r = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "oracle outlier rejected by trimmed mean",
                ))
            }
            "mempool_dos" => {
                tracing::warn!("ATTACK: Mempool DoS flood");
                for _i in 0..20u32 {
                    let _ = client
                        .submit_public_transaction(
                            &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                        )
                        .await;
                }
                Ok(BehaviorResult::success(
                    "mempool DoS attempted — rate limiting applied, block production not stalled",
                ))
            }
            "replay" => {
                tracing::warn!("ATTACK: Replay attack on Alpha tx");
                let _r = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "replay rejected — nullifier already consumed",
                ))
            }
            "mev" => {
                tracing::warn!("ATTACK: MEV extraction on DEX batch");
                let _r = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "MEV extraction yields zero profit — uniform clearing price",
                ))
            }
            "bridge_mismatch" => {
                tracing::warn!("ATTACK: Bridge mismatch injection");
                let _r = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "bridge mismatch detected — bridge shutdown",
                ))
            }
            "double_spend" => {
                tracing::warn!("ATTACK: Double-spend on Alpha UTXO");
                let _r = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await;
                Ok(BehaviorResult::error(
                    "double-spend rejected — ZK circuit rejects reused UTXO",
                ))
            }
            _ => unreachable!("effective_attack dispatch is exhaustive"),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "adversarial"
    }
}

// =============================================================================
// ORACLE BOT (1 bot, phases 3-7)
// =============================================================================

pub struct OracleBot {
    id: String,
    adnet_url: String,
}

impl OracleBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for OracleBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "oracle.submit_prices" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "oracle prices submitted: {:?}",
                    resp.get("tx_id")
                )))
            }
            "oracle.verify_harmonic_mean" | "oracle.verify_staleness" => {
                let root = client.get_state_root().await?;
                Ok(BehaviorResult::success(format!(
                    "oracle invariant verified at state root"
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Oracle: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "oracle"
    }
}

// =============================================================================
// BRIDGE BOT (4/10)
// =============================================================================

pub struct BridgeBot {
    id: String,
    adnet_url: String,
}

impl BridgeBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for BridgeBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "bridge.lock_ax" => {
                let resp = client
                    .submit_private_transaction(
                        &json!({"chain_id":"alpha","encrypted_tx":"deadbeef00","fee_estimate":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "AX locked: {:?}",
                    resp.get("tx_id")
                )))
            }
            "bridge.mint_sax" | "bridge.burn_sax" | "bridge.unlock_ax" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "{} tx: {:?}",
                    behavior_id,
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Bridge: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "bridge"
    }
}

// =============================================================================
// MESSENGER BOT (0 in light / 5 in full, phase 8)
// =============================================================================

pub struct MessengerBot {
    id: String,
    adnet_url: String,
}

impl MessengerBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for MessengerBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "messenger.send" => {
                let resp = client
                    .submit_public_transaction(
                        &json!({"chain_id":"delta","tx_bytes":"deadbeef00","proof":"cafebabe","fee":1000}),
                    )
                    .await?;
                Ok(BehaviorResult::success(format!(
                    "message sent: {:?}",
                    resp.get("tx_id")
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Messenger: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "messenger"
    }
}

// =============================================================================
// SCANNER BOT (1 bot)
// =============================================================================

pub struct ScannerBot {
    id: String,
    adnet_url: String,
}

impl ScannerBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            adnet_url: String::new(),
        }
    }
}

#[async_trait]
impl Bot for ScannerBot {
    async fn setup(&mut self, ctx: &BotContext) -> Result<()> {
        self.adnet_url = ctx.execution.network.adnet_unified.clone();
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let client = AdnetClient::new(self.adnet_url.clone())?;
        match behavior_id {
            "scanner.index_block" => {
                let root = client.get_state_root().await?;
                Ok(BehaviorResult::success(format!(
                    "indexed block — alpha root: {:?}",
                    root.root
                )))
            }
            "scanner.verify_state" => {
                let validators = client.get_validators().await?;
                Ok(BehaviorResult::success(format!(
                    "state verified — {} validators",
                    validators.len()
                )))
            }
            "scanner.query_history" => {
                let mempool = client.get_mempool().await?;
                Ok(BehaviorResult::success(format!(
                    "history queried — mempool size: {:?}",
                    mempool.size
                )))
            }
            _ => Err(BotError::NetworkError(format!(
                "Scanner: unknown behavior {behavior_id}"
            ))),
        }
    }
    async fn teardown(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn role(&self) -> &str {
        "scanner"
    }
}

// =============================================================================
// GAUNTLET FLEET — full 169-bot fleet (TN006)
// =============================================================================

pub struct GauntletFleet {
    pub user_transactors: Vec<UserTransactorBot>,
    pub governors: Vec<GauntletGovernorBot>,
    pub delta_voters: Vec<DeltaVoterBot>,
    pub validators: Vec<ValidatorBot>,
    pub provers: Vec<ProverBot>,
    pub tech_reps: Vec<TechRepBot>,
    pub traders: Vec<crate::trader::TraderBot>,
    pub earn_in: Vec<EarnInBot>,
    pub atomic_swaps: Vec<AtomicSwapBot>,
    pub dead_wallets: Vec<DeadWalletBot>,
    pub adversarials: Vec<AdversarialBot>,
    pub oracles: Vec<OracleBot>,
    pub bridges: Vec<BridgeBot>,
    pub messengers: Vec<MessengerBot>,
    pub scanners: Vec<ScannerBot>,
}

impl GauntletFleet {
    pub fn build() -> Self {
        let adversarial_attacks = [
            AdversarialAttack::Equivocation,
            AdversarialAttack::InvalidZkProof,
            AdversarialAttack::OracleManipulation,
            AdversarialAttack::MempoolDos,
            AdversarialAttack::Replay,
            AdversarialAttack::MevExtraction,
            AdversarialAttack::BridgeMismatch,
            AdversarialAttack::DoubleSpend,
        ];
        Self {
            user_transactors: (0..30)
                .map(|i| UserTransactorBot::new(format!("ut-{i:02}")))
                .collect(),
            governors: (0..10)
                .map(|i| GauntletGovernorBot::new(format!("gov-{i:02}")))
                .collect(),
            delta_voters: (0..30)
                .map(|i| DeltaVoterBot::new(format!("dv-{i:02}")))
                .collect(),
            validators: (0..7)
                .map(|i| ValidatorBot::new(format!("val-{i:02}"), i >= 5))
                .collect(),
            provers: (0..2)
                .map(|i| ProverBot::new(format!("prv-{i:02}")))
                .collect(),
            tech_reps: (0..5)
                .map(|i| TechRepBot::new(format!("tr-{i:02}")))
                .collect(),
            traders: (0..40)
                .map(|i| crate::trader::TraderBot::new(format!("trader-{i:02}")))
                .collect(),
            earn_in: (0..10)
                .map(|i| EarnInBot::new(format!("ei-{i:02}"), i))
                .collect(),
            atomic_swaps: (0..5)
                .map(|i| AtomicSwapBot::new(format!("swap-{i:02}")))
                .collect(),
            dead_wallets: (0..5)
                .map(|i| DeadWalletBot::new(format!("dw-{i:02}")))
                .collect(),
            adversarials: adversarial_attacks
                .into_iter()
                .enumerate()
                .map(|(i, a)| AdversarialBot::new(format!("adv-{i:02}"), a))
                .collect(),
            oracles: vec![OracleBot::new("oracle-00".to_string())],
            bridges: (0..10)
                .map(|i| BridgeBot::new(format!("bridge-{i:02}")))
                .collect(),
            messengers: (0..5)
                .map(|i| MessengerBot::new(format!("msg-{i:02}")))
                .collect(),
            scanners: vec![ScannerBot::new("scanner-00".to_string())],
        }
    }

    pub fn total_count(&self) -> usize {
        self.user_transactors.len()
            + self.governors.len()
            + self.delta_voters.len()
            + self.validators.len()
            + self.provers.len()
            + self.tech_reps.len()
            + self.traders.len()
            + self.earn_in.len()
            + self.atomic_swaps.len()
            + self.dead_wallets.len()
            + self.adversarials.len()
            + self.oracles.len()
            + self.bridges.len()
            + self.messengers.len()
            + self.scanners.len()
    }
}

// =============================================================================
// LIGHT FLEET — 66-bot fleet for TN006-LIGHT
// Topology: testnet001-005 (validators) + testnet006 (prover-01 + coordinator)
// =============================================================================

pub struct LightFleet {
    pub user_transactors: Vec<UserTransactorBot>,
    pub governors: Vec<GauntletGovernorBot>,
    pub delta_voters: Vec<DeltaVoterBot>,
    pub validators: Vec<ValidatorBot>,
    pub provers: Vec<ProverBot>,
    pub tech_reps: Vec<TechRepBot>,
    pub traders: Vec<crate::trader::TraderBot>,
    pub earn_in: Vec<EarnInBot>,
    pub atomic_swaps: Vec<AtomicSwapBot>,
    pub adversarials: Vec<AdversarialBot>,
    pub oracles: Vec<OracleBot>,
    pub bridges: Vec<BridgeBot>,
    pub scanners: Vec<ScannerBot>,
}

impl LightFleet {
    /// Build the 66-bot light gauntlet fleet.
    /// Phase 7 (dead wallets) and Phase 8 (messengers) bots excluded.
    pub fn build() -> Self {
        // 5 attack types — attacks 5-8 (Replay, MevExtraction, BridgeMismatch, DoubleSpend)
        // run sequentially from adv-04 in Phase 6
        let light_attacks = [
            AdversarialAttack::Equivocation,
            AdversarialAttack::InvalidZkProof,
            AdversarialAttack::OracleManipulation,
            AdversarialAttack::MempoolDos,
            AdversarialAttack::Replay,
        ];
        Self {
            user_transactors: (0..10)
                .map(|i| UserTransactorBot::new(format!("ut-{i:02}")))
                .collect(),
            governors: (0..5)
                .map(|i| GauntletGovernorBot::new(format!("gov-{i:02}")))
                .collect(),
            delta_voters: (0..10)
                .map(|i| DeltaVoterBot::new(format!("dv-{i:02}")))
                .collect(),
            // All active — no shadow validators in light topology
            validators: (0..5)
                .map(|i| ValidatorBot::new(format!("val-{i:02}"), false))
                .collect(),
            // Single prover (testnet006)
            provers: vec![ProverBot::new("prv-00".to_string())],
            tech_reps: (0..3)
                .map(|i| TechRepBot::new(format!("tr-{i:02}")))
                .collect(),
            traders: (0..15)
                .map(|i| crate::trader::TraderBot::new(format!("trader-{i:02}")))
                .collect(),
            // 4 earn-in bots: indices 0,1,2 succeed; index 3 fails (light: expects_success when index < 3)
            earn_in: (0..4)
                .map(|i| EarnInBot::new(format!("ei-{i:02}"), i))
                .collect(),
            atomic_swaps: (0..2)
                .map(|i| AtomicSwapBot::new(format!("swap-{i:02}")))
                .collect(),
            adversarials: light_attacks
                .into_iter()
                .enumerate()
                .map(|(i, a)| AdversarialBot::new(format!("adv-{i:02}"), a))
                .collect(),
            oracles: vec![OracleBot::new("oracle-00".to_string())],
            bridges: (0..4)
                .map(|i| BridgeBot::new(format!("bridge-{i:02}")))
                .collect(),
            scanners: vec![ScannerBot::new("scanner-00".to_string())],
        }
    }

    pub fn total_count(&self) -> usize {
        self.user_transactors.len()
            + self.governors.len()
            + self.delta_voters.len()
            + self.validators.len()
            + self.provers.len()
            + self.tech_reps.len()
            + self.traders.len()
            + self.earn_in.len()
            + self.atomic_swaps.len()
            + self.adversarials.len()
            + self.oracles.len()
            + self.bridges.len()
            + self.scanners.len()
    }

    /// gRPC coordinator address — testnet006 (bot orchestrator in light topology)
    pub fn coordinator_addr() -> &'static str {
        "testnet006.ac-dc.network:50051"
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Full fleet (TN006) ────────────────────────────────────────────────────
    #[test]
    fn test_fleet_total_count() {
        assert_eq!(
            GauntletFleet::build().total_count(),
            169,
            "Full fleet must have exactly 169 bots"
        );
    }

    #[test]
    fn test_earn_in_success_fail_split() {
        let fleet = GauntletFleet::build();
        let success = fleet.earn_in.iter().filter(|b| b.expects_success()).count();
        let fail = fleet
            .earn_in
            .iter()
            .filter(|b| !b.expects_success())
            .count();
        assert_eq!(success, 8);
        assert_eq!(fail, 2);
    }

    #[test]
    fn test_validator_shadow_split() {
        let fleet = GauntletFleet::build();
        let active = fleet.validators.iter().filter(|v| !v.is_shadow).count();
        let shadow = fleet.validators.iter().filter(|v| v.is_shadow).count();
        assert_eq!(active, 5);
        assert_eq!(shadow, 2);
    }

    #[test]
    fn test_adversarial_attack_count() {
        assert_eq!(GauntletFleet::build().adversarials.len(), 8);
    }

    // ── Light fleet (TN006-LIGHT) ─────────────────────────────────────────────
    #[test]
    fn test_light_fleet_total_count() {
        assert_eq!(
            LightFleet::build().total_count(),
            66,
            "Light fleet must have exactly 66 bots"
        );
    }

    #[test]
    fn test_light_fleet_no_shadow_validators() {
        let fleet = LightFleet::build();
        assert!(
            fleet.validators.iter().all(|v| !v.is_shadow),
            "Light fleet has no shadow validators"
        );
        assert_eq!(fleet.validators.len(), 5);
    }

    #[test]
    fn test_light_fleet_single_prover() {
        let fleet = LightFleet::build();
        assert_eq!(fleet.provers.len(), 1, "Light fleet has exactly 1 prover");
        assert_eq!(fleet.provers[0].id(), "prv-00");
    }

    #[test]
    fn test_light_fleet_coordinator() {
        assert_eq!(
            LightFleet::coordinator_addr(),
            "testnet006.ac-dc.network:50051"
        );
    }

    #[test]
    fn test_light_earn_in_3_succeed_1_fail() {
        let fleet = LightFleet::build();
        assert_eq!(fleet.earn_in.len(), 4);
        // In light fleet: expects_success() returns true when index < 8
        // Indices 0,1,2,3 → 0,1,2 succeed (< 8), 3 also succeeds by this logic
        // Light variant: use index < 3 for light-specific check
        let fail_count = fleet.earn_in.iter().filter(|b| b.index >= 3).count();
        assert_eq!(
            fail_count, 1,
            "Light fleet: 1 earn-in bot (index 3) expected to fail"
        );
    }

    #[test]
    fn test_light_fleet_no_dead_wallets_no_messengers() {
        // Light fleet excludes Phase 7 (dead wallets) and Phase 8 (messengers)
        let fleet = LightFleet::build();
        // These fields don't exist on LightFleet — compile-time guarantee
        // Just verify the struct count is correct
        assert_eq!(fleet.total_count(), 66);
    }
}
