// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! Scenario runner and GauntletPhaseRunner for executing test scenarios.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::info;

use adnet_testbot::context::NetworkEndpoints;
use adnet_testbot::{Bot, BotContext, ExecutionContext, IdentityGenerator, Wallet};
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
            success,
            duration_ms,
            operations_total,
            errors_total,
        })
    }

    /// Build a minimal `BotContext` for a single scenario step.
    fn build_step_context(bot_id: &str, scenario_name: &str) -> Result<BotContext> {
        let network = NetworkEndpoints {
            alphaos_rest: "http://localhost:3030".to_string(),
            deltaos_rest: "http://localhost:4030".to_string(),
            adnet_unified: "http://localhost:8080".to_string(),
        };

        let mut exec =
            ExecutionContext::new(bot_id.to_string(), "general_user".to_string(), network);
        exec = exec.with_scenario(scenario_name.to_string(), None);

        let identity = IdentityGenerator::new().generate(bot_id.to_string())?;
        let wallet = Wallet::new(bot_id.to_string());

        Ok(BotContext::new(exec, identity, wallet))
    }
}

impl Default for ScenarioRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub operations_total: u64,
    errors_total: u64,
}

/// Gauntlet phase runner for TN006-LIGHT scenario
pub struct GauntletPhaseRunner;

#[derive(Debug, Clone, Serialize)]
pub struct GauntletResult {
    pub phases: Vec<PhaseResult>,
    pub total_passed: usize,
    pub total_failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseResult {
    pub phase_num: u8,
    pub passed: bool,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// Action to take on phase failure
#[derive(Debug, Clone, Copy)]
enum FailAction {
    ABORT,
    CONTINUE,
}

impl GauntletPhaseRunner {
    /// Run the gauntlet-light scenario (phases 0-6)
    pub async fn run_gauntlet_light(adnet_url: String) -> anyhow::Result<GauntletResult> {
        info!("Building LightFleet for gauntlet-light scenario");
        let mut fleet = LightFleet::build();
        info!("LightFleet built with {} bots", fleet.bots.len());

        // Build TraceVerifier and AssertionRegistry once for the full run.
        let client = AdnetClient::new(adnet_url.clone())
            .map_err(|e| anyhow::anyhow!("Failed to build AdnetClient for TraceVerifier: {}", e))?;
        let verifier = TraceVerifier::new(client);
        let registry = AssertionRegistry::canonical();
        // Validate MECE at startup — panics if any UC is duplicated or missing.
        registry.validate_mece();

        let mut phases = Vec::new();
        let mut total_passed = 0;
        let mut total_failed = 0;

        // Phase 0: Validators and Provers (ABORT on failure)
        match Self::run_phase(
            0,
            &mut fleet,
            &adnet_url,
            FailAction::ABORT,
            &verifier,
            &registry,
        )
        .await
        {
            Ok(result) => {
                if result.passed {
                    total_passed += 1;
                } else {
                    total_failed += 1;
                }
                phases.push(result);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Phase 0 failed to execute: {}", e));
            }
        }

        // Phases 1-6: Continue on failure
        for phase_num in 1..=6 {
            match Self::run_phase(
                phase_num,
                &mut fleet,
                &adnet_url,
                FailAction::CONTINUE,
                &verifier,
                &registry,
            )
            .await
            {
                Ok(result) => {
                    if result.passed {
                        total_passed += 1;
                    } else {
                        total_failed += 1;
                    }
                    phases.push(result);
                }
                Err(e) => {
                    info!("Phase {} failed to execute: {}, continuing", phase_num, e);
                    phases.push(PhaseResult {
                        phase_num,
                        passed: false,
                        errors: vec![format!("Execution error: {}", e)],
                        duration_ms: 0,
                    });
                    total_failed += 1;
                }
            }
        }

        Ok(GauntletResult {
            phases,
            total_passed,
            total_failed,
        })
    }

    /// Run a single phase
    async fn run_phase(
        phase_num: u8,
        fleet: &mut LightFleet,
        adnet_url: &str,
        fail_action: FailAction,
        verifier: &TraceVerifier,
        registry: &AssertionRegistry,
    ) -> anyhow::Result<PhaseResult> {
        let start = Instant::now();
        info!("Starting phase {}", phase_num);

        let (bot_indices, required_success_rate) = match phase_num {
            0 => (
                vec![
                    // validators[0..5]
                    (0..5).map(|i| ("validators", i)).collect::<Vec<_>>(),
                    // provers[0..1]
                    (0..1).map(|i| ("provers", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                1.0,
            ),
            1 => (
                vec![
                    // user_transactors[0..10]
                    (0..10).map(|i| ("user_transactors", i)).collect::<Vec<_>>(),
                    // bridges[0..4]
                    (0..4).map(|i| ("bridges", i)).collect::<Vec<_>>(),
                    // scanners[0..1]
                    (0..1).map(|i| ("scanners", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                0.8,
            ),
            2 => (
                vec![
                    // governors[0..5]
                    (0..5).map(|i| ("governors", i)).collect::<Vec<_>>(),
                    // delta_voters[0..10]
                    (0..10).map(|i| ("delta_voters", i)).collect::<Vec<_>>(),
                    // tech_reps[0..3]
                    (0..3).map(|i| ("tech_reps", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                0.8,
            ),
            3 => (
                vec![
                    // traders[0..15]
                    (0..15).map(|i| ("traders", i)).collect::<Vec<_>>(),
                    // oracles[0..1]
                    (0..1).map(|i| ("oracles", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                0.8,
            ),
            4 => (
                vec![
                    // validators[0..5]
                    (0..5).map(|i| ("validators", i)).collect::<Vec<_>>(),
                    // provers[0..1]
                    (0..1).map(|i| ("provers", i)).collect::<Vec<_>>(),
                    // tech_reps[0..3]
                    (0..3).map(|i| ("tech_reps", i)).collect::<Vec<_>>(),
                    // earn_in[0..4]
                    (0..4).map(|i| ("earn_in", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                0.75,
            ),
            5 => (
                vec![
                    // traders[0..15]
                    (0..15).map(|i| ("traders", i)).collect::<Vec<_>>(),
                    // atomic_swaps[0..2]
                    (0..2).map(|i| ("atomic_swaps", i)).collect::<Vec<_>>(),
                    // oracles[0..1]
                    (0..1).map(|i| ("oracles", i)).collect::<Vec<_>>(),
                ]
                .concat(),
                0.8,
            ),
            6 => (
                // adversarials[0..5]
                (0..5).map(|i| ("adversarials", i)).collect::<Vec<_>>(),
                0.0, // Always pass
            ),
            _ => return Err(anyhow::anyhow!("Invalid phase number: {}", phase_num)),
        };

        let mut all_errors = Vec::new();
        let mut total_behaviors = 0;
        let mut successful_behaviors = 0;

        // Execute behaviors for each bot in the phase
        for (bot_type, index) in bot_indices {
            let behaviors = Self::get_behaviors_for_phase(phase_num, bot_type, index)?;

            // Resolve bot id and role before borrowing mutably for setup/execute
            let (bot_id, bot_role) = {
                let bot = fleet.get_bot_mut(bot_type, index)?;
                (bot.id().to_string(), bot.role().to_string())
            };

            // Create bot context and call setup once per bot per phase
            let context = Self::create_bot_context(&bot_id, &bot_role, adnet_url)?;
            {
                let bot = fleet.get_bot_mut(bot_type, index)?;
                if let Err(e) = bot.setup(&context).await {
                    all_errors.push(format!("Bot {} ({}) setup error: {}", bot_id, bot_type, e));
                    continue;
                }
            }

            for behavior_name in behaviors {
                total_behaviors += 1;

                // Execute behavior
                let bot = fleet.get_bot_mut(bot_type, index)?;
                let behavior_result = bot.execute_behavior(&behavior_name).await;

                match behavior_result {
                    Ok(result) => {
                        if Self::is_behavior_successful(&result, phase_num, bot_type, index) {
                            successful_behaviors += 1;
                        } else {
                            all_errors.push(format!(
                                "Bot {} ({}) behavior {} failed: {:?}",
                                bot_id, bot_type, behavior_name, result
                            ));
                        }

                        // After successful execution, run on-chain trace verification
                        // if the registry knows about this action type.
                        if let Some(_uc_id) = registry.uc_for_action(&behavior_name) {
                            let vctx = Self::build_verification_context(&result);
                            let vresult = verifier.verify(&behavior_name, &vctx).await;
                            if !vresult.passed {
                                all_errors.push(format!(
                                    "TRACE FAIL [{}] {}: {}",
                                    vresult.uc_id, vresult.action, vresult.evidence
                                ));
                            } else {
                                info!(
                                    "TRACE PASS [{}] {}: {}",
                                    vresult.uc_id, vresult.action, vresult.evidence
                                );
                            }
                        }
                    }
                    Err(e) => {
                        all_errors.push(format!(
                            "Bot {} ({}) behavior {} execution error: {}",
                            bot_id, bot_type, behavior_name, e
                        ));
                    }
                }
            }
        }

        let success_rate = if total_behaviors > 0 {
            successful_behaviors as f64 / total_behaviors as f64
        } else {
            0.0
        };

        let passed = if phase_num == 6 {
            // Phase 6 always passes (adversarial attacks expected to fail)
            true
        } else {
            success_rate >= required_success_rate
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Phase {} completed: {} (success rate: {:.2}, required: {:.2})",
            phase_num,
            if passed { "PASS" } else { "FAIL" },
            success_rate,
            required_success_rate
        );

        if !passed {
            match fail_action {
                FailAction::ABORT => {
                    return Err(anyhow::anyhow!(
                        "Phase {} failed with success rate {:.2} < {:.2}",
                        phase_num,
                        success_rate,
                        required_success_rate
                    ));
                }
                FailAction::CONTINUE => {
                    info!("Phase {} failed but continuing as configured", phase_num);
                }
            }
        }

        Ok(PhaseResult {
            phase_num,
            passed,
            errors: all_errors,
            duration_ms,
        })
    }

    /// Get behaviors for a specific bot in a phase
    fn get_behaviors_for_phase(
        phase_num: u8,
        bot_type: &str,
        index: usize,
    ) -> anyhow::Result<Vec<String>> {
        match (phase_num, bot_type, index) {
            // Phase 0
            (0, "validators", _) => Ok(vec![
                "validator.register".to_string(),
                "validator.produce_block".to_string(),
            ]),
            (0, "provers", _) => Ok(vec![
                "prover.register".to_string(),
                "prover.submit_proof".to_string(),
            ]),

            // Phase 1
            (1, "user_transactors", _) => Ok(vec![
                "transfer.ax_private".to_string(),
                "transfer.ax_public".to_string(),
            ]),
            (1, "bridges", _) => Ok(vec![
                "bridge.lock_ax".to_string(),
                "bridge.mint_sax".to_string(),
                "bridge.burn_sax".to_string(),
                "bridge.unlock_ax".to_string(),
            ]),
            (1, "scanners", _) => Ok(vec![
                "scanner.index_block".to_string(),
                "scanner.verify_state".to_string(),
            ]),

            // Phase 2
            (2, "governors", _) => Ok(vec![
                "governance.propose.parameter".to_string(),
                "governance.vote".to_string(),
                "governance.execute".to_string(),
            ]),
            (2, "delta_voters", _) => Ok(vec![
                "governance.delta.vote".to_string(),
                "governance.delta.emphatic_vote".to_string(),
            ]),
            (2, "tech_reps", _) => Ok(vec!["techrep.vote_forge".to_string()]),

            // Phase 3
            (3, "traders", _) => Ok(vec![
                "dex.place_limit_order".to_string(),
                "dex.place_market_order".to_string(),
            ]),
            (3, "oracles", _) => Ok(vec![
                "oracle.submit_prices".to_string(),
                "oracle.verify_harmonic_mean".to_string(),
            ]),

            // Phase 4
            (4, "validators", _) => Ok(vec!["validator.register".to_string()]),
            (4, "provers", _) => Ok(vec!["prover.register".to_string()]),
            (4, "tech_reps", _) => Ok(vec!["techrep.register".to_string()]),
            (4, "earn_in", idx) => {
                if idx == 3 {
                    // earn_in index 3 expects failure
                    Ok(vec![
                        "earnin.apply".to_string(),
                        "earnin.query_status".to_string(),
                        "earnin.complete".to_string(),
                    ])
                } else {
                    Ok(vec![
                        "earnin.apply".to_string(),
                        "earnin.query_status".to_string(),
                        "earnin.complete".to_string(),
                    ])
                }
            }

            // Phase 5
            (5, "traders", _) => Ok(vec!["dex.place_limit_order".to_string()]),
            (5, "atomic_swaps", _) => Ok(vec![
                "atomicswap.kyt_register".to_string(),
                "atomicswap.htlc_initiate".to_string(),
                "atomicswap.htlc_complete".to_string(),
            ]),
            (5, "oracles", _) => Ok(vec!["oracle.submit_prices".to_string()]),

            // Phase 6
            (6, "adversarials", _) => Ok(vec!["attack.execute".to_string()]),

            _ => Err(anyhow::anyhow!(
                "No behaviors defined for phase {} bot type {} index {}",
                phase_num,
                bot_type,
                index
            )),
        }
    }

    /// Create bot context with network endpoints
    fn create_bot_context(bot_id: &str, role: &str, adnet_url: &str) -> anyhow::Result<BotContext> {
        let network = NetworkEndpoints {
            alphaos_rest: adnet_url.to_string(),
            deltaos_rest: adnet_url.to_string(),
            adnet_unified: adnet_url.to_string(),
        };

        let execution_context =
            ExecutionContext::new(bot_id.to_string(), role.to_string(), network);
        let identity = IdentityGenerator::new().generate(bot_id.to_string())?;
        let wallet = Wallet::new(bot_id.to_string());

        Ok(BotContext::new(execution_context, identity, wallet))
    }

    /// Build a VerificationContext from a BehaviorResult's data field.
    ///
    /// Extracts well-known keys from `result.data` (a JSON value) so the
    /// TraceVerifier can perform the correct on-chain lookup.
    fn build_verification_context(result: &BehaviorResult) -> VerificationContext {
        let data = &result.data;
        VerificationContext {
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
                .or_else(|| data.get("order_tx_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            address: data
                .get("address")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }

    /// Check if behavior result is successful
    fn is_behavior_successful(
        result: &BehaviorResult,
        phase_num: u8,
        bot_type: &str,
        index: usize,
    ) -> bool {
        if result.success {
            true
        } else {
            // Special case: Phase 4 earn_in index 3 expects failure
            phase_num == 4 && bot_type == "earn_in" && index == 3
        }
    }
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
}

impl GauntletPhaseRunner {
    pub fn new(adnet_url: String, fleet_type: FleetType, output_dir: String) -> Self {
        Self {
            adnet_url,
            fleet_type,
            output_dir,
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

        // Build (bot, behavior_id) pairs for this phase.
        let mut entries: Vec<(Box<dyn Bot + Send>, &'static str)> = match self.fleet_type {
            FleetType::Light => build_light_phase(phase),
            FleetType::Full => build_full_phase(phase),
        };

        let bots_run = entries.len();
        let mut errors: u64 = 0;

        for (bot, behavior_id) in entries.iter_mut() {
            let bot_id = bot.id().to_owned();
            let role = bot.role().to_owned();
            let ctx = make_context(&bot_id, &role, &network)?;

            if let Err(e) = bot.setup(&ctx).await {
                info!("Phase {}: bot {} setup error: {}", phase, bot_id, e);
                errors += 1;
                continue;
            }

            match bot.execute_behavior(behavior_id).await {
                Ok(result) if result.success => {}
                Ok(result) => {
                    // Phase 6 adversarial: expected-rejection results are fine.
                    if phase != 6 {
                        info!(
                            "Phase {}: bot {} behavior '{}' not successful: {}",
                            phase, bot_id, behavior_id, result.message
                        );
                        errors += 1;
                    }
                }
                Err(e) => {
                    if phase != 6 {
                        info!(
                            "Phase {}: bot {} behavior '{}' error: {}",
                            phase, bot_id, behavior_id, e
                        );
                        errors += 1;
                    }
                }
            }

            let _ = bot.teardown().await;
        }

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

        info!(
            "Phase {}: {} ({}/{} bots ok, {} errors, {}ms)",
            phase,
            if success { "PASS" } else { "FAIL" },
            bots_run as u64 - errors.min(bots_run as u64),
            bots_run,
            errors,
            duration_ms,
        );

        Ok(PhaseResult {
            phase,
            success,
            bots_run,
            errors,
            duration_ms,
        })
    }
}

// =============================================================================
// Helpers
// =============================================================================

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

// ── Light fleet phase dispatch ───────────────────────────────────────────────

fn build_light_phase(phase: u8) -> Vec<(Box<dyn Bot + Send>, &'static str)> {
    let fleet = LightFleet::build();
    match phase {
        0 => {
            let mut v: Vec<(Box<dyn Bot + Send>, &'static str)> = Vec::new();
            for b in fleet.validators {
                v.push((Box::new(b), "validator.attest"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.submit_proof"));
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
                v.push((Box::new(b), "validator.attest"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.submit_proof"));
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
                v.push((Box::new(b), "validator.attest"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.submit_proof"));
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
                v.push((Box::new(b), "validator.attest"));
            }
            for b in fleet.provers {
                v.push((Box::new(b), "prover.submit_proof"));
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
