// Copyright (c) 2025-2026 ACDC Network
// SPDX-License-Identifier: Apache-2.0

//! CLI interface for adnet-testbots

use adnet_testbot::{Bot, BotContext, ExecutionContext, IdentityGenerator, Wallet};
use adnet_testbot_distributed::{Coordinator, Worker};
use adnet_testbot_roles::{GeneralUserBot, TraderBot};
use adnet_testbot_scenarios::{GauntletPhaseRunner, ScenarioRunner};
use clap::{Parser, Subcommand};
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
            println!("Status command - show_workers: {}", show_workers);
            // TODO: Implement status reporting
        }

        Commands::Test { test_type } => {
            println!("Running test: {}", test_type);
            // TODO: Implement test execution
        }
    }

    Ok(())
}

async fn run_scenario(
    scenario: &str,
    distributed: bool,
    duration: Option<u64>,
    bots: usize,
) -> anyhow::Result<()> {
    if distributed {
        println!("Running scenario '{}' in distributed mode", scenario);
        // TODO: Implement distributed execution
    } else {
        println!("Running scenario '{}' locally", scenario);
    }

    match scenario {
        "gauntlet-light" | "gauntlet_light" => {
            let adnet_url = std::env::var("ADNET_URL")
                .unwrap_or_else(|_| "http://testnet001.ac-dc.network:8080".to_string());
            println!("Running gauntlet-light on {}", adnet_url);
            let result = GauntletPhaseRunner::run_gauntlet_light(adnet_url).await?;
            println!(
                "Gauntlet-light complete: {}/{} phases passed",
                result.total_passed,
                result.total_passed + result.total_failed
            );
            for phase in &result.phases {
                println!(
                    "  Phase {}: {} ({} ms)",
                    phase.phase_num,
                    if phase.passed { "PASS" } else { "FAIL" },
                    phase.duration_ms
                );
                if !phase.errors.is_empty() {
                    println!("    Errors: {}", phase.errors.len());
                    for error in &phase.errors {
                        println!("      - {}", error);
                    }
                }
            }
        }
        _ => {
            println!("Running generic scenario: {}", scenario);
            let runner = ScenarioRunner::new();
            let result = runner.run_scenario(scenario).await?;
            println!("Scenario result: {:?}", result);
        }
    }

    Ok(())
}
