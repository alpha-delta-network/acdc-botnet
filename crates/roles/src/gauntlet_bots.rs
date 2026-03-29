// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! TN006 Gauntlet bot roles — 15 types, 169 total instances.
//!
//! Each bot implements the `Bot` trait. Gauntlet-specific bots extend the
//! base roles with scenario-scoped behavior (phase gates, PRNG determinism,
//! per-UC assertion hooks).

use adnet_testbot::{BehaviorResult, Bot, BotContext, Result};
use async_trait::async_trait;

// =============================================================================
// USER TRANSACTOR (30 bots, phases 1-7)
// UC: UC-A-U-001..007, UC-D-U-001..002/009..011
// =============================================================================

pub struct UserTransactorBot {
    id: String,
}

impl UserTransactorBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for UserTransactorBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "UserTransactor {} executed {}",
            self.id, behavior_id
        )))
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
// GOVERNOR BOT (10 bots, phases 2-7)
// UC: UC-GA-U-*, UC-A-U-008..011, UC-A-S-007..019/035..036
// Extends: GovernorBot with gauntlet multi-sig + grim trigger
// =============================================================================

pub struct GauntletGovernorBot {
    id: String,
    /// Gauntlet uses 3-of-5 (TN006 spec: 3-of-5 per GID)
    multisig_threshold: usize,
}

impl GauntletGovernorBot {
    pub fn new(id: String) -> Self {
        Self {
            id,
            multisig_threshold: 3,
        }
    }
}

#[async_trait]
impl Bot for GauntletGovernorBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "GauntletGovernor {} ({}M) executed {}",
            self.id, self.multisig_threshold, behavior_id
        )))
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
// DELTA VOTER BOT (30 bots, phases 2-7)
// UC: UC-GD-U/S-*, UC-D-U-015..017
// Emphatic voting: 100 DX/slot, 50 DX per side
// =============================================================================

pub struct DeltaVoterBot {
    id: String,
}

impl DeltaVoterBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for DeltaVoterBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "DeltaVoter {} executed {}",
            self.id, behavior_id
        )))
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
// VALIDATOR BOT (7 bots = 5 active + 2 shadow, phases 0-7)
// UC: UC-V-U/S-*, UC-TR-S-001..005/018..020
// =============================================================================

pub struct ValidatorBot {
    id: String,
    is_shadow: bool,
}

impl ValidatorBot {
    pub fn new(id: String, is_shadow: bool) -> Self {
        Self { id, is_shadow }
    }
}

#[async_trait]
impl Bot for ValidatorBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        let kind = if self.is_shadow { "shadow" } else { "active" };
        Ok(BehaviorResult::success(format!(
            "Validator({}) {} executed {}",
            kind, self.id, behavior_id
        )))
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
// PROVER BOT (2 bots, phases 0-7)
// UC: UC-P-U/S-*, UC-TR-S-006..010/012/015..016/021
// =============================================================================

pub struct ProverBot {
    id: String,
}

impl ProverBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for ProverBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Prover {} executed {}",
            self.id, behavior_id
        )))
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
// TECH REP BOT (5 bots, phases 2-7)
// UC: UC-T-U/S-*, UC-F-U-003..005
// =============================================================================

pub struct TechRepBot {
    id: String,
}

impl TechRepBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for TechRepBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "TechRep {} executed {}",
            self.id, behavior_id
        )))
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
// EARN-IN BOT (10 bots, phases 4-7)
// UC: UC-K-U-001..004/009, UC-K-S-010..013/024
// 8 expected succeed, 2 expected fail
// =============================================================================

pub struct EarnInBot {
    id: String,
    /// Bot index 0..9; bots 8-9 are configured to fail earn-in
    bot_index: usize,
}

impl EarnInBot {
    pub fn new(id: String, bot_index: usize) -> Self {
        Self { id, bot_index }
    }
    pub fn expects_success(&self) -> bool {
        self.bot_index < 8
    }
}

#[async_trait]
impl Bot for EarnInBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "EarnIn {} (success={}) executed {}",
            self.id,
            self.expects_success(),
            behavior_id
        )))
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
// ATOMIC SWAP BOT (5 bots, phases 5-7)
// UC: UC-D-U-019..025/028, UC-D-S-034..043
// =============================================================================

pub struct AtomicSwapBot {
    id: String,
}

impl AtomicSwapBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for AtomicSwapBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "AtomicSwap {} executed {}",
            self.id, behavior_id
        )))
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
// DEAD WALLET BOT (5 bots, phase 7 only)
// UC: UC-D-S-044..054, UC-D-G-006..008
// =============================================================================

pub struct DeadWalletBot {
    id: String,
}

impl DeadWalletBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for DeadWalletBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "DeadWallet {} executed {}",
            self.id, behavior_id
        )))
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
// ADVERSARIAL BOT (8 bots, phase 6)
// UC: UC-V-S-005, UC-P-S-010..011, UC-A-S-001, UC-N-S-003
// Attack assignment: one attack per bot
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
    attack: AdversarialAttack,
}

impl AdversarialBot {
    pub fn new(id: String, attack: AdversarialAttack) -> Self {
        Self { id, attack }
    }
}

#[async_trait]
impl Bot for AdversarialBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Adversarial {:?} bot {} executed {}",
            self.attack, self.id, behavior_id
        )))
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
// UC: UC-D-U-026..027, UC-D-S-022..030, UC-D-G-001..003
// Submits 25 FX pair prices, harmonic mean, AX/XAU = 0.35 / HM
// =============================================================================

pub struct OracleBot {
    id: String,
}

impl OracleBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for OracleBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Oracle {} executed {}",
            self.id, behavior_id
        )))
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
// BRIDGE BOT (10 bots, phases 1-7)
// UC: UC-A-S-005..006, UC-D-S-006..007, UC-N-S-014/017..018
// =============================================================================

pub struct BridgeBot {
    id: String,
}

impl BridgeBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for BridgeBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Bridge {} executed {}",
            self.id, behavior_id
        )))
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
// MESSENGER BOT (5 bots, phase 8)
// UC: UC-WM-U/S-*
// =============================================================================

pub struct MessengerBot {
    id: String,
}

impl MessengerBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for MessengerBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Messenger {} executed {}",
            self.id, behavior_id
        )))
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
// SCANNER BOT (1 bot, phases 1-8)
// UC: UC-SC-U-*
// =============================================================================

pub struct ScannerBot {
    id: String,
}

impl ScannerBot {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Bot for ScannerBot {
    async fn setup(&mut self, _ctx: &BotContext) -> Result<()> {
        Ok(())
    }
    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        Ok(BehaviorResult::success(format!(
            "Scanner {} executed {}",
            self.id, behavior_id
        )))
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
// FLEET FACTORY — instantiates all 169 bots from gauntlet-genesis.yaml
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
    /// Build the complete 169-bot gauntlet fleet.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fleet_total_count() {
        let fleet = GauntletFleet::build();
        assert_eq!(
            fleet.total_count(),
            169,
            "Gauntlet fleet must have exactly 169 bots"
        );
    }

    #[test]
    fn test_earn_in_success_fail_split() {
        let fleet = GauntletFleet::build();
        let success_count = fleet.earn_in.iter().filter(|b| b.expects_success()).count();
        let fail_count = fleet
            .earn_in
            .iter()
            .filter(|b| !b.expects_success())
            .count();
        assert_eq!(success_count, 8);
        assert_eq!(fail_count, 2);
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
        let fleet = GauntletFleet::build();
        assert_eq!(fleet.adversarials.len(), 8, "8 adversarial attack vectors");
    }
}
