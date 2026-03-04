use anyhow::Result;
use rig::providers::{ollama, gemini};
use super::{WikiChunk, QAPair};
use super::generator::{generate_qa, generate_missing_qa, GeneratorClient};
use super::extractor::extract_acus;
use super::verifier::verify_coverage;
use crate::services::db::DbPool;
use crate::services::iam::IamService;
use crate::config::QAConfig;
use std::env;
use tokio::fs;
use tracing::{info, error};
use chrono::Utc;
use sqlx::Row;

/// Resolve Heimdall URL/key from tenant config, falling back to env vars.
async fn resolve_heimdall_config(db_pool: &DbPool, tenant_id: &str) -> (String, String) {
    let iam = IamService::new_with_env(db_pool.clone());
    if let Ok(tc) = iam.get_tenant_config(tenant_id).await {
        let llm = tc.llm_config.as_ref().map(|c| &c.0);
        let url = llm.and_then(|c| c.heimdall_url.clone())
            .or_else(|| env::var("HEIMDALL_API_URL").ok())
            .unwrap_or_else(|| "http://192.168.1.133:3000/v1".to_string());
        let key = llm.and_then(|c| c.heimdall_api_key.clone())
            .or_else(|| env::var("HEIMDALL_API_KEY").ok())
            .unwrap_or_else(|| "heimdall-key".to_string());
        (url, key)
    } else {
        let url = env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://192.168.1.133:3000/v1".to_string());
        let key = env::var("HEIMDALL_API_KEY").unwrap_or_else(|_| "heimdall-key".to_string());
        (url, key)
    }
}

pub async fn run_pipeline(
    db_pool: &DbPool,
    run_id: String,
    provider: &str,
    model: &str,
    input_dir: &str,
    is_test_run: bool,
    qa_count: usize,
    tenant_id: String
) -> Result<()> {
    // Convert fixed qa_count to a QAConfig with default rules
    let qa_config = QAConfig {
        default_count: qa_count,
        rules: vec![],
        file_patterns: crate::config::FilePatternConfig {
            comment: None,
            patterns: vec![],
        },
    };
    run_pipeline_with_config(db_pool, run_id, provider, model, input_dir, is_test_run, qa_config, tenant_id).await
}

/// Run pipeline with full QAConfig support (size-based and pattern-based rules)
pub async fn run_pipeline_with_config(
    db_pool: &DbPool,
    run_id: String,
    provider: &str,
    model: &str,
    input_dir: &str,
    is_test_run: bool,
    qa_config: QAConfig,
    tenant_id: String
) -> Result<()> {
    let started_at = Utc::now();

    // 1. Initialize Run in DB
    sqlx::query(
        "INSERT INTO pipeline_runs (id, status, provider, model, started_at, tenant_id) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&run_id)
    .bind("RUNNING")
    .bind(provider)
    .bind(model)
    .bind(started_at)
    .bind(&tenant_id)
    .execute(db_pool).await?;

    info!("🚀 Starting Pipeline Run: {}", run_id);
    info!("📋 QA Config: default_count={}, {} size rules, {} file patterns", 
        qa_config.default_count, qa_config.rules.len(), qa_config.file_patterns.patterns.len());

    // Clients
    let local_client = ollama::Client::new();
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    let gemini_client = gemini::Client::new(&api_key);
    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());

    let gen_client = match provider {
        "gemini" | "google" => GeneratorClient::Gemini(gemini_client.clone()),
        "heimdall" => {
            let (hd_endpoint, hd_api_key) = resolve_heimdall_config(db_pool, &tenant_id).await;
            GeneratorClient::Heimdall {
                client: reqwest::Client::new(),
                endpoint: hd_endpoint,
                api_key: hd_api_key,
            }
        }
        _ => GeneratorClient::Ollama(local_client.clone()),
    };

    let mut all_qa = Vec::new();
    let mut total_steps = 0usize;
    let mut failed_steps = 0usize;

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

                // Calculate Q/A count based on file name and content size
                let qa_count = qa_config.get_qa_count(&filename, wiki_chunk.content.len());

                // Create Step in DB
                total_steps += 1;
                let step_id = sqlx::query(
                    "INSERT INTO pipeline_steps (run_id, file_name, chunk_index, status, step_type, started_at, tenant_id) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(&run_id)
                .bind(&filename)
                .bind(i as i64)
                .bind("RUNNING")
                .bind("FULL_PROCESS")
                .bind(Utc::now())
                .bind(&tenant_id)
                .execute(db_pool).await?.last_insert_id();

                // Process
                match process_chunk(db_pool, step_id, &gen_client, model, &gemini_client, &gemini_model, &wiki_chunk, qa_count, &tenant_id).await {
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
                        failed_steps += 1;
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

    // Finalize Run - set status based on step results
    let final_status = if failed_steps == 0 {
        "COMPLETED"
    } else if failed_steps == total_steps {
        "FAILED"
    } else {
        "PARTIAL"
    };
    
    info!("📊 Pipeline finished: {} total steps, {} failed, status={}", total_steps, failed_steps, final_status);
    
    sqlx::query(
        "UPDATE pipeline_runs SET status = ?, finished_at = ? WHERE id = ?"
    )
    .bind(final_status)
    .bind(Utc::now())
    .bind(&run_id)
    .execute(db_pool).await?;

    Ok(())
}

pub async fn retry_step(
    db_pool: &DbPool,
    step_id: i64,
    qa_count: usize
) -> Result<()> {
    // Convert to QAConfig
    let qa_config = QAConfig {
        default_count: qa_count,
        rules: vec![],
        file_patterns: crate::config::FilePatternConfig {
            comment: None,
            patterns: vec![],
        },
    };
    retry_step_with_config(db_pool, step_id, qa_config).await
}

/// Retry step with full QAConfig support
pub async fn retry_step_with_config(
    db_pool: &DbPool,
    step_id: i64,
    qa_config: QAConfig
) -> Result<()> {
    // 1. Fetch Step Details
    let step = sqlx::query("SELECT * FROM pipeline_steps WHERE id = ?")
        .bind(step_id)
        .fetch_one(db_pool)
        .await?;
    
    let run_id: String = step.get("run_id");
    let file_name: String = step.get("file_name");
    let chunk_index: i64 = step.get("chunk_index");
    let tenant_id: String = step.get("tenant_id");

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

    // Calculate Q/A count based on file name and content size
    let qa_count = qa_config.get_qa_count(&file_name, wiki_chunk.content.len());

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
    let gemini_model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());

    let gen_client = match provider.as_str() {
        "gemini" | "google" => GeneratorClient::Gemini(gemini_client.clone()),
        "heimdall" => {
            let (hd_endpoint, hd_api_key) = resolve_heimdall_config(db_pool, &tenant_id).await;
            GeneratorClient::Heimdall {
                client: reqwest::Client::new(),
                endpoint: hd_endpoint,
                api_key: hd_api_key,
            }
        }
        _ => GeneratorClient::Ollama(local_client.clone()),
    };

    // 6. Process Chunk
    info!("🔄 Retrying Step #{} (File: {}, Chunk: {}) with {} Q/A pairs", step_id, file_name, chunk_index, qa_count);
    
    match process_chunk(db_pool, step_id as u64, &gen_client, &model, &gemini_client, &gemini_model, &wiki_chunk, qa_count, &tenant_id).await {
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

pub async fn resume_pipeline(
    db_pool: &DbPool,
    run_id: String,
    qa_count: usize
) -> Result<()> {
    // Convert to QAConfig
    let qa_config = QAConfig {
        default_count: qa_count,
        rules: vec![],
        file_patterns: crate::config::FilePatternConfig {
            comment: None,
            patterns: vec![],
        },
    };
    resume_pipeline_with_config(db_pool, run_id, qa_config).await
}

/// Resume pipeline with full QAConfig support
pub async fn resume_pipeline_with_config(
    db_pool: &DbPool,
    run_id: String,
    qa_config: QAConfig
) -> Result<()> {
    // 1. Check/Update Run Status
    let run = sqlx::query("SELECT status, tenant_id FROM pipeline_runs WHERE id = ?")
        .bind(&run_id)
        .fetch_one(db_pool).await?;
        
    let status: String = run.get("status");
    let tenant_id: String = run.get("tenant_id");
    if status == "COMPLETED" {
        info!("Run {} is already COMPLETED.", run_id);
        return Ok(());
    }

    sqlx::query("UPDATE pipeline_runs SET status = 'RUNNING', finished_at = NULL WHERE id = ?")
        .bind(&run_id)
        .execute(db_pool).await?;

    // 2. Iterate Config (Hardcoded data/wiki)
    let input_dir = "data/wiki";
    let mut dir = fs::read_dir(input_dir).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content_raw = fs::read_to_string(&path).await?;
            
            // Frontmatter extraction
            let (_, content) = if content_raw.starts_with("---") {
                 if let Some(end) = content_raw[3..].find("---") {
                     // We don't need URL here as retry_step re-extracts it
                     ("unknown".to_string(), &content_raw[end+6..])
                 } else { ("unknown".to_string(), content_raw.as_str()) }
            } else { ("unknown".to_string(), content_raw.as_str()) };

            let chunks: Vec<&str> = content.split("\n#").collect();
            
            for (i, raw_chunk) in chunks.iter().enumerate() {
                if raw_chunk.trim().len() < 50 { continue; }
                
                // Check if step exists
                let step = sqlx::query("SELECT id, status FROM pipeline_steps WHERE run_id = ? AND file_name = ? AND chunk_index = ?")
                    .bind(&run_id)
                    .bind(&filename)
                    .bind(i as i64)
                    .fetch_optional(db_pool).await?;

                if let Some(row) = step {
                    let status: String = row.get("status");
                    if status != "COMPLETED" {
                         let id: i64 = row.get("id");
                         info!("Resuming step #{}", id);
                         if let Err(e) = retry_step_with_config(db_pool, id, qa_config.clone()).await {
                             error!("Failed to resume step #{}: {}", id, e);
                         }
                    }
                } else {
                    // Create new step
                     let step_id = sqlx::query(
                        "INSERT INTO pipeline_steps (run_id, file_name, chunk_index, status, step_type, started_at, tenant_id) \
                         VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(&run_id)
                    .bind(&filename)
                    .bind(i as i64)
                    .bind("RUNNING")
                    .bind("FULL_PROCESS")
                    .bind(Utc::now())
                    .bind(&tenant_id)
                    .execute(db_pool).await?.last_insert_id();
                    
                    info!("Created new step #{} for resume", step_id);
                     if let Err(e) = retry_step_with_config(db_pool, step_id as i64, qa_config.clone()).await {
                         error!("Failed to process new step #{}: {}", step_id, e);
                     }
                }
            }
        }
    }

    // Finalize
    sqlx::query("UPDATE pipeline_runs SET status = 'COMPLETED', finished_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(&run_id)
        .execute(db_pool).await?;

    Ok(())
}

pub async fn process_chunk(
    db_pool: &DbPool,
    step_id: u64,
    gen_client: &GeneratorClient,
    gen_model: &str,
    gemini_client: &gemini::Client,
    gemini_model: &str,
    chunk: &WikiChunk,
    qa_count: usize,
    tenant_id: &str
) -> Result<Vec<QAPair>> {
    // 1. Generate Q/A
    let qa_pairs = generate_qa(gen_client, gen_model, chunk, qa_count).await?;
    
    // Save Q/A to DB
    for qa in &qa_pairs {
        sqlx::query(
            "INSERT INTO qa_results (step_id, question, answer, tenant_id) VALUES (?, ?, ?, ?)"
        )
        .bind(step_id as i64)
        .bind(&qa.question)
        .bind(&qa.answer)
        .bind(tenant_id)
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
        "INSERT INTO evaluation_reports (step_id, coverage_score, atomic_facts, missing_facts, reasoning, tenant_id) \
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(step_id as i64)
    .bind(report.coverage_score)
    .bind(facts_json)
    .bind(missing_json)
    .bind(report.reasoning)
    .bind(tenant_id)
    .execute(db_pool).await?;

    Ok(qa_pairs)
}

pub async fn generate_missing_qa_for_step(
    db_pool: &DbPool,
    step_id: u64,
    qa_config: QAConfig
) -> Result<()> {
    // 1. Fetch step details and missing facts
    let step_record = sqlx::query(
        "SELECT ps.file_name, ps.chunk_index, ps.tenant_id, pr.provider, pr.model
         FROM pipeline_steps ps
         JOIN pipeline_runs pr ON ps.run_id = pr.id
         WHERE ps.id = ?"
    )
    .bind(step_id as i64)
    .fetch_optional(db_pool)
    .await?;

    let step = match step_record {
        Some(r) => r,
        None => return Err(anyhow::anyhow!("Step not found")),
    };

    let filename: String = step.get("file_name");
    let chunk_index: i64 = step.get("chunk_index");
    let tenant_id: String = step.get("tenant_id");
    let provider: String = step.get("provider");
    let model: String = step.get("model");

    let file_path = format!("data/wiki/{}", filename);
    let content = fs::read_to_string(&file_path).await?;
    let chunks: Vec<&str> = content.split("\n#").collect();
    
    if chunk_index < 0 || chunk_index >= chunks.len() as i64 {
         return Err(anyhow::anyhow!("Invalid chunk index"));
    }
    
    let chunk_content = &chunks[chunk_index as usize];
    let chunk = WikiChunk {
        source_file: filename.clone(),
        url: format!("local://{}", filename),
        content: chunk_content.to_string(),
    };

    // 3. Setup client
    let gemini_api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not set");
    let gemini_client = gemini::Client::new(&gemini_api_key);
    let db_model = "gemini-2.5-flash";

    let (gen_client, gen_model) = match provider.as_str() {
        "ollama" => {
            let ollama_client = ollama::Client::new();
            (GeneratorClient::Ollama(ollama_client), model)
        },
        "google" => {
            (GeneratorClient::Gemini(gemini_client.clone()), model)
        },
        "heimdall" => {
            let (hd_endpoint, hd_api_key) = resolve_heimdall_config(db_pool, &tenant_id).await;
            (GeneratorClient::Heimdall {
                client: reqwest::Client::new(),
                endpoint: hd_endpoint,
                api_key: hd_api_key,
            }, model)
        },
        _ => return Err(anyhow::anyhow!("Unsupported provider: {}", provider)),
    };

    loop {
        // Fetch current coverage and missing facts
        let report_record = sqlx::query(
            "SELECT coverage_score, CAST(missing_facts AS CHAR) AS missing_facts FROM evaluation_reports WHERE step_id = ?"
        )
        .bind(step_id as i64)
        .fetch_optional(db_pool)
        .await?;

        let (coverage, missing_facts_json): (f32, String) = match report_record {
            Some(r) => (r.get("coverage_score"), r.get("missing_facts")),
            None => return Err(anyhow::anyhow!("Evaluation report not found")),
        };

        // Normalize coverage score (handle old bug where score might be > 1)
        let normalized_coverage = if coverage > 1.0 { coverage / 100.0 } else { coverage };

        if normalized_coverage >= 0.99 {
            info!("Coverage for step #{} reached {:.1}%, breaking loop.", step_id, normalized_coverage * 100.0);
            break;
        }

        let missing_facts_vec: Vec<super::AtomicFact> = serde_json::from_str(&missing_facts_json)?;
        // Limit to 15 missing facts max to prevent prompt explosion and LLM timeouts
        let missing_facts: Vec<String> = missing_facts_vec.into_iter().take(15).map(|f| f.fact).collect();

        if missing_facts.is_empty() {
            info!("No missing facts for step #{}, breaking loop.", step_id);
            break;
        }

        // 4. Generate missing QA
        let new_qa_pairs = generate_missing_qa(
            &gen_client, 
            &gen_model.as_str(), 
            &chunk, 
            &missing_facts, 
            qa_config.get_qa_count(&filename, chunk.content.len())
        ).await?;

        info!("Generated {} new Q/A pairs for step #{}", new_qa_pairs.len(), step_id);

        if new_qa_pairs.is_empty() {
            info!("LLM failed to generate any new pairs for step #{}, breaking to avoid infinite loop.", step_id);
            break;
        }

        // Save New Q/A to DB
        for qa in &new_qa_pairs {
            sqlx::query(
                "INSERT INTO qa_results (step_id, question, answer, tenant_id) VALUES (?, ?, ?, ?)"
            )
            .bind(step_id as i64)
            .bind(&qa.question)
            .bind(&qa.answer)
            .bind(&tenant_id)
            .execute(db_pool).await?;
        }

        // 5. Re-evaluate
        // Fetch all QA pairs for this step now
        let all_qa_records = sqlx::query(
            "SELECT question, answer FROM qa_results WHERE step_id = ?"
        )
        .bind(step_id as i64)
        .fetch_all(db_pool)
        .await?;

        let all_qa_pairs: Vec<QAPair> = all_qa_records.into_iter().map(|row| QAPair {
            question: row.get("question"),
            answer: row.get("answer"),
        }).collect();

        let report_record2 = sqlx::query(
            "SELECT CAST(atomic_facts AS CHAR) AS atomic_facts FROM evaluation_reports WHERE step_id = ?"
        )
        .bind(step_id as i64)
        .fetch_optional(db_pool)
        .await?;

        let atomic_facts_json: String = match report_record2 {
            Some(r) => r.get("atomic_facts"),
            None => Default::default(),
        };

        let facts: Vec<super::AtomicFact> = if !atomic_facts_json.is_empty() {
            serde_json::from_str(&atomic_facts_json)?
        } else {
            Vec::new()
        };

        if !facts.is_empty() {
            let report = verify_coverage(&gemini_client, db_model, &chunk, &facts, &all_qa_pairs).await?;

            let missing_json = serde_json::to_string(&report.missing_facts)?;
            
            sqlx::query(
                "UPDATE evaluation_reports SET coverage_score = ?, missing_facts = ?, reasoning = ? WHERE step_id = ?"
            )
            .bind(report.coverage_score)
            .bind(missing_json)
            .bind(report.reasoning)
            .bind(step_id as i64)
            .execute(db_pool).await?;
            
            info!("Updated coverage score for step #{} to {:.1}%", step_id, report.coverage_score * 100.0);
        } else {
            break; // No facts to verify against
        }
    }

    Ok(())
}
