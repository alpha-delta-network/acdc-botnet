/// CLI interface for adnet-testbots
// Placeholder for Phase 1 task #7

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "adnet-testbots")]
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
    },

    /// Show status
    Status {
        /// Show worker details
        #[arg(long)]
        show_workers: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { scenario, distributed } => {
            println!("Running scenario: {} (distributed: {})", scenario, distributed);
            // TODO: Implement scenario runner
        }
        Commands::Coordinator { bind } => {
            println!("Starting coordinator on: {}", bind);
            // TODO: Implement coordinator
        }
        Commands::Worker { coordinator, max_bots } => {
            println!("Starting worker (coordinator: {}, max_bots: {})", coordinator, max_bots);
            // TODO: Implement worker
        }
        Commands::Status { show_workers } => {
            println!("Status (show_workers: {})", show_workers);
            // TODO: Implement status
        }
    }

    Ok(())
}
