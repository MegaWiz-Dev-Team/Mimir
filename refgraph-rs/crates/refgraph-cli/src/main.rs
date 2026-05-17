//! RefGraph CLI: 100% Rust insurance data pipeline.
//!
//! Commands:
//!   scrape         Fetch insurance URLs → chunks JSONL
//!   extract        Chunks JSONL → entities JSONL (refgraph-core)
//!   ingest         Chunks JSONL → Heimdall embed + Qdrant upsert (direct,
//!                  bypasses buggy mimir-api embed-chunks endpoint)
//!   test-hit-rate  Run 10 standard queries against /api/v1/search
//!   pipeline       Run scrape → extract → ingest → test-hit-rate

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod extract;
mod heimdall;
mod hit_rate;
mod hit_rate_direct;
mod ingest;
mod qdrant;
mod scrape;
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
        #[arg(long, default_value = "config/insurer_urls.json")]
        config: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Extract entities from chunks JSONL → entities JSONL (refgraph-core)
    Extract {
        #[arg(long, short)]
        input: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Ingest chunks JSONL → Heimdall embeddings → Qdrant upsert (direct).
    ///
    /// Bypasses the buggy mimir-api `/vector/embed-chunks` endpoint
    /// (filters on a non-existent `chunks.tenant_id` column AND uses a
    /// hash-based pseudo-embedder). This command goes direct so
    /// `/api/v1/search` sees real BGE-M3 vectors.
    Ingest {
        /// Input chunks JSONL (one chunk per line — phase1 or scrape output)
        #[arg(long, short)]
        input: PathBuf,
        /// Heimdall gateway URL
        #[arg(long, default_value = "http://localhost:8080", env = "HEIMDALL_URL")]
        heimdall_url: String,
        /// Heimdall API key (Bearer). Default to local launchd value.
        #[arg(long, env = "HEIMDALL_API_KEY")]
        heimdall_key: Option<String>,
        /// Embedding model id
        #[arg(long, default_value = "bge-m3")]
        embed_model: String,
        /// Qdrant URL
        #[arg(long, default_value = "http://localhost:6333", env = "QDRANT_URL")]
        qdrant_url: String,
        /// Qdrant collection (matches what /api/v1/search reads)
        #[arg(long, default_value = "source_chunks")]
        collection: String,
        /// Tenant ID stored in payload + used by /api/v1/search filters
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
    },
    /// Run 10 standard queries → Hit Rate@3 against Qdrant DIRECTLY.
    ///
    /// Bypasses mimir-api `/api/v1/search` because that route's query
    /// embedder (`embed_texts` in routes/vector.rs) generates fake hash
    /// vectors instead of calling Heimdall. This command embeds the query
    /// via Heimdall (real BGE-M3) and searches Qdrant directly, giving
    /// the baseline number that *would* be reported if the Mimir bug
    /// were fixed.
    TestHitRateDirect {
        #[arg(long, default_value = "http://localhost:8080", env = "HEIMDALL_URL")]
        heimdall_url: String,
        #[arg(long, env = "HEIMDALL_API_KEY")]
        heimdall_key: Option<String>,
        #[arg(long, default_value = "bge-m3")]
        embed_model: String,
        #[arg(long, default_value = "http://localhost:6333", env = "QDRANT_URL")]
        qdrant_url: String,
        #[arg(long, default_value = "source_chunks")]
        collection: String,
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
        #[arg(long, default_value_t = 3)]
        top_k: usize,
    },
    /// Run 10 standard queries → Hit Rate@3 report against /api/v1/search.
    TestHitRate {
        #[arg(long, default_value = "http://localhost:8090", env = "MIMIR_URL")]
        mimir_url: String,
        #[arg(long, default_value = "asgard_insurance")]
        tenant_id: String,
        #[arg(long, default_value_t = 3)]
        top_k: usize,
    },
    /// Run end-to-end: scrape → extract → ingest → test-hit-rate
    Pipeline {
        #[arg(long, default_value = "config/insurer_urls.json")]
        config: PathBuf,
        #[arg(long, default_value = "data/output")]
        out_dir: PathBuf,
        #[arg(long, default_value = "http://localhost:8080")]
        heimdall_url: String,
        #[arg(long, env = "HEIMDALL_API_KEY")]
        heimdall_key: Option<String>,
        #[arg(long, default_value = "http://localhost:6333")]
        qdrant_url: String,
        #[arg(long, default_value = "http://localhost:8090")]
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
            heimdall_url,
            heimdall_key,
            embed_model,
            qdrant_url,
            collection,
            tenant_id,
        } => {
            let api_key = heimdall_key
                .ok_or_else(|| anyhow::anyhow!("--heimdall-key or HEIMDALL_API_KEY env required"))?;
            let cfg = ingest::IngestConfig {
                heimdall_url,
                heimdall_api_key: api_key,
                heimdall_model: embed_model,
                qdrant_url,
                qdrant_collection: collection,
                tenant_id,
                ..Default::default()
            };
            ingest::run(&input, cfg).await
        }
        Command::TestHitRate {
            mimir_url,
            tenant_id,
            top_k,
        } => hit_rate::run(&mimir_url, &tenant_id, top_k).await,
        Command::TestHitRateDirect {
            heimdall_url,
            heimdall_key,
            embed_model,
            qdrant_url,
            collection,
            tenant_id,
            top_k,
        } => {
            let api_key = heimdall_key
                .ok_or_else(|| anyhow::anyhow!("--heimdall-key or HEIMDALL_API_KEY env required"))?;
            let cfg = hit_rate_direct::DirectConfig {
                heimdall_url,
                heimdall_api_key: api_key,
                heimdall_model: embed_model,
                qdrant_url,
                qdrant_collection: collection,
                tenant_id,
            };
            hit_rate_direct::run(cfg, top_k).await
        }
        Command::Pipeline {
            config,
            out_dir,
            heimdall_url,
            heimdall_key,
            qdrant_url,
            mimir_url,
            tenant_id,
        } => {
            let api_key = heimdall_key
                .ok_or_else(|| anyhow::anyhow!("--heimdall-key or HEIMDALL_API_KEY env required"))?;
            let chunks_path = out_dir.join("chunks.jsonl");
            let entities_path = out_dir.join("entities.jsonl");
            std::fs::create_dir_all(&out_dir)?;
            scrape::run(&config, &chunks_path).await?;
            extract::run(&chunks_path, &entities_path)?;
            let cfg = ingest::IngestConfig {
                heimdall_url,
                heimdall_api_key: api_key,
                qdrant_url,
                tenant_id: tenant_id.clone(),
                ..Default::default()
            };
            ingest::run(&chunks_path, cfg).await?;
            hit_rate::run(&mimir_url, &tenant_id, 3).await?;
            Ok(())
        }
    }
}
