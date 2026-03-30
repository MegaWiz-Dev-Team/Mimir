use anyhow::Result;
use dotenvy::dotenv;
use rig::providers::{ollama, gemini};
use mimir_core_ai::qa_qc::{
    generator::generate_qa,
    extractor::extract_acus,
    verifier::verify_coverage,
    WikiChunk,
};
use mimir_core_ai::services::llm_router::UniversalClient;
use mimir_core_ai::config::QAConfig;
use std::env;
use tokio::fs;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    // 1. Configure Agents
    // Local LLM (Ollama)
    info!("🤖 Configuring Native Ollama Client (defaulting to localhost:11434)");
    let local_client = ollama::Client::new();
    let local_model = env::var("LOCAL_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string());

    // Cloud LLM (Gemini)
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set in .env");
    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());
    
    info!("☁️ Configuring Native Gemini Client");
    let gemini_client = UniversalClient::Gemini(gemini::Client::new(&api_key));

    // Generator Configuration
    let gen_provider = env::var("GENERATOR_PROVIDER").unwrap_or_else(|_| "ollama".to_string());
    let (gen_client, gen_model) = match gen_provider.as_str() {
        "gemini" => {
            info!("⚙️ Generator Provider: GEMINI ({})", gemini_model);
            (gemini_client.clone(), gemini_model.clone())
        },
        _ => {
            info!("⚙️ Generator Provider: OLLAMA ({})", local_model);
            (UniversalClient::Ollama(local_client.clone()), local_model.clone())
        }
    };

    // 2. Scan Data
    let input_dir = "data/wiki";
    let output_file = "data/qa_dataset.json";
    let report_file = "data/qa_evaluation_report.json";

    let mut dir = fs::read_dir(input_dir).await?;
    
    // Prepare outputs
    let mut all_reports = Vec::new();
    let mut all_qa = Vec::new();

    info!("🚀 Starting Multi-Agent Q/A Pipeline...");
    
    let is_test_run = env::var("TEST_RUN").unwrap_or_default() == "1";
    
    // Load Q/A config (from file or use defaults)
    let config_path = env::var("QA_CONFIG_PATH").unwrap_or_else(|_| "data/qa_config.json".to_string());
    let qa_config = QAConfig::from_file_or_default(&config_path);
    info!("📋 Q/A Config loaded: default_count={}, {} size rules, {} file patterns", 
        qa_config.default_count, 
        qa_config.rules.len(), 
        qa_config.file_patterns.patterns.len());

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            info!("📂 Processing: {}", filename);

            let content_raw = fs::read_to_string(&path).await?;
            
            // Extract Frontmatter
            let (url, content) = if content_raw.starts_with("---") {
                 if let Some(end) = content_raw[3..].find("---") {
                     let frontmatter = &content_raw[3..end+3];
                     // Simple grep for url
                     let url = frontmatter.lines()
                         .find(|l| l.trim().starts_with("url:"))
                         .map(|l| l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').to_string())
                         .unwrap_or_else(|| "unknown".to_string());
                     
                     (url, &content_raw[end+6..]) // +6 to skip --- and \n\n usually
                 } else {
                     ("unknown".to_string(), content_raw.as_str())
                 }
            } else {
                ("unknown".to_string(), content_raw.as_str())
            };


            // 3. Chunking (Simple Split by Header for now)
            // A smarter chunker would be another agent, but regex/split is faster/cheaper.
            let chunks: Vec<&str> = content.split("\n#").collect(); // Rough split

            for (i, raw_chunk) in chunks.iter().enumerate() {
                if raw_chunk.trim().len() < 50 { continue; } // Skip empty/tiny chunks

                // Restore header marker if needed or just process content
                let chunk_text = if i > 0 { format!("#{}", raw_chunk) } else { raw_chunk.to_string() };
                
                let wiki_chunk = WikiChunk {
                    source_file: filename.clone(),
                    url: url.clone(),
                    content: chunk_text.replace("\n", " "), // Flatten for easier processing
                };

                // Determine Q/A count based on file name and content size
                let qa_count = qa_config.get_qa_count(&filename, wiki_chunk.content.len());
                info!("   🧩 Chunk {}: Generating {} Q/A pairs...", i, qa_count);

                // 4. Agent 1: Generate Q/A
                let qa_pairs = match generate_qa(&gen_client, &gen_model, &wiki_chunk, qa_count).await {
                    Ok(pairs) => pairs,
                    Err(e) => {
                        error!("   ❌ Q/A Generation failed: {}", e);
                        continue;
                    }
                };
                
                if qa_pairs.is_empty() {
                    warn!("   ⚠️ No Q/A generated for chunk {}", i);
                    continue;
                }

                info!("   ✅ Generated {} pairs. Extracting ACUs (Gemini)...", qa_pairs.len());

                // 5. Agent 2: Extract ACUs (Gemini)
                let facts = match extract_acus(&gemini_client, &gemini_model, &wiki_chunk).await {
                    Ok(f) => f,
                    Err(e) => {
                        error!("   ❌ ACU Extraction failed: {}", e);
                        continue;
                    }
                };

                info!("   🔍 Found {} Atomic Facts. Verifying Coverage...", facts.len());

                // 6. Agent 3: Verify Coverage (Gemini)
                let report = match verify_coverage(&gemini_client, &gemini_model, &wiki_chunk, &facts, &qa_pairs).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!("   ❌ Verification failed: {}", e);
                        error!("   ❌ Verification failed for file '{}', chunk {}: {}", filename, i, e);
                        continue;
                    }
                };

                info!("   🎯 Coverage: {:.1}% | Missing: {}", report.coverage_score, report.missing_facts.len());

                // Store Result
                // Add metadata to QA pairs?
                for qa in &qa_pairs {
                    // We might want to store more metadata like chunk_id/file
                    // For now, keep it simple conform to struct
                }
                
                all_qa.extend(qa_pairs);
                all_reports.push(report);
            }

            if is_test_run {
                info!("🧪 TEST_RUN enabled. Stopping after first file: {}", filename);
                break;
            }
        }
    }

    // Save outputs
    let qa_json = serde_json::to_string_pretty(&all_qa)?;
    fs::write(output_file, qa_json).await?;
    
    let report_json = serde_json::to_string_pretty(&all_reports)?;
    fs::write(report_file, report_json).await?;

    info!("✨ Pipeline Complete!");
    info!("   📝 Dataset: {} pairs saved to {}", all_qa.len(), output_file);
    info!("   📊 Report: Saved to {}", report_file);

    Ok(())
}
