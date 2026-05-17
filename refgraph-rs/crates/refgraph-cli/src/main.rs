//! RefGraph CLI: 100% Rust insurance data pipeline.
//!
//! Commands:
//!   scrape         Fetch insurance URLs → chunks JSONL
//!   extract        Chunks JSONL → entities JSONL (refgraph-core)
//!   ingest         Chunks JSONL → Mimir API
//!   test-hit-rate  Run 10 standard queries against /api/search
//!   pipeline       Run scrape → extract → ingest → test-hit-rate

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod scrape;
mod extract;
mod ingest;
mod hit_rate;
mod types;

#[derive(Parser)]
#[command(name = "refgraph", version, about = "RefGraph insurance pipeline (100% Rust)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scrape insurance URLs → chunks JSONL
    Scrape {
        /// Path to insurer_urls.json config
        #[arg(long, default_value = "config/insurer_urls.json")]
        config: PathBuf,
        /// Output chunks JSONL
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Extract entities from chunks JSONL → entities JSONL
    Extract {
        /// Input chunks JSONL
        #[arg(long, short)]
        input: PathBuf,
        /// Output entities JSONL
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Ingest chunks JSONL into Mimir
    Ingest {
        /// Input chunks JSONL
        #[arg(long, short)]
        input: PathBuf,
        /// Mimir API URL
        #[arg(long, default_value = "http://localhost:8080")]
        mimir_url: String,
        /// Tenant ID
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
    },
    /// Run 10 standard queries → Hit Rate@3 report
    TestHitRate {
        /// Mimir API URL
        #[arg(long, default_value = "http://localhost:8080")]
        mimir_url: String,
        /// Tenant ID
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
        /// Top-K cutoff for Hit Rate (default 3)
        #[arg(long, default_value_t = 3)]
        top_k: usize,
    },
    /// Run end-to-end pipeline
    Pipeline {
        #[arg(long, default_value = "config/insurer_urls.json")]
        config: PathBuf,
        #[arg(long, default_value = "data/output")]
        out_dir: PathBuf,
        #[arg(long, default_value = "http://localhost:8080")]
        mimir_url: String,
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Scrape { config, out } => scrape::run(&config, &out).await,
        Command::Extract { input, out } => extract::run(&input, &out),
        Command::Ingest {
            input,
            mimir_url,
            tenant_id,
        } => ingest::run(&input, &mimir_url, &tenant_id).await,
        Command::TestHitRate {
            mimir_url,
            tenant_id,
            top_k,
        } => hit_rate::run(&mimir_url, &tenant_id, top_k).await,
        Command::Pipeline {
            config,
            out_dir,
            mimir_url,
            tenant_id,
        } => {
            let chunks_path = out_dir.join("chunks.jsonl");
            let entities_path = out_dir.join("entities.jsonl");
            std::fs::create_dir_all(&out_dir)?;
            scrape::run(&config, &chunks_path).await?;
            extract::run(&chunks_path, &entities_path)?;
            ingest::run(&chunks_path, &mimir_url, &tenant_id).await?;
            hit_rate::run(&mimir_url, &tenant_id, 3).await?;
            Ok(())
        }
    }
}
