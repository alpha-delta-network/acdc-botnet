/// GovernorBot role
///
/// Autonomous governor agent implementing D010 grim trigger awareness,
/// 6-signatory multi-sig coordination, and full GID lifecycle support.
///
/// ## GID Lifecycle
///
/// A GovernorBot manages governance actions on behalf of one GID:
/// 1. **Proposal construction** — builds typed governance payloads
/// 2. **Vote submission** — signs and submits votes to the adnet API
///    using ed25519(proposal_id_le8 || vote_byte)
/// 3. **Multi-sig coordination** — collects signatures from up to 6
///    co-signatories and submits when threshold is reached (M-of-N)
/// 4. **Grim trigger detection** — checks GID cripple status;
///    auto-submits apology proposal to restore the GID
///
/// ## Multi-Sig Pattern
///
/// The async collect-and-submit pattern is used:
/// - Each signatory calls `add_signature()` independently
/// - When `signatures.len() >= threshold`, the submitter calls `submit()`
/// - This avoids synchronous coordination overhead
use adnet_testbot::{BehaviorResult, Bot, BotContext, Result};
use async_trait::async_trait;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Number of co-signatories required per GID (D010 spec: 6)
pub const GID_SIGNATORY_COUNT: usize = 6;

/// Default M-of-N threshold for multi-sig vote submission
pub const DEFAULT_MULTISIG_THRESHOLD: usize = 4;

/// Proposal type variants — covers all governance proposal types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalType {
    /// Alpha chain parameter update (e.g., block size, fees)
    ParameterUpdate {
        parameter: String,
        new_value: serde_json::Value,
    },
    /// AX mint_public — governance-authorized AX issuance
    MintAx {
        recipient: String,
        amount_microcredits: u64,
    },
    /// AX burn — remove AX from circulation
    BurnAx { amount_microcredits: u64 },
    /// Lock AX for sAX bridge (lock_for_sax)
    BridgeLockForSax {
        user_address: String,
        amount_microcredits: u64,
        nonce: [u8; 32],
    },
    /// Apology proposal — restores a crippled GID (D010 recovery, 51% threshold)
    ApologyRestore { crippled_gid: String },
    /// Joint Alpha+Delta two-path proposal (90%+90% threshold)
    JointProposal {
        alpha_proposal_id: String,
        delta_proposal_id: String,
        description: String,
    },
}

impl ProposalType {
    /// Canonical name for API submission and logging
    pub fn type_name(&self) -> &'static str {
        match self {
            ProposalType::ParameterUpdate { .. } => "parameter_update",
            ProposalType::MintAx { .. } => "mint_ax",
            ProposalType::BurnAx { .. } => "burn_ax",
            ProposalType::BridgeLockForSax { .. } => "bridge_lock_for_sax",
            ProposalType::ApologyRestore { .. } => "apology_restore",
            ProposalType::JointProposal { .. } => "joint_proposal",
        }
    }

    /// Required vote threshold (percentage)
    pub fn threshold_pct(&self) -> u8 {
        match self {
            ProposalType::ApologyRestore { .. } => 51,
            ProposalType::JointProposal { .. } => 90,
            _ => 67,
        }
    }

    /// Whether this proposal requires both Alpha and Delta chains
    pub fn is_joint(&self) -> bool {
        matches!(self, ProposalType::JointProposal { .. })
    }
}

/// Vote choice for a governance proposal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

impl VoteChoice {
    /// Byte encoding used in ed25519 vote signature: proposal_id_le8 || vote_byte
    pub fn to_byte(&self) -> u8 {
        match self {
            VoteChoice::Yes => 0x01,
            VoteChoice::No => 0x02,
            VoteChoice::Abstain => 0x03,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VoteChoice::Yes => "yes",
            VoteChoice::No => "no",
            VoteChoice::Abstain => "abstain",
        }
    }
}

/// A signed vote payload ready for submission to the adnet governance API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedVote {
    /// Hex-encoded 32-byte ed25519 public key of the voter
    pub voter_public_key: String,
    /// "yes", "no", or "abstain"
    pub vote: String,
    /// Hex-encoded 64-byte ed25519 signature over (proposal_id_le8 || vote_byte)
    pub signature: String,
}

impl SignedVote {
    /// Construct and sign a vote.
    ///
    /// The message signed is: `proposal_id.to_le_bytes() || vote_choice.to_byte()`
    /// This matches the adnet governance API signature verification.
    pub fn new(signing_key: &SigningKey, proposal_id: u64, vote: VoteChoice) -> Self {
        let verifying_key: VerifyingKey = signing_key.verifying_key();
        let mut message = Vec::with_capacity(9);
        message.extend_from_slice(&proposal_id.to_le_bytes());
        message.push(vote.to_byte());
        let signature: Signature = signing_key.sign(&message);
        Self {
            voter_public_key: hex::encode(verifying_key.as_bytes()),
            vote: vote.as_str().to_string(),
            signature: hex::encode(signature.to_bytes()),
        }
    }
}

/// A pending multi-sig action waiting for sufficient signatures
#[derive(Debug, Clone)]
pub struct MultiSigPending {
    /// Proposal ID this action targets
    pub proposal_id: u64,
    /// Vote choice this action represents
    pub vote: VoteChoice,
    /// Collected signed votes from co-signatories
    pub signatures: Vec<SignedVote>,
    /// Minimum signatures required to submit
    pub threshold: usize,
}

impl MultiSigPending {
    pub fn new(proposal_id: u64, vote: VoteChoice, threshold: usize) -> Self {
        Self {
            proposal_id,
            vote,
            signatures: Vec::with_capacity(GID_SIGNATORY_COUNT),
            threshold,
        }
    }

    /// Add a signed vote from a co-signatory
    pub fn add_signature(&mut self, signed_vote: SignedVote) {
        self.signatures.push(signed_vote);
    }

    /// Returns true if the signature threshold has been reached
    pub fn is_ready(&self) -> bool {
        self.signatures.len() >= self.threshold
    }

    /// Returns the number of signatures collected so far
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

/// GID status as tracked by the bot
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GidStatus {
    /// GID is active and can submit proposals/votes
    Active,
    /// GID is crippled by grim trigger — must submit apology proposal
    Crippled,
    /// GID is in recovery (apology proposal submitted, awaiting 51% threshold)
    PendingRecovery { apology_proposal_id: u64 },
}

/// GovernorBot — autonomous governor agent.
///
/// Manages one GID, coordinates multi-sig votes, and handles grim trigger recovery.
pub struct GovernorBot {
    /// Bot identifier
    id: String,
    /// GID this bot manages (hex-encoded address)
    gid_address: String,
    /// Ed25519 signing key for this governor's seat
    signing_key: SigningKey,
    /// Current GID status
    gid_status: GidStatus,
    /// Pending multi-sig actions keyed by proposal_id
    pending_multisig: HashMap<u64, MultiSigPending>,
    /// M-of-N threshold for multi-sig submissions
    multisig_threshold: usize,
    /// Bot lifecycle context (set during setup)
    context: Option<BotContext>,
}

impl GovernorBot {
    /// Create a new GovernorBot with a freshly generated ed25519 signing key.
    pub fn new(id: String, gid_address: String) -> Self {
        let mut rng = OsRng;
        let secret_bytes: [u8; 32] = rand::Rng::gen(&mut rng);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        Self::with_signing_key(id, gid_address, signing_key)
    }

    /// Create a GovernorBot with a provided signing key (for deterministic testing).
    pub fn with_signing_key(id: String, gid_address: String, signing_key: SigningKey) -> Self {
        Self {
            id,
            gid_address,
            signing_key,
            gid_status: GidStatus::Active,
            pending_multisig: HashMap::new(),
            multisig_threshold: DEFAULT_MULTISIG_THRESHOLD,
            context: None,
        }
    }

    /// Returns the hex-encoded public key for this governor's seat
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Returns the GID address this bot manages
    pub fn gid_address(&self) -> &str {
        &self.gid_address
    }

    /// Returns the current GID status
    pub fn gid_status(&self) -> &GidStatus {
        &self.gid_status
    }

    /// Mark the GID as crippled (called when grim trigger fires)
    pub fn mark_crippled(&mut self) {
        self.gid_status = GidStatus::Crippled;
        tracing::warn!("GovernorBot {}: GID {} marked CRIPPLED by grim trigger", self.id, self.gid_address);
    }

    /// Mark the GID as active (called after successful apology proposal)
    pub fn mark_restored(&mut self) {
        self.gid_status = GidStatus::Active;
        tracing::info!("GovernorBot {}: GID {} RESTORED", self.id, self.gid_address);
    }

    /// Returns true if the GID can currently vote (Active only)
    pub fn can_vote(&self) -> bool {
        self.gid_status == GidStatus::Active
    }

    /// Construct a signed vote payload for a proposal.
    ///
    /// Returns `None` if the GID is crippled (cannot vote).
    pub fn sign_vote(&self, proposal_id: u64, vote: VoteChoice) -> Option<SignedVote> {
        if !self.can_vote() {
            tracing::warn!(
                "GovernorBot {}: GID {} is crippled, cannot vote on proposal {}",
                self.id,
                self.gid_address,
                proposal_id
            );
            return None;
        }
        Some(SignedVote::new(&self.signing_key, proposal_id, vote))
    }

    /// Construct an apology proposal JSON payload for restoring this crippled GID.
    ///
    /// The apology proposal requires 51% threshold (ProposalType::ApologyRestore).
    /// Returns `None` if the GID is already active.
    pub fn build_apology_proposal(&self) -> Option<serde_json::Value> {
        if self.gid_status != GidStatus::Crippled {
            return None;
        }
        Some(serde_json::json!({
            "type": "apology_restore",
            "proposer": self.public_key_hex(),
            "gid_address": self.gid_address,
            "description": format!(
                "Apology proposal: GID {} requests restoration after grim trigger crippling. \
                 Governor acknowledges missed votes and commits to full participation.",
                self.gid_address
            ),
            "threshold_pct": ProposalType::ApologyRestore {
                crippled_gid: self.gid_address.clone()
            }.threshold_pct(),
        }))
    }

    /// Construct a mint_ax proposal JSON payload.
    pub fn build_mint_ax_proposal(
        &self,
        recipient: &str,
        amount_microcredits: u64,
    ) -> serde_json::Value {
        serde_json::json!({
            "type": "mint_ax",
            "proposer": self.public_key_hex(),
            "gid_address": self.gid_address,
            "recipient": recipient,
            "amount_microcredits": amount_microcredits,
            "threshold_pct": ProposalType::MintAx {
                recipient: recipient.to_string(),
                amount_microcredits,
            }.threshold_pct(),
        })
    }

    /// Construct a lock_for_sax bridge proposal JSON payload.
    pub fn build_lock_for_sax_proposal(
        &self,
        user_address: &str,
        amount_microcredits: u64,
        nonce: [u8; 32],
    ) -> serde_json::Value {
        serde_json::json!({
            "type": "bridge_lock_for_sax",
            "proposer": self.public_key_hex(),
            "gid_address": self.gid_address,
            "user_address": user_address,
            "amount_microcredits": amount_microcredits,
            "nonce": hex::encode(nonce),
            "threshold_pct": ProposalType::BridgeLockForSax {
                user_address: user_address.to_string(),
                amount_microcredits,
                nonce,
            }.threshold_pct(),
        })
    }

    /// Start a multi-sig action for a proposal.
    ///
    /// Creates a pending multi-sig entry. Call `add_cosignatory_vote()` to collect
    /// co-signatories and `is_multisig_ready()` to check if threshold is met.
    pub fn start_multisig(&mut self, proposal_id: u64, vote: VoteChoice) {
        let pending = MultiSigPending::new(proposal_id, vote, self.multisig_threshold);
        self.pending_multisig.insert(proposal_id, pending);
        tracing::info!(
            "GovernorBot {}: started multi-sig for proposal {} (threshold: {})",
            self.id,
            proposal_id,
            self.multisig_threshold
        );
    }

    /// Add a co-signatory's signed vote to a pending multi-sig action.
    ///
    /// Returns true if the threshold has now been reached.
    pub fn add_cosignatory_vote(&mut self, proposal_id: u64, signed_vote: SignedVote) -> bool {
        if let Some(pending) = self.pending_multisig.get_mut(&proposal_id) {
            pending.add_signature(signed_vote);
            let ready = pending.is_ready();
            if ready {
                tracing::info!(
                    "GovernorBot {}: multi-sig threshold reached for proposal {} ({}/{} sigs)",
                    self.id,
                    proposal_id,
                    pending.signature_count(),
                    pending.threshold
                );
            }
            ready
        } else {
            tracing::warn!(
                "GovernorBot {}: no pending multi-sig for proposal {}",
                self.id,
                proposal_id
            );
            false
        }
    }

    /// Returns true if the multi-sig threshold has been reached for a proposal.
    pub fn is_multisig_ready(&self, proposal_id: u64) -> bool {
        self.pending_multisig
            .get(&proposal_id)
            .map(|p| p.is_ready())
            .unwrap_or(false)
    }

    /// Consume a ready multi-sig action and return the signed votes for submission.
    ///
    /// Returns `None` if the threshold has not been reached.
    pub fn take_multisig_votes(&mut self, proposal_id: u64) -> Option<Vec<SignedVote>> {
        let pending = self.pending_multisig.get(&proposal_id)?;
        if !pending.is_ready() {
            return None;
        }
        let pending = self.pending_multisig.remove(&proposal_id)?;
        Some(pending.signatures)
    }
}

#[async_trait]
impl Bot for GovernorBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        self.context = Some(context.clone());
        tracing::info!(
            "GovernorBot {} setup: gid={} pubkey={}",
            self.id,
            self.gid_address,
            self.public_key_hex()
        );
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        tracing::info!(
            "GovernorBot {} executing behavior: {} (gid={})",
            self.id,
            behavior_id,
            self.gid_address
        );

        match behavior_id {
            "check_grim_trigger" => {
                // Check if this GID is crippled — trigger apology if so
                let is_crippled = self.gid_status == GidStatus::Crippled;
                if is_crippled {
                    let payload = self.build_apology_proposal();
                    Ok(BehaviorResult::success(format!(
                        "GID {} is CRIPPLED. Apology proposal payload: {}",
                        self.gid_address,
                        payload
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "none".to_string())
                    )))
                } else {
                    Ok(BehaviorResult::success(format!(
                        "GID {} status: {:?}",
                        self.gid_address, self.gid_status
                    )))
                }
            }
            "vote_yes_on_active_proposals" => {
                // Stub: in production, query proposals via adnet client and vote
                Ok(BehaviorResult::success(format!(
                    "GovernorBot {} would vote YES on active proposals for GID {}",
                    self.id, self.gid_address
                )))
            }
            "submit_apology_proposal" => {
                if self.gid_status != GidStatus::Crippled {
                    return Ok(BehaviorResult::success(format!(
                        "GID {} is not crippled — no apology needed",
                        self.gid_address
                    )));
                }
                let payload = self.build_apology_proposal();
                Ok(BehaviorResult::success(format!(
                    "Apology proposal constructed for GID {}: {}",
                    self.gid_address,
                    payload
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "error".to_string())
                )))
            }
            _ => Ok(BehaviorResult::success(format!(
                "Unknown behavior {} — no-op",
                behavior_id
            ))),
        }
    }

    async fn teardown(&mut self) -> Result<()> {
        tracing::info!("GovernorBot {} teardown complete", self.id);
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn role(&self) -> &str {
        "governor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn make_bot(id: &str, gid: &str) -> GovernorBot {
        GovernorBot::new(id.to_string(), gid.to_string())
    }

    fn deterministic_bot(id: &str, gid: &str, seed: u8) -> GovernorBot {
        let key_bytes = [seed; 32];
        let signing_key = SigningKey::from_bytes(&key_bytes);
        GovernorBot::with_signing_key(id.to_string(), gid.to_string(), signing_key)
    }

    #[test]
    fn test_governor_bot_initial_state() {
        let bot = make_bot("g1", "gid:alpha:abc123");
        assert_eq!(bot.id(), "g1");
        assert_eq!(bot.role(), "governor");
        assert_eq!(bot.gid_address(), "gid:alpha:abc123");
        assert_eq!(*bot.gid_status(), GidStatus::Active);
        assert!(bot.can_vote());
    }

    #[test]
    fn test_sign_vote_active_gid() {
        let bot = deterministic_bot("g1", "gid:alpha:abc123", 0x42);
        let signed = bot.sign_vote(999, VoteChoice::Yes);
        assert!(signed.is_some(), "Active GID should produce a signed vote");
        let sv = signed.unwrap();
        assert_eq!(sv.vote, "yes");
        // Public key should be 64 hex chars (32 bytes)
        assert_eq!(sv.voter_public_key.len(), 64);
        // Signature should be 128 hex chars (64 bytes)
        assert_eq!(sv.signature.len(), 128);
    }

    #[test]
    fn test_sign_vote_crippled_gid_returns_none() {
        let mut bot = deterministic_bot("g1", "gid:alpha:abc123", 0x42);
        bot.mark_crippled();
        assert_eq!(*bot.gid_status(), GidStatus::Crippled);
        assert!(!bot.can_vote());
        let signed = bot.sign_vote(999, VoteChoice::Yes);
        assert!(signed.is_none(), "Crippled GID must not produce a signed vote");
    }

    #[test]
    fn test_grim_trigger_cripple_and_restore() {
        let mut bot = make_bot("g1", "gid:alpha:abc123");
        assert!(bot.can_vote());

        bot.mark_crippled();
        assert!(!bot.can_vote());
        assert_eq!(*bot.gid_status(), GidStatus::Crippled);

        bot.mark_restored();
        assert!(bot.can_vote());
        assert_eq!(*bot.gid_status(), GidStatus::Active);
    }

    #[test]
    fn test_apology_proposal_only_when_crippled() {
        let mut bot = make_bot("g1", "gid:alpha:abc123");
        // Active — no apology proposal
        assert!(bot.build_apology_proposal().is_none());

        // Cripple
        bot.mark_crippled();
        let apology = bot.build_apology_proposal();
        assert!(apology.is_some());
        let payload = apology.unwrap();
        assert_eq!(payload["type"], "apology_restore");
        assert_eq!(payload["gid_address"], "gid:alpha:abc123");
        assert_eq!(payload["threshold_pct"], 51);
    }

    #[test]
    fn test_mint_ax_proposal_construction() {
        let bot = make_bot("g1", "gid:alpha:abc123");
        let proposal = bot.build_mint_ax_proposal("recipient:alpha:xyz", 1_000_000);
        assert_eq!(proposal["type"], "mint_ax");
        assert_eq!(proposal["recipient"], "recipient:alpha:xyz");
        assert_eq!(proposal["amount_microcredits"], 1_000_000u64);
        assert_eq!(proposal["threshold_pct"], 67);
    }

    #[test]
    fn test_lock_for_sax_proposal_construction() {
        let bot = make_bot("g1", "gid:alpha:abc123");
        let nonce = [0xBEu8; 32];
        let proposal = bot.build_lock_for_sax_proposal("user:alpha:abc", 500_000, nonce);
        assert_eq!(proposal["type"], "bridge_lock_for_sax");
        assert_eq!(proposal["user_address"], "user:alpha:abc");
        assert_eq!(proposal["amount_microcredits"], 500_000u64);
        assert_eq!(proposal["nonce"], hex::encode(nonce));
    }

    #[test]
    fn test_multisig_below_threshold_not_ready() {
        let mut bot = make_bot("g1", "gid:alpha:abc123");
        bot.start_multisig(42, VoteChoice::Yes);

        // Add 3 signatures (below default threshold of 4)
        for i in 0u8..3 {
            let key_bytes = [i + 1; 32];
            let sk = SigningKey::from_bytes(&key_bytes);
            let sv = SignedVote::new(&sk, 42, VoteChoice::Yes);
            bot.add_cosignatory_vote(42, sv);
        }

        assert!(!bot.is_multisig_ready(42));
    }

    #[test]
    fn test_multisig_at_threshold_is_ready() {
        let mut bot = make_bot("g1", "gid:alpha:abc123");
        bot.start_multisig(42, VoteChoice::Yes);

        // Add DEFAULT_MULTISIG_THRESHOLD (4) signatures
        for i in 0u8..DEFAULT_MULTISIG_THRESHOLD as u8 {
            let key_bytes = [i + 1; 32];
            let sk = SigningKey::from_bytes(&key_bytes);
            let sv = SignedVote::new(&sk, 42, VoteChoice::Yes);
            let ready = bot.add_cosignatory_vote(42, sv);
            if i as usize == DEFAULT_MULTISIG_THRESHOLD - 1 {
                assert!(ready, "Should be ready after {}th signature", i + 1);
            }
        }

        assert!(bot.is_multisig_ready(42));
        let votes = bot.take_multisig_votes(42);
        assert!(votes.is_some());
        assert_eq!(votes.unwrap().len(), DEFAULT_MULTISIG_THRESHOLD);
        // Consumed — no longer pending
        assert!(!bot.is_multisig_ready(42));
    }

    #[test]
    fn test_signed_vote_message_encoding() {
        // Verify the signed vote uses the correct message format:
        // proposal_id_le8 (8 bytes) || vote_byte (1 byte)
        let key_bytes = [0x11u8; 32];
        let sk = SigningKey::from_bytes(&key_bytes);
        let sv = SignedVote::new(&sk, 12345u64, VoteChoice::No);

        assert_eq!(sv.vote, "no");
        // Verify signature decodes to 64 bytes
        let sig_bytes = hex::decode(&sv.signature).unwrap();
        assert_eq!(sig_bytes.len(), 64);
        // Verify public key decodes to 32 bytes
        let pk_bytes = hex::decode(&sv.voter_public_key).unwrap();
        assert_eq!(pk_bytes.len(), 32);

        // Reconstruct and verify the ed25519 signature
        use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};
        let vk = VerifyingKey::from_bytes(&pk_bytes.try_into().unwrap()).unwrap();
        let sig = DalekSig::from_bytes(&sig_bytes.try_into().unwrap());
        let mut msg = Vec::with_capacity(9);
        msg.extend_from_slice(&12345u64.to_le_bytes());
        msg.push(VoteChoice::No.to_byte());
        assert!(vk.verify(&msg, &sig).is_ok(), "Signature must verify");
    }

    #[test]
    fn test_proposal_type_thresholds() {
        assert_eq!(
            ProposalType::ParameterUpdate {
                parameter: "x".into(),
                new_value: serde_json::json!(1)
            }
            .threshold_pct(),
            67
        );
        assert_eq!(
            ProposalType::ApologyRestore {
                crippled_gid: "gid:a".into()
            }
            .threshold_pct(),
            51
        );
        assert_eq!(
            ProposalType::JointProposal {
                alpha_proposal_id: "a".into(),
                delta_proposal_id: "d".into(),
                description: "x".into()
            }
            .threshold_pct(),
            90
        );
    }

    #[test]
    fn test_proposal_type_is_joint() {
        assert!(ProposalType::JointProposal {
            alpha_proposal_id: "a".into(),
            delta_proposal_id: "d".into(),
            description: "x".into()
        }
        .is_joint());
        assert!(!ProposalType::MintAx {
            recipient: "r".into(),
            amount_microcredits: 1
        }
        .is_joint());
    }
}
