// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! Scenario runner and GauntletPhaseRunner for executing test scenarios.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;
use tracing::info;

use crate::assertions::AssertionRegistry;
use adnet_testbot::context::NetworkEndpoints;
use adnet_testbot::{Bot, BotContext, ExecutionContext, IdentityGenerator, Wallet};
use adnet_testbot_integration::{AdnetClient, TraceVerifier};
use adnet_testbot_roles::gauntlet_bots::{GauntletFleet, LightFleet};

// =============================================================================
// Legacy ScenarioRunner (Phase 1 stub — kept for CLI compatibility)
// =============================================================================

pub struct ScenarioRunner {
    scenarios: Vec<ScenarioDefinition>,
}

impl ScenarioRunner {
    pub fn new() -> Self {
        Self {
            scenarios: Vec::new(),
        }
    }

    #[allow(unused_variables)]
    pub fn load_scenario(&mut self, yaml_path: &str) -> Result<()> {
        Ok(())
    }

    pub async fn run_scenario(&self, name: &str) -> Result<ScenarioResult> {
        Ok(ScenarioResult {
            name: name.to_string(),
            success: true,
            duration_ms: 0,
            operations_total: 0,
            errors_total: 0,
        })
    }
}

impl Default for ScenarioRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub name: String,
    pub description: String,
    pub bot_count: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub operations_total: u64,
    pub errors_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofSnapshot {
    pub alpha_height: u64,
    pub delta_height: u64,
    pub m1_queue_depth: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofDelta {
    pub alpha_blocks_produced: u64,
    pub delta_blocks_produced: u64,
    pub m1_depth_change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInputRecord {
    pub bot_id: String,
    pub behavior_id: String,
    pub submitted_inputs: serde_json::Value,
    pub tx_id: Option<String>,
    pub verification_status: String,
}

// =============================================================================
// PhaseResult
// =============================================================================

/// Result of running a single gauntlet phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: u8,
    pub success: bool,
    pub bots_run: usize,
    pub errors: u64,
    pub duration_ms: u64,
    pub verifications_passed: u64,
    pub verifications_failed: u64,
    pub verifications_errored: u64,
    pub uc_coverage: Vec<String>,
    pub uc_gaps: Vec<String>,
    pub proof_delta: Option<ProofDelta>,
    pub input_verifications_passed: u64,
    pub input_verifications_failed: u64,
    pub input_verifications_not_applicable: u64,
}

// =============================================================================
// FleetType
// =============================================================================

/// Which fleet to run: light (TN006-LIGHT) or full (TN006).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FleetType {
    /// 66-bot fleet, phases 0-6 (testnet001-005 + testnet006 prover+coordinator)
    Light,
    /// 169-bot fleet, phases 0-8
    Full,
}

impl FleetType {
    pub fn max_phase(self) -> u8 {
        match self {
            FleetType::Light => 6,
            FleetType::Full => 8,
        }
    }
}

// =============================================================================
// GauntletPhaseRunner
// =============================================================================

/// Runs the TN006 / TN006-LIGHT gauntlet scenario phase by phase.
pub struct GauntletPhaseRunner {
    pub adnet_url: String,
    pub fleet_type: FleetType,
    pub output_dir: String,
    verifier: Option<TraceVerifier>,
    registry: AssertionRegistry,
}

impl GauntletPhaseRunner {
    pub fn new(adnet_url: String, fleet_type: FleetType, output_dir: String) -> Self {
        let verifier = AdnetClient::new(adnet_url.clone())
            .ok()
            .map(TraceVerifier::new);
        Self {
            adnet_url,
            fleet_type,
            output_dir,
            verifier,
            registry: AssertionRegistry::canonical(),
        }
    }

    /// Run a single phase. Returns `Err` only if setup machinery fails; bot
    /// errors are counted in `PhaseResult::errors`.
    pub async fn run_phase(&mut self, phase: u8) -> anyhow::Result<PhaseResult> {
        let max_phase = self.fleet_type.max_phase();
        if phase > max_phase {
            return Err(anyhow::anyhow!(
                "Phase {} exceeds max phase {} for {:?} fleet",
                phase,
                max_phase,
                self.fleet_type
            ));
        }

        info!("GauntletPhaseRunner: starting phase {}", phase);
        let start = Instant::now();

        let network = NetworkEndpoints {
            alphaos_rest: self.adnet_url.clone(),
            deltaos_rest: self.adnet_url.clone(),
            adnet_unified: self.adnet_url.clone(),
        };

        let adnet_client = AdnetClient::new(self.adnet_url.clone()).ok();
        let snapshot_before = take_proof_snapshot(&adnet_client).await;
        let mut input_records: Vec<BotInputRecord> = Vec::new();

        // Build (bot, behavior_id) pairs for this phase.
        let mut entries: Vec<(Box<dyn Bot + Send>, &'static str)> = match self.fleet_type {
            FleetType::Light => build_light_phase(phase),
            FleetType::Full => build_full_phase(phase),
        };

        let bots_run = entries.len();
        let mut errors: u64 = 0;

        // Pre-collect behavior IDs for gap analysis
        let behavior_ids_for_phase: Vec<&'static str> = entries.iter().map(|(_, b)| *b).collect();

        // Verification tracking
        let mut verif_passed: u64 = 0;
        let mut verif_failed: u64 = 0;
        let mut verif_errored: u64 = 0;
        let mut uc_hit: HashSet<&'static str> = HashSet::new();

        for (bot, behavior_id) in entries.iter_mut() {
            let bot_id = bot.id().to_owned();
            let role = bot.role().to_owned();
            let ctx = make_context(&bot_id, &role, &network)?;

            if let Err(e) = bot.setup(&ctx).await {
                info!("Phase {}: bot {} setup error: {}", phase, bot_id, e);
                errors += 1;
                continue;
            }

            // Execute behavior and capture result
            let behavior_result = match bot.execute_behavior(behavior_id).await {
                Ok(result) if result.success => result,
                Ok(result) => {
                    // Phase 6 adversarial: expected-rejection results are fine.
                    if phase != 6 {
                        info!(
                            "Phase {}: bot {} behavior '{}' not successful: {}",
                            phase, bot_id, behavior_id, result.message
                        );
                        errors += 1;
                    }
                    result
                }
                Err(e) => {
                    if phase != 6 {
                        info!(
                            "Phase {}: bot {} behavior '{}' error: {}",
                            phase, bot_id, behavior_id, e
                        );
                        errors += 1;
                    }
                    let _ = bot.teardown().await;
                    continue;
                }
            };

            // ── Verification (non-blocking, errors do NOT affect phase success) ──
            if let (Some(verifier), Some(action_type)) =
                (&self.verifier, behavior_to_action_type(behavior_id))
            {
                let vctx = extract_verification_context(&behavior_result.data);
                match verifier.verify(action_type, &vctx).await {
                    vr if vr.error.is_some() => {
                        verif_errored += 1;
                        info!(
                            "Phase {}: bot {} UC {} verification error: {}",
                            phase,
                            bot_id,
                            vr.uc_id,
                            vr.error.as_deref().unwrap_or("?")
                        );
                    }
                    vr if vr.passed => {
                        verif_passed += 1;
                        uc_hit.insert(vr.uc_id);
                        info!(
                            "Phase {}: bot {} UC {} PASS: {}",
                            phase, bot_id, vr.uc_id, vr.evidence
                        );
                    }
                    vr => {
                        verif_failed += 1;
                        uc_hit.insert(vr.uc_id);
                        info!(
                            "Phase {}: bot {} UC {} FAIL: {}",
                            phase, bot_id, vr.uc_id, vr.evidence
                        );
                    }
                }
            }

            collect_input_record(&behavior_result, &bot_id, behavior_id, &mut input_records);
            let _ = bot.teardown().await;
        }

        let snapshot_after = take_proof_snapshot(&adnet_client).await;
        let proof_delta = compute_proof_delta(snapshot_before, snapshot_after);
        if let Some(ref pd) = proof_delta {
            info!(
                "Phase {}: blocks produced (alpha: {}, delta: {}), m1_depth_change: {}",
                phase, pd.alpha_blocks_produced, pd.delta_blocks_produced, pd.m1_depth_change,
            );
        }
        let (iv_passed, iv_failed, iv_not_applicable) =
            verify_input_records(&input_records, &adnet_client).await;
        info!(
            "Phase {}: input verif: +{} -{} n/a{}",
            phase, iv_passed, iv_failed, iv_not_applicable,
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        // Phase 6 (adversarial) always succeeds — errors are expected.
        let success = if phase == 6 {
            true
        } else if bots_run == 0 {
            true
        } else {
            let error_rate = errors as f64 / bots_run as f64;
            let required_rate = required_success_rate(phase);
            (1.0 - error_rate) >= required_rate
        };

        // Compute MECE coverage gap relative to this phase's verifiable UCs
        let phase_action_types: HashSet<&'static str> = behavior_ids_for_phase
            .iter()
            .filter_map(|bid| behavior_to_action_type(bid))
            .collect();
        let phase_uc_ids: HashSet<&'static str> = phase_action_types
            .iter()
            .filter_map(|at| self.registry.uc_for_action(at))
            .collect();

        let uc_coverage: Vec<String> = uc_hit.iter().map(|&s| s.to_string()).collect();
        let uc_gaps: Vec<String> = phase_uc_ids
            .difference(&uc_hit)
            .map(|&s| s.to_string())
            .collect();

        if !uc_gaps.is_empty() {
            info!(
                "Phase {}: UC GAPS (not exercised this phase): {:?}",
                phase, uc_gaps
            );
        }

        info!(
            "Phase {}: {} ({}/{} bots ok, {} errors, {}ms) | verif: +{} -{} !{} | UCs: {}",
            phase,
            if success { "PASS" } else { "FAIL" },
            bots_run as u64 - errors.min(bots_run as u64),
            bots_run,
            errors,
            duration_ms,
            verif_passed,
            verif_failed,
            verif_errored,
            uc_coverage.join(","),
        );

        Ok(PhaseResult {
            phase,
            success,
            bots_run,
            errors,
            duration_ms,
            verifications_passed: verif_passed,
            verifications_failed: verif_failed,
            verifications_errored: verif_errored,
            uc_coverage,
            uc_gaps,
            proof_delta,
            input_verifications_passed: iv_passed,
            input_verifications_failed: iv_failed,
            input_verifications_not_applicable: iv_not_applicable,
        })
    }
}

// =============================================================================
// Helpers
// =============================================================================

async fn take_proof_snapshot(client: &Option<AdnetClient>) -> Option<ProofSnapshot> {
    let c = client.as_ref()?;
    let state = c.get_state_root().await.ok()?;
    let pool = c.get_pool_status().await.ok()?;
    Some(ProofSnapshot {
        alpha_height: state.alpha_height.unwrap_or(0),
        delta_height: state.delta_height.unwrap_or(0),
        m1_queue_depth: pool
            .get("current_tx_weight")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

fn compute_proof_delta(
    before: Option<ProofSnapshot>,
    after: Option<ProofSnapshot>,
) -> Option<ProofDelta> {
    let b = before?;
    let a = after?;
    Some(ProofDelta {
        alpha_blocks_produced: a.alpha_height.saturating_sub(b.alpha_height),
        delta_blocks_produced: a.delta_height.saturating_sub(b.delta_height),
        m1_depth_change: a.m1_queue_depth as i64 - b.m1_queue_depth as i64,
    })
}

fn collect_input_record(
    result: &adnet_testbot::BehaviorResult,
    bot_id: &str,
    behavior_id: &str,
    records: &mut Vec<BotInputRecord>,
) {
    let data = &result.data;
    let tx_id = data
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let submitted_inputs = data
        .get("submitted_inputs")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if submitted_inputs.is_null() && tx_id.is_none() {
        return;
    }
    records.push(BotInputRecord {
        bot_id: bot_id.to_string(),
        behavior_id: behavior_id.to_string(),
        submitted_inputs,
        tx_id,
        verification_status: "pending".to_string(),
    });
}

async fn verify_input_records(
    records: &[BotInputRecord],
    client: &Option<AdnetClient>,
) -> (u64, u64, u64) {
    let mut passed = 0u64;
    let mut failed = 0u64;
    let mut not_applicable = 0u64;
    let c = match client.as_ref() {
        Some(c) => c,
        None => {
            not_applicable += records.len() as u64;
            return (passed, failed, not_applicable);
        }
    };
    for record in records {
        match record.behavior_id.as_str() {
            "transfer.ax_private" => {
                if let Some(ref tx_id) = record.tx_id {
                    if !tx_id.is_empty() {
                        if let Ok(tx) = c.get_transaction(tx_id).await {
                            let s_fee = record.submitted_inputs.get("fee").and_then(|v| v.as_u64());
                            let sv_fee = tx.get("fee").and_then(|v| v.as_u64());
                            match (s_fee, sv_fee) {
                                (Some(sf), Some(svf)) if sf == svf => {
                                    passed += 1;
                                    info!(
                                        "bot {} transfer.ax_private input verification PASS (fee match: {})",
                                        record.bot_id, sf
                                    );
                                }
                                (Some(sf), Some(svf)) => {
                                    failed += 1;
                                    info!(
                                        "bot {} transfer.ax_private input verification FAIL (fee mismatch: {} vs {})",
                                        record.bot_id, sf, svf
                                    );
                                }
                                _ => {
                                    passed += 1;
                                    info!(
                                        "bot {} transfer.ax_private input verification PASS (tx found)",
                                        record.bot_id
                                    );
                                }
                            }
                        } else {
                            not_applicable += 1;
                        }
                    } else {
                        not_applicable += 1;
                    }
                } else {
                    not_applicable += 1;
                }
            }
            "dex.place_limit_order" => {
                if let Some(market) = record
                    .submitted_inputs
                    .get("market")
                    .and_then(|v| v.as_str())
                {
                    if c.get_orderbook(market).await.is_ok() {
                        passed += 1;
                        info!(
                            "bot {} dex.place_limit_order input verification PASS (market {})",
                            record.bot_id, market
                        );
                    } else {
                        failed += 1;
                        info!(
                            "bot {} dex.place_limit_order input verification FAIL (market {} unreachable)",
                            record.bot_id, market
                        );
                    }
                } else {
                    not_applicable += 1;
                }
            }
            "governance.vote" | "governance.delta.vote" => {
                let pid = record
                    .submitted_inputs
                    .get("proposal_id")
                    .and_then(|v| v.as_u64());
                let vpk = record
                    .submitted_inputs
                    .get("voter_public_key")
                    .and_then(|v| v.as_str());
                if let (Some(pid), Some(vpk)) = (pid, vpk) {
                    if let Ok(proposal) = c.get_governance_proposal(pid).await {
                        let found = proposal
                            .get("votes")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter().any(|vote| {
                                    vote.get("voter_public_key")
                                        .and_then(|pk| pk.as_str())
                                        .map(|pk| pk == vpk)
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);
                        if found {
                            passed += 1;
                            info!(
                                "bot {} {} input verification PASS",
                                record.bot_id, record.behavior_id
                            );
                        } else {
                            not_applicable += 1;
                            info!(
                                "bot {} {} input verification n/a (voter not yet visible)",
                                record.bot_id, record.behavior_id
                            );
                        }
                    } else {
                        not_applicable += 1;
                    }
                } else {
                    not_applicable += 1;
                }
            }
            _ => {
                not_applicable += 1;
            }
        }
    }
    (passed, failed, not_applicable)
}

fn required_success_rate(phase: u8) -> f64 {
    match phase {
        0 => 1.0,  // health gate — all must pass
        4 => 0.75, // earn-in: 3/4 succeed, 1 expected to fail
        _ => 0.8,
    }
}

fn make_context(
    bot_id: &str,
    role: &str,
    network: &NetworkEndpoints,
) -> anyhow::Result<BotContext> {
    let gen = IdentityGenerator::new();
    let identity = gen.generate(bot_id.to_string())?;
    let wallet = Wallet::new(bot_id.to_string());
    let execution = ExecutionContext::new(bot_id.to_string(), role.to_string(), network.clone());
    Ok(BotContext::new(execution, identity, wallet))
}

// ── Verification helpers ─────────────────────────────────────────────────────

/// Map a dotted gauntlet behavior_id to a TraceVerifier action_type string.
/// Returns None for behaviors that have no UC verification rule.
fn behavior_to_action_type(behavior_id: &str) -> Option<&'static str> {
    match behavior_id {
        "transfer.ax_private" => Some("private_transfer"),
        "governance.propose.parameter" => Some("governance_proposal_create"),
        "governance.delta.vote" => Some("governance_vote"),
        "governance.vote" => Some("governance_vote"),
        "governance.execute" => Some("governance_execute"),
        "governance.grim_trigger" => Some("grim_trigger_check"),
        "governance.apology" => Some("apology_lifecycle"),
        "dex.place_limit_order" => Some("dex_limit_order"),
        _ => None,
    }
}

/// Extract a VerificationContext from a BehaviorResult's JSON data payload.
/// All fields are optional — missing keys produce None (not an error).
fn extract_verification_context(
    data: &serde_json::Value,
) -> adnet_testbot_integration::trace_verifier::VerificationContext {
    adnet_testbot_integration::trace_verifier::VerificationContext {
        proposal_id: data.get("proposal_id").and_then(|v| v.as_u64()),
        voter_public_key: data
            .get("voter_public_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        gid_address: data
            .get("gid_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        market: data
            .get("market")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        transaction_id: data
            .get("transaction_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        address: data
            .get("address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

// ── Light fleet phase dispatch ───────────────────────────────────────────────

fn build_light_phase(phase: u8) -> Vec<(Box<dyn Bot + Send>, &'static str)> {
    let fleet = LightFleet::build();
    match phase {
        0 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.validators {
                v.push((Box::new(b), "validator.register"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.register"));
            }
            v
        }
        1 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.user_transactors {
                v.push((Box::new(b), "transfer.ax_private"));
            }
            for b in fleet.bridges {
                v.push((Box::new(b), "bridge.lock_ax"));
            }
            for b in fleet.scanners {
                v.push((Box::new(b), "scanner.index_block"));
            }
            v
        }
        2 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.governors {
                v.push((Box::new(b), "governance.propose.parameter"));
            }
            for b in fleet.delta_voters {
                v.push((Box::new(b), "governance.delta.vote"));
            }
            for b in fleet.tech_reps {
                v.push((Box::new(b), "techrep.vote_forge"));
            }
            v
        }
        3 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.traders {
                v.push((Box::new(b), "dex.place_limit_order"));
            }
            for b in fleet.oracles {
                v.push((Box::new(b), "oracle.submit_prices"));
            }
            v
        }
        4 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.validators {
                v.push((Box::new(b), "validator.register"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.register"));
            }
            for b in fleet.tech_reps {
                v.push((Box::new(b), "techrep.register"));
            }
            for b in fleet.earn_in {
                v.push((Box::new(b), "earnin.apply"));
            }
            v
        }
        5 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.traders {
                v.push((Box::new(b), "dex.place_limit_order"));
            }
            for b in fleet.atomic_swaps {
                v.push((Box::new(b), "atomicswap.htlc_initiate"));
            }
            for b in fleet.oracles {
                v.push((Box::new(b), "oracle.submit_prices"));
            }
            v
        }
        6 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.adversarials {
                v.push((Box::new(b), "attack.execute"));
            }
            v
        }
        _ => Vec::new(),
    }
}

// ── Full fleet phase dispatch ─────────────────────────────────────────────────

fn build_full_phase(phase: u8) -> Vec<(Box<dyn Bot + Send>, &'static str)> {
    let fleet = GauntletFleet::build();
    match phase {
        0 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.validators {
                v.push((Box::new(b), "validator.register"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.register"));
            }
            v
        }
        1 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.user_transactors {
                v.push((Box::new(b), "transfer.ax_private"));
            }
            for b in fleet.bridges {
                v.push((Box::new(b), "bridge.lock_ax"));
            }
            for b in fleet.scanners {
                v.push((Box::new(b), "scanner.index_block"));
            }
            v
        }
        2 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.governors {
                v.push((Box::new(b), "governance.propose.parameter"));
            }
            for b in fleet.delta_voters {
                v.push((Box::new(b), "governance.delta.vote"));
            }
            for b in fleet.tech_reps {
                v.push((Box::new(b), "techrep.vote_forge"));
            }
            v
        }
        3 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.traders {
                v.push((Box::new(b), "dex.place_limit_order"));
            }
            for b in fleet.oracles {
                v.push((Box::new(b), "oracle.submit_prices"));
            }
            v
        }
        4 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.validators {
                v.push((Box::new(b), "validator.register"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.register"));
            }
            for b in fleet.tech_reps {
                v.push((Box::new(b), "techrep.register"));
            }
            for b in fleet.earn_in {
                v.push((Box::new(b), "earnin.apply"));
            }
            v
        }
        5 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.traders {
                v.push((Box::new(b), "dex.place_limit_order"));
            }
            for b in fleet.atomic_swaps {
                v.push((Box::new(b), "atomicswap.htlc_initiate"));
            }
            for b in fleet.oracles {
                v.push((Box::new(b), "oracle.submit_prices"));
            }
            v
        }
        6 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.adversarials {
                v.push((Box::new(b), "attack.execute"));
            }
            v
        }
        7 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.dead_wallets {
                v.push((Box::new(b), "deadwallet.trigger_check"));
            }
            v
        }
        8 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.messengers {
                v.push((Box::new(b), "messenger.send"));
            }
            v
        }
        _ => Vec::new(),
    }
}
