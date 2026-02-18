use anyhow::Result;
use rig::providers::{ollama, gemini};
use super::{WikiChunk, QAPair, AtomicFact, CoverageReport};
use super::generator::{generate_qa, GeneratorClient};
use super::extractor::extract_acus;
use super::verifier::verify_coverage;
use crate::services::db::DbPool;
use std::env;
use tokio::fs;
use tracing::{info, warn, error};
use uuid::Uuid;
use chrono::Utc;
use sqlx::Row;

pub async fn run_pipeline(
    db_pool: &DbPool,
    run_id: String,
    provider: &str,
    model: &str,
    input_dir: &str,
    is_test_run: bool
) -> Result<()> {
    let started_at = Utc::now();

    // 1. Initialize Run in DB
    sqlx::query(
        "INSERT INTO pipeline_runs (id, status, provider, model, started_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&run_id)
    .bind("RUNNING")
    .bind(provider)
    .bind(model)
    .bind(started_at)
    .execute(db_pool).await?;

    info!("🚀 Starting Pipeline Run: {}", run_id);

    // Clients
    let local_client = ollama::Client::new();
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    let gemini_client = gemini::Client::new(&api_key);
    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

    let gen_client = match provider {
        "gemini" => GeneratorClient::Gemini(gemini_client.clone()),
        _ => GeneratorClient::Ollama(local_client.clone()),
    };

    let mut all_qa = Vec::new();

    let mut dir = fs::read_dir(input_dir).await?;
    
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content_raw = fs::read_to_string(&path).await?;
            
            // Extract Frontmatter
            let (url, content) = if content_raw.starts_with("---") {
                 if let Some(end) = content_raw[3..].find("---") {
                     let frontmatter = &content_raw[3..end+3];
                     let url = frontmatter.lines()
                         .find(|l| l.trim().starts_with("url:"))
                         .map(|l| l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').to_string())
                         .unwrap_or_else(|| "unknown".to_string());
                     (url, &content_raw[end+6..])
                 } else { ("unknown".to_string(), content_raw.as_str()) }
            } else { ("unknown".to_string(), content_raw.as_str()) };

            let chunks: Vec<&str> = content.split("\n#").collect();

            for (i, raw_chunk) in chunks.iter().enumerate() {
                if raw_chunk.trim().len() < 50 { continue; }
                let chunk_text = if i > 0 { format!("#{}", raw_chunk) } else { raw_chunk.to_string() };
                
                let wiki_chunk = WikiChunk {
                    source_file: filename.clone(),
                    url: url.clone(),
                    content: chunk_text.replace("\n", " "),
                };

                // Create Step in DB
                let step_id = sqlx::query(
                    "INSERT INTO pipeline_steps (run_id, file_name, chunk_index, status, step_type, started_at) \
                     VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(&run_id)
                .bind(&filename)
                .bind(i as i64)
                .bind("RUNNING")
                .bind("FULL_PROCESS")
                .bind(Utc::now())
                .execute(db_pool).await?.last_insert_id();

                // Process
                match process_chunk(db_pool, step_id, &gen_client, model, &gemini_client, &gemini_model, &wiki_chunk).await {
                    Ok(qa_pairs) => {
                        all_qa.extend(qa_pairs);
                        sqlx::query(
                            "UPDATE pipeline_steps SET status = ?, finished_at = ? WHERE id = ?"
                        )
                        .bind("COMPLETED")
                        .bind(Utc::now())
                        .bind(step_id as i64)
                        .execute(db_pool).await?;
                    },
                    Err(e) => {
                        let err_msg = e.to_string();
                        error!("Step failed: {}", err_msg);
                        sqlx::query(
                            "UPDATE pipeline_steps SET status = ?, finished_at = ?, error_message = ? WHERE id = ?"
                        )
                        .bind("FAILED")
                        .bind(Utc::now())
                        .bind(err_msg)
                        .bind(step_id as i64)
                        .execute(db_pool).await?;
                    }
                }
            }

            if is_test_run { break; }
        }
    }

    // Finalize Run
    sqlx::query(
        "UPDATE pipeline_runs SET status = ?, finished_at = ? WHERE id = ?"
    )
    .bind("COMPLETED")
    .bind(Utc::now())
    .bind(&run_id)
    .execute(db_pool).await?;

    Ok(())
}

pub async fn retry_step(
    db_pool: &DbPool,
    step_id: i64,
) -> Result<()> {
    // 1. Fetch Step Details
    let step = sqlx::query("SELECT * FROM pipeline_steps WHERE id = ?")
        .bind(step_id)
        .fetch_one(db_pool)
        .await?;
    
    let run_id: String = step.get("run_id");
    let file_name: String = step.get("file_name");
    let chunk_index: i64 = step.get("chunk_index");

    // 2. Fetch Run Details
    let run = sqlx::query("SELECT * FROM pipeline_runs WHERE id = ?")
        .bind(&run_id)
        .fetch_one(db_pool)
        .await?;

    let provider: String = run.get("provider");
    let model: String = run.get("model");
    
    // 3. Re-read Content (Assuming data/wiki)
    let input_dir = "data/wiki"; 
    let path = std::path::Path::new(input_dir).join(&file_name);
    let content_raw = fs::read_to_string(&path).await?;
    
    // Extract Frontmatter (Duplicate logic from run_pipeline - ideally refactor)
    let (url, content) = if content_raw.starts_with("---") {
         if let Some(end) = content_raw[3..].find("---") {
             let frontmatter = &content_raw[3..end+3];
             let url_val = frontmatter.lines()
                 .find(|l| l.trim().starts_with("url:"))
                 .map(|l| l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').to_string())
                 .unwrap_or_else(|| "unknown".to_string());
             (url_val, &content_raw[end+6..])
         } else { ("unknown".to_string(), content_raw.as_str()) }
    } else { ("unknown".to_string(), content_raw.as_str()) };

    let chunks: Vec<&str> = content.split("\n#").collect();
    if chunk_index as usize >= chunks.len() {
        return Err(anyhow::anyhow!("Chunk index out of bounds"));
    }

    let raw_chunk = chunks[chunk_index as usize];
    let chunk_text = if chunk_index > 0 { format!("#{}", raw_chunk) } else { raw_chunk.to_string() };
    
    let wiki_chunk = WikiChunk {
        source_file: file_name.clone(),
        url: url.clone(),
        content: chunk_text.replace("\n", " "),
    };

    // 3.5 Cleanup Previous Results
    sqlx::query("DELETE FROM qa_results WHERE step_id = ?")
        .bind(step_id)
        .execute(db_pool).await?;

    sqlx::query("DELETE FROM evaluation_reports WHERE step_id = ?")
        .bind(step_id)
        .execute(db_pool).await?;

    // 4. Update Status to RUNNING
    sqlx::query("UPDATE pipeline_steps SET status = 'RUNNING', error_message = NULL, started_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(step_id)
        .execute(db_pool).await?;

    // 5. Initialize Clients
    let local_client = ollama::Client::new();
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    let gemini_client = gemini::Client::new(&api_key);
    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

    let gen_client = match provider.as_str() {
        "gemini" => GeneratorClient::Gemini(gemini_client.clone()),
        _ => GeneratorClient::Ollama(local_client.clone()),
    };

    // 6. Process Chunk
    info!("🔄 Retrying Step #{} (File: {}, Chunk: {})", step_id, file_name, chunk_index);
    
    match process_chunk(db_pool, step_id as u64, &gen_client, &model, &gemini_client, &gemini_model, &wiki_chunk).await {
        Ok(_) => {
            sqlx::query("UPDATE pipeline_steps SET status = 'COMPLETED', finished_at = ? WHERE id = ?")
                .bind(Utc::now())
                .bind(step_id)
                .execute(db_pool).await?;
            info!("✅ Retry Step #{} Completed", step_id);
        },
        Err(e) => {
            let err_msg = e.to_string();
            error!("❌ Retry Step #{} Failed: {}", step_id, err_msg);
            sqlx::query("UPDATE pipeline_steps SET status = 'FAILED', finished_at = ?, error_message = ? WHERE id = ?")
                .bind(Utc::now())
                .bind(err_msg)
                .bind(step_id)
                .execute(db_pool).await?;
        }
    }

    Ok(())
}

pub async fn process_chunk(
    db_pool: &DbPool,
    step_id: u64,
    gen_client: &GeneratorClient,
    gen_model: &str,
    gemini_client: &gemini::Client,
    gemini_model: &str,
    chunk: &WikiChunk
) -> Result<Vec<QAPair>> {
    // 1. Generate Q/A
    let qa_pairs = generate_qa(gen_client, gen_model, chunk).await?;
    
    // Save Q/A to DB
    for qa in &qa_pairs {
        sqlx::query(
            "INSERT INTO qa_results (step_id, question, answer) VALUES (?, ?, ?)"
        )
        .bind(step_id as i64)
        .bind(&qa.question)
        .bind(&qa.answer)
        .execute(db_pool).await?;
    }

    // 2. Extract ACUs
    let facts = extract_acus(gemini_client, gemini_model, chunk).await?;

    // 3. Verify Coverage
    let report = verify_coverage(gemini_client, gemini_model, chunk, &facts, &qa_pairs).await?;

    // Save Report to DB
    let facts_json = serde_json::to_string(&facts)?;
    let missing_json = serde_json::to_string(&report.missing_facts)?;
    
    sqlx::query(
        "INSERT INTO evaluation_reports (step_id, coverage_score, atomic_facts, missing_facts, reasoning) \
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(step_id as i64)
    .bind(report.coverage_score)
    .bind(facts_json)
    .bind(missing_json)
    .bind(report.reasoning)
    .execute(db_pool).await?;

    Ok(qa_pairs)
}
