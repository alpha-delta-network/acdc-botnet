/// CLI interface for adnet-testbots
use adnet_testbot::{Bot, BotContext, ExecutionContext, Identity, IdentityGenerator, Wallet};
use adnet_testbot_distributed::{Coordinator, Worker};
use adnet_testbot_metrics::{EventRecorder, MetricsAggregator};
use adnet_testbot_roles::{GeneralUserBot, TraderBot};
use adnet_testbot_scenarios::ScenarioRunner;
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
    /// Run a scenario
    Run {
        /// Scenario name
        scenario: String,

        /// Run in distributed mode
        #[arg(long)]
        distributed: bool,

        /// Duration limit in seconds
        #[arg(long)]
        duration: Option<u64>,

        /// Number of bots
        #[arg(long, default_value = "10")]
        bots: usize,
    },

    /// Start coordinator server
    Coordinator {
        /// Bind address
        #[arg(long, default_value = "0.0.0.0:50051")]
        bind: String,
    },

    /// Start worker daemon
    Worker {
        /// Coordinator address
        #[arg(long)]
        coordinator: String,

        /// Maximum number of bots
        #[arg(long, default_value = "100")]
        max_bots: u32,

        /// Worker ID (auto-generated if not provided)
        #[arg(long)]
        worker_id: Option<String>,
    },

    /// Show status
    Status {
        /// Show worker details
        #[arg(long)]
        show_workers: bool,
    },

    /// Run a simple test
    Test {
        /// Test type: "simple-transfer", "identity", "wallet"
        test_type: String,
    },
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

        Commands::Coordinator { bind } => {
            println!("🚀 Starting coordinator on: {}", bind);
            let coordinator = Coordinator::new();
            coordinator.serve(bind).await?;
        }

        Commands::Worker {
            coordinator,
            max_bots,
            worker_id,
        } => {
            let worker_id = worker_id.unwrap_or_else(|| {
                format!("worker-{}", uuid::Uuid::new_v4().to_string()[..8].to_string())
            });

            println!(
                "🤖 Starting worker: {} (coordinator: {}, max_bots: {})",
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

async fn run_scenario(
    scenario: &str,
    distributed: bool,
    duration: Option<u64>,
    bot_count: usize,
) -> anyhow::Result<()> {
    println!("📊 Running scenario: {}", scenario);
    println!("   Mode: {}", if distributed { "distributed" } else { "local" });
    println!("   Bots: {}", bot_count);

    if let Some(d) = duration {
        println!("   Duration: {}s", d);
    }

    // For Phase 1, run a simple local scenario
    if !distributed {
        run_local_scenario(scenario, bot_count).await?;
    } else {
        println!("⚠️  Distributed mode not fully implemented in Phase 1");
        println!("   Use 'coordinator' and 'worker' commands to set up cluster first");
    }

    Ok(())
}

async fn run_local_scenario(scenario: &str, bot_count: usize) -> anyhow::Result<()> {
    println!("\n🏗️  Setting up local scenario...");

    // Create metrics infrastructure
    let recorder = EventRecorder::new();
    let aggregator = MetricsAggregator::new();

    // Generate bot identities
    println!("🔑 Generating {} bot identities...", bot_count);
    let generator = IdentityGenerator::new();
    let mut bots: Vec<Box<dyn Bot>> = Vec::new();

    for i in 0..bot_count {
        let bot_id = format!("bot-{}", i);
        let identity = generator.generate(bot_id.clone())?;

        // Create bot based on scenario
        let bot: Box<dyn Bot> = match scenario {
            "alpha-transfer" | "simple-transfer" => {
                Box::new(GeneralUserBot::new(bot_id))
            }
            "delta-trade" | "spot-trade" => {
                Box::new(TraderBot::new(bot_id))
            }
            _ => {
                // Default to general user
                Box::new(GeneralUserBot::new(bot_id))
            }
        };

        bots.push(bot);
    }

    println!("✅ Created {} bots", bots.len());

    // Setup bots
    println!("\n⚙️  Setting up bots...");
    for (i, bot) in bots.iter_mut().enumerate() {
        let identity = generator.generate(format!("bot-{}", i))?;
        let wallet = Wallet::new(format!("bot-{}", i));

        let execution_context = ExecutionContext::new(
            format!("bot-{}", i),
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

    println!("✅ All bots ready");

    // Execute behaviors
    println!("\n🎬 Executing scenario: {}", scenario);

    let start_time = std::time::Instant::now();

    for (i, bot) in bots.iter_mut().enumerate() {
        let behavior_id = match scenario {
            "alpha-transfer" | "simple-transfer" => "transfer",
            "delta-trade" | "spot-trade" => "spot_trade",
            _ => "default",
        };

        match bot.execute_behavior(behavior_id).await {
            Ok(result) => {
                if result.success {
                    print!(".");
                } else {
                    print!("F");
                }
            }
            Err(_) => {
                print!("E");
            }
        }

        if (i + 1) % 50 == 0 {
            println!(" {}/{}", i + 1, bots.len());
        }
    }

    println!();

    let elapsed = start_time.elapsed();

    // Teardown
    println!("\n🧹 Cleaning up...");
    for bot in bots.iter_mut() {
        let _ = bot.teardown().await;
    }

    // Print summary
    println!("\n📈 Summary:");
    println!("   Total bots: {}", bot_count);
    println!("   Duration: {:.2}s", elapsed.as_secs_f64());
    println!("   Ops/bot: 1");
    println!("   Total ops: {}", bot_count);
    println!("   TPS: {:.2}", bot_count as f64 / elapsed.as_secs_f64());

    println!("\n✅ Scenario complete");

    Ok(())
}

async fn show_status(show_workers: bool) -> anyhow::Result<()> {
    println!("📊 adnet-testbots Status\n");

    // TODO: Query coordinator for real status
    println!("Coordinator: Not connected");
    println!("Workers: 0 active, 0 down");
    println!("Total bots: 0");
    println!("TPS: 0.00");

    if show_workers {
        println!("\n💼 Worker Details:");
        println!("   No workers registered");
    }

    Ok(())
}

async fn run_test(test_type: &str) -> anyhow::Result<()> {
    println!("🧪 Running test: {}\n", test_type);

    match test_type {
        "identity" => {
            println!("Testing identity generation...");
            let generator = IdentityGenerator::new();
            let identity = generator.generate("test-bot".to_string())?;

            println!("✅ Identity created:");
            println!("   ID: {}", identity.id);
            println!("   Alpha: {}", identity.alpha_address);
            println!("   Delta: {}", identity.delta_address);
            println!("   Can sign: {}", identity.can_sign());
        }

        "wallet" => {
            println!("Testing wallet operations...");
            let mut wallet = Wallet::new("test-bot".to_string());

            use adnet_testbot::{Balance, Token};

            wallet.credit(Token::AX, Balance::new(1000))?;
            println!("✅ Credited 1000 AX");

            wallet.debit(Token::AX, Balance::new(300))?;
            println!("✅ Debited 300 AX");

            println!("   Final balance: {} AX", wallet.balance(&Token::AX));
        }

        "simple-transfer" => {
            println!("Testing simple bot lifecycle...");
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
            println!("✅ Bot setup complete");

            let result = bot.execute_behavior("transfer").await?;
            println!("✅ Behavior executed: {}", result.message);

            bot.teardown().await?;
            println!("✅ Bot teardown complete");
        }

        _ => {
            println!("❌ Unknown test type: {}", test_type);
            println!("   Available: identity, wallet, simple-transfer");
        }
    }

    println!("\n✅ Test complete");

    Ok(())
}
