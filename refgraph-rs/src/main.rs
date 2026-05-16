//! RefGraph CLI: Multi-domain data consolidation engine

use refgraph::{RefGraph, ManifestConfig, types::RawChunk};
use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "RefGraph",
    version = "0.1.0",
    author = "Asgard Team",
    about = "Multi-domain data consolidation for Mimir RAG"
)]
struct Args {
    /// Domain (insurance, medical, legal, finance)
    #[arg(short, long, default_value = "insurance")]
    domain: String,

    /// Input JSONL file with raw chunks
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output JSON file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output JSONL file
    #[arg(long)]
    jsonl: Option<PathBuf>,

    /// Manifest config file
    #[arg(short, long)]
    manifest: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Run tests
    #[arg(long)]
    test: bool,
}

#[tokio::main]
async fn main() -> refgraph::Result<()> {
    let args = Args::parse();

    // Setup logging
    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    log::info!("RefGraph v{}", refgraph::VERSION);

    if args.test {
        run_tests().await?;
        return Ok(());
    }

    // Load or create manifest
    let config = if let Some(manifest_path) = args.manifest {
        log::info!("Loading manifest from {}", manifest_path.display());
        ManifestConfig::from_file(manifest_path.to_str().unwrap())?
    } else {
        log::info!("Using default {} manifest", args.domain);
        match args.domain.as_str() {
            "medical" => ManifestConfig::medical(),
            "insurance" => ManifestConfig::insurance(),
            _ => ManifestConfig::default(),
        }
    };

    config.validate()?;
    log::info!("✅ Manifest validated");

    // Load input chunks if provided
    if let Some(input_path) = args.input {
        log::info!("Loading chunks from {}", input_path.display());

        let content = std::fs::read_to_string(&input_path)?;
        let mut chunks = Vec::new();

        for line in content.lines() {
            if let Ok(chunk) = serde_json::from_str::<RawChunk>(line) {
                chunks.push(chunk);
            }
        }

        log::info!("Loaded {} chunks", chunks.len());

        // Run consolidation
        let mut graph = RefGraph::new(config)?;
        log::info!("🚀 Starting consolidation...");

        let output = graph.consolidate(chunks).await?;
        log::info!("✅ Consolidation complete");
        log::info!(
            "📊 Results: {} entities, {} relationships",
            output.metadata.entity_count,
            output.metadata.relationship_count
        );

        // Save outputs
        if let Some(output_path) = args.output {
            output.save_json(output_path.to_str().unwrap())?;
            log::info!("💾 Saved JSON to {}", output_path.display());
        }

        if let Some(jsonl_path) = args.jsonl {
            output.save_jsonl(jsonl_path.to_str().unwrap())?;
            log::info!("💾 Saved JSONL to {}", jsonl_path.display());
        }
    } else {
        println!("Usage: refgraph --input <file.jsonl> --output <file.json> [--jsonl <file.jsonl>]");
        println!("\nExample:");
        println!("  refgraph --domain insurance --input raw_chunks.jsonl --output consolidated.json");
    }

    Ok(())
}

async fn run_tests() -> refgraph::Result<()> {
    log::info!("Running integration tests...");

    // Test 1: Create RefGraph with default config
    log::info!("Test 1: Creating RefGraph with insurance config...");
    let config = ManifestConfig::insurance();
    let _graph = RefGraph::new(config)?;
    log::info!("✅ Test 1 passed");

    // Test 2: Test with empty chunks
    log::info!("Test 2: Consolidating empty chunks...");
    let mut graph = RefGraph::new(ManifestConfig::insurance())?;
    let result = graph.consolidate(vec![]).await;
    assert!(result.is_ok());
    log::info!("✅ Test 2 passed");

    // Test 3: Test with sample chunks
    log::info!("Test 3: Consolidating sample chunks...");
    let mut graph = RefGraph::new(ManifestConfig::insurance())?;
    let chunks = vec![
        RawChunk {
            chunk_id: "chunk_001".to_string(),
            content: "PRU Critical Illness covers Heart Attack and Stroke".to_string(),
            source_url: "prudential.co.th/product".to_string(),
            page_index: Some(1),
            token_count: 10,
        },
        RawChunk {
            chunk_id: "chunk_002".to_string(),
            content: "Pre-existing conditions are excluded".to_string(),
            source_url: "prudential.co.th/terms".to_string(),
            page_index: Some(2),
            token_count: 6,
        },
    ];

    match graph.consolidate(chunks).await {
        Ok(output) => {
            log::info!(
                "✅ Test 3 passed - {} entities, {} relationships",
                output.metadata.entity_count,
                output.metadata.relationship_count
            );
        }
        Err(e) => {
            log::error!("❌ Test 3 failed: {}", e);
            return Err(e);
        }
    }

    log::info!("✅ All integration tests passed!");
    Ok(())
}

