// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! CLI interface for adnet-testbots

use adnet_testbot::{Bot, BotContext, ExecutionContext, IdentityGenerator, Wallet};
use adnet_testbot_distributed::{Coordinator, Worker};
use adnet_testbot_roles::{GeneralUserBot, TraderBot};
use adnet_testbot_scenarios::{FleetType, GauntletPhaseRunner};
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "adnet-testbots")]
#[command(version = "0.1.0")]
#[command(about = "Production-grade bot testing infrastructure for Alpha/Delta protocol")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a named scenario (use "gauntlet-light" for TN006-LIGHT)
    Run {
        scenario: String,
        #[arg(long)]
        distributed: bool,
        #[arg(long)]
        duration: Option<u64>,
        #[arg(long, default_value = "10")]
        bots: usize,
    },

    /// Run the TN006 / TN006-LIGHT gauntlet phase-by-phase
    Gauntlet {
        /// Fleet size: light (66 bots, phases 0-6) or full (169 bots, phases 0-8)
        #[arg(long, default_value = "light")]
        fleet: FleetArg,

        /// Run only this phase number (0-8); omit to run all phases sequentially
        #[arg(long)]
        phase: Option<u8>,

        /// adnet unified API URL
        #[arg(long, default_value = "http://testnet001.ac-dc.network:8080")]
        adnet_url: String,

        /// Directory to write gauntlet output artifacts
        #[arg(long, default_value = "./gauntlet-output")]
        output_dir: String,
    },

    /// Start coordinator server
    Coordinator {
        #[arg(long, default_value = "0.0.0.0:50051")]
        bind: String,
    },

    /// Start worker daemon
    Worker {
        #[arg(long)]
        coordinator: String,
        #[arg(long, default_value = "100")]
        max_bots: u32,
        #[arg(long)]
        worker_id: Option<String>,
    },

    /// Show status
    Status {
        #[arg(long)]
        show_workers: bool,
    },

    /// Run a simple unit test
    Test { test_type: String },
}

/// Clap-friendly fleet argument.
#[derive(Clone, Debug, ValueEnum)]
enum FleetArg {
    Light,
    Full,
}

impl From<FleetArg> for FleetType {
    fn from(a: FleetArg) -> FleetType {
        match a {
            FleetArg::Light => FleetType::Light,
            FleetArg::Full => FleetType::Full,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            scenario,
            distributed,
            duration,
            bots,
        } => {
            run_scenario(&scenario, distributed, duration, bots).await?;
        }

        Commands::Gauntlet {
            fleet,
            phase,
            adnet_url,
            output_dir,
        } => {
            run_gauntlet(fleet.into(), phase, adnet_url, output_dir).await?;
        }

        Commands::Coordinator { bind } => {
            println!("Starting coordinator on: {}", bind);
            let coordinator = Coordinator::new();
            coordinator.serve(bind).await?;
        }

        Commands::Worker {
            coordinator,
            max_bots,
            worker_id,
        } => {
            let worker_id = worker_id
                .unwrap_or_else(|| format!("worker-{}", &uuid::Uuid::new_v4().to_string()[..8]));
            println!(
                "Starting worker: {} (coordinator: {}, max_bots: {})",
                worker_id, coordinator, max_bots
            );
            let worker = Worker::new(worker_id, coordinator, max_bots);
            worker.run().await?;
        }

        Commands::Status { show_workers } => {
            show_status(show_workers).await?;
        }

        Commands::Test { test_type } => {
            run_test(&test_type).await?;
        }
    }

    Ok(())
}

// =============================================================================
// Gauntlet command handler
// =============================================================================

async fn run_gauntlet(
    fleet_type: FleetType,
    phase_arg: Option<u8>,
    adnet_url: String,
    output_dir: String,
) -> anyhow::Result<()> {
    let max_phase = fleet_type.max_phase();
    println!(
        "Gauntlet: fleet={:?}, adnet_url={}, output_dir={}",
        fleet_type, adnet_url, output_dir
    );

    let mut runner = GauntletPhaseRunner::new(adnet_url, fleet_type, output_dir);

    if let Some(p) = phase_arg {
        // Single phase run
        let result = runner.run_phase(p).await?;
        print_phase_result(&result);
        if !result.success {
            anyhow::bail!("Phase {} FAILED", p);
        }
    } else {
        // Run all phases sequentially; abort on Phase 0 failure
        let mut total_pass = 0u8;
        let mut total_fail = 0u8;

        for p in 0..=max_phase {
            let result = runner.run_phase(p).await?;
            let ok = result.success;
            print_phase_result(&result);
            if ok {
                total_pass += 1;
            } else {
                total_fail += 1;
                if p == 0 {
                    println!("Phase 0 FAILED — aborting gauntlet (health gate not satisfied)");
                    anyhow::bail!("Gauntlet aborted: Phase 0 health gate failed");
                }
            }
        }

        println!(
            "\nGauntlet complete: {}/{} phases passed",
            total_pass,
            total_pass + total_fail
        );

        if total_fail > 0 {
            anyhow::bail!("Gauntlet FAILED ({} phase(s) failed)", total_fail);
        }
    }

    println!("Gauntlet PASSED");
    Ok(())
}

fn print_phase_result(r: &adnet_testbot_scenarios::PhaseResult) {
    println!(
        "Phase {}: {} ({}/{} bots ok, {} errors, {}ms)",
        r.phase,
        if r.success { "OK" } else { "FAILED" },
        r.bots_run as u64 - r.errors,
        r.bots_run,
        r.errors,
        r.duration_ms,
    );
}

// =============================================================================
// Scenario command handler (legacy)
// =============================================================================

async fn run_scenario(
    scenario: &str,
    distributed: bool,
    _duration: Option<u64>,
    bot_count: usize,
) -> anyhow::Result<()> {
    println!("Running scenario: {}", scenario);
    if !distributed {
        run_local_scenario(scenario, bot_count).await?;
    } else {
        println!("Distributed mode: use 'coordinator' and 'worker' commands first");
    }
    Ok(())
}

async fn run_local_scenario(scenario: &str, bot_count: usize) -> anyhow::Result<()> {
    // Route gauntlet scenarios to the dedicated command.
    if scenario == "gauntlet-light" || scenario == "gauntlet_light" {
        println!("Tip: use 'adnet-testbots gauntlet --fleet light' for the full gauntlet runner.");
        let adnet_url = std::env::var("ADNET_URL")
            .unwrap_or_else(|_| "http://testnet001.ac-dc.network:8080".to_string());
        run_gauntlet(
            FleetType::Light,
            None,
            adnet_url,
            "./gauntlet-output".to_string(),
        )
        .await?;
        return Ok(());
    }

    println!(
        "Setting up local scenario: {} ({} bots)",
        scenario, bot_count
    );
    let generator = IdentityGenerator::new();
    let mut bots: Vec<Box<dyn Bot>> = Vec::new();

    for i in 0..bot_count {
        let bot_id = format!("bot-{}", i);
        let bot: Box<dyn Bot> = match scenario {
            "alpha-transfer" | "simple-transfer" => Box::new(GeneralUserBot::new(bot_id)),
            "delta-trade" | "spot-trade" => Box::new(TraderBot::new(bot_id)),
            _ => Box::new(GeneralUserBot::new(bot_id)),
        };
        bots.push(bot);
    }

    println!("Created {} bots", bots.len());

    for (i, bot) in bots.iter_mut().enumerate() {
        let bot_id = format!("bot-{}", i);
        let identity = generator.generate(bot_id.clone())?;
        let wallet = Wallet::new(bot_id.clone());
        let execution_context = ExecutionContext::new(
            bot_id.clone(),
            "general_user".to_string(),
            adnet_testbot::context::NetworkEndpoints {
                alphaos_rest: "http://localhost:3030".to_string(),
                deltaos_rest: "http://localhost:3031".to_string(),
                adnet_unified: "http://localhost:3000".to_string(),
            },
        );
        let context = BotContext::new(execution_context, identity, wallet);
        bot.setup(&context).await?;
    }

    let start = std::time::Instant::now();
    let total = bots.len();

    for (i, bot) in bots.iter_mut().enumerate() {
        let behavior_id = match scenario {
            "alpha-transfer" | "simple-transfer" => "transfer",
            "delta-trade" | "spot-trade" => "spot_trade",
            _ => "default",
        };
        match bot.execute_behavior(behavior_id).await {
            Ok(r) => {
                if r.success {
                    print!(".");
                } else {
                    print!("F");
                }
            }
            Err(_) => print!("E"),
        }
        if (i + 1) % 50 == 0 {
            println!(" {}/{}", i + 1, total);
        }
    }

    println!();

    for bot in bots.iter_mut() {
        let _ = bot.teardown().await;
    }

    println!(
        "Done: {} bots, {:.2}s, {:.2} ops/s",
        bot_count,
        start.elapsed().as_secs_f64(),
        bot_count as f64 / start.elapsed().as_secs_f64()
    );

    Ok(())
}

// =============================================================================
// Status
// =============================================================================

async fn show_status(_show_workers: bool) -> anyhow::Result<()> {
    println!("adnet-testbots Status\nCoordinator: Not connected\nWorkers: 0 active");
    Ok(())
}

// =============================================================================
// Test command
// =============================================================================

async fn run_test(test_type: &str) -> anyhow::Result<()> {
    println!("Running test: {}", test_type);

    match test_type {
        "identity" => {
            let generator = IdentityGenerator::new();
            let identity = generator.generate("test-bot".to_string())?;
            println!(
                "Identity: {} / {} / {}",
                identity.id, identity.alpha_address, identity.delta_address
            );
        }

        "wallet" => {
            use adnet_testbot::{Balance, Token};
            let mut wallet = Wallet::new("test-bot".to_string());
            wallet.credit(Token::AX, Balance::new(1000))?;
            wallet.debit(Token::AX, Balance::new(300))?;
            println!("Wallet: {} AX", wallet.balance(&Token::AX));
        }

        "simple-transfer" => {
            let mut bot = GeneralUserBot::new("test-bot".to_string());
            let generator = IdentityGenerator::new();
            let identity = generator.generate("test-bot".to_string())?;
            let wallet = Wallet::new("test-bot".to_string());
            let execution_context = ExecutionContext::new(
                "test-bot".to_string(),
                "general_user".to_string(),
                adnet_testbot::context::NetworkEndpoints {
                    alphaos_rest: "http://localhost:3030".to_string(),
                    deltaos_rest: "http://localhost:3031".to_string(),
                    adnet_unified: "http://localhost:3000".to_string(),
                },
            );
            let context = BotContext::new(execution_context, identity, wallet);
            bot.setup(&context).await?;
            let result = bot.execute_behavior("transfer").await?;
            println!("Result: {}", result.message);
            bot.teardown().await?;
        }

        _ => {
            println!(
                "Unknown test: {}. Available: identity, wallet, simple-transfer",
                test_type
            );
        }
    }

    println!("Test complete");
    Ok(())
}
