use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hivefuzz")]
#[command(about = "Autonomous distributed fuzzing swarm — leaderless vulnerability discovery")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new fuzzing target
    Init {
        /// Path to the target binary
        #[arg(long)]
        target: String,

        /// Path to seed corpus directory
        #[arg(long)]
        corpus: Option<String>,
    },

    /// Start a hivefuzz node
    Run {
        /// Path to hivefuzz.toml configuration file
        #[arg(long, default_value = "hivefuzz.toml")]
        config: String,

        /// Seed nodes for initial swarm discovery (host:port)
        #[arg(long, value_delimiter = ',')]
        seeds: Vec<String>,

        /// Port to listen on for gossip protocol
        #[arg(long, default_value = "7878")]
        port: u16,

        /// Bind address
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
    },

    /// Start a local development swarm
    Dev {
        /// Number of nodes to spawn
        #[arg(long, default_value = "3")]
        nodes: u32,

        /// Path to hivefuzz.toml configuration file
        #[arg(long, default_value = "hivefuzz.toml")]
        config: String,
    },

    /// Show node/swarm status
    Status {
        /// Node address to query
        #[arg(long, default_value = "127.0.0.1:7878")]
        node: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { target, corpus } => {
            tracing::info!("Initializing target: {}", target);
            hivefuzz::commands::init::run(&target, corpus.as_deref()).await?;
        }
        Commands::Run {
            config,
            seeds,
            port,
            bind,
        } => {
            tracing::info!("Starting hivefuzz node on {}:{}", bind, port);
            hivefuzz::commands::run::run(&config, &seeds, &bind, port).await?;
        }
        Commands::Dev { nodes, config } => {
            tracing::info!("Starting local dev swarm with {} nodes", nodes);
            hivefuzz::commands::dev::run(nodes, &config).await?;
        }
        Commands::Status { node } => {
            hivefuzz::commands::status::run(&node).await?;
        }
    }

    Ok(())
}
