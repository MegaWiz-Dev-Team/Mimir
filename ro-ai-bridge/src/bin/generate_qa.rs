use anyhow::Result;
use dotenvy::dotenv;
use rig::providers::openai;
use ro_ai_bridge::agents::wiki_workshop::{
    generator::generate_qa,
    extractor::extract_acus,
    verifier::verify_coverage,
    WikiChunk, QAPair, AtomicFact, CoverageReport
};
use std::env;
use tokio::fs;
use tracing::{info, warn, error};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    // 1. Configure Agents
    // Local LLM (Ollama)
    let ollama_url = env::var("OLLAMA_API_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let local_model = env::var("LOCAL_MODEL").unwrap_or_else(|_| "gemma:2b".to_string());
    
    let local_client = openai::Client::from_url(&ollama_url, "ollama"); // 'ollama' as api key is dummy

    // Cloud LLM (Gemini via OpenAI-compatible endpoint or just reused local for dev if getting keys is hard)
    // Note: In detailed implementation, we would use a proper Gemini provider or a proxy like LiteLLM/OpenRouter.
    // For this proof-of-concept, accessing Gemini might require a specific Rig provider crate not currently in Cargo.toml.
    // We will assume an OpenAI-compatible endpoint for Gemini (e.g. via OpenRouter or a bridge) is provided in env,
    // OR just use the local client if GEMINI_API_KEY is missing (fallback).
    
    // Check if we have Gemini config, else fallback to Local
    let gemini_client = if let Ok(base_url) = env::var("GEMINI_BASE_URL") {
        let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| "missing-key".to_string());
        info!("☁️ configuring Gemini Client via {}", base_url);
        openai::Client::from_url(&base_url, &api_key)
    } else {
        warn!("⚠️ GEMINI_BASE_URL not set. Falling back to Local Client for all agents.");
        local_client.clone()
    };

    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());

    // 2. Scan Data
    let input_dir = "data/wiki";
    let output_file = "data/qa_dataset.json";
    let report_file = "data/qa_evaluation_report.json";

    let mut dir = fs::read_dir(input_dir).await?;
    
    // Prepare outputs
    let mut all_reports = Vec::new();
    let mut all_qa = Vec::new();

    info!("🚀 Starting Multi-Agent Q/A Pipeline...");
    
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

                info!("   🧩 Chunk {}: Genererating Q/A...", i);

                // 4. Agent 1: Generate Q/A (Local)
                let qa_pairs = match generate_qa(&local_client, &local_model, &wiki_chunk).await {
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
