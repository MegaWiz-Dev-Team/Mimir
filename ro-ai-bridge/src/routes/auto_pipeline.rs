//! Auto-Pipeline — 1-click source processing pipeline
//!
//! POST /api/v1/sources/{id}/auto-pipeline
//! Steps:  1. Chunk → 2. Embed → 3. KG Extract → 4. QA Extract → 5. QA Index
//!
//! Returns immediately with pipeline_run_id; progress tracked via pipeline_steps.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::routes::sources::pageindex::generate_tree;
use crate::routes::sources::{call_llm_api_with_logging, infer_api_base};
use crate::routes::tenant::extract_tenant_id;
use crate::routes::vector::embed_texts;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::qdrant::QdrantService;

/// Strip `<think>...</think>` blocks from Qwen-style reasoning responses,
/// then extract the first JSON object or array from the remaining text.
fn clean_llm_json(raw: &str) -> String {
    // 1. Remove <think>...</think> blocks (greedy, handles multiline)
    let mut text = raw.to_string();
    while let Some(start) = text.find("<think>") {
        if let Some(end) = text.find("</think>") {
            text = format!("{}{}", &text[..start], &text[end + 8..]);
        } else {
            // Unclosed <think> — remove everything from <think> onwards
            text = text[..start].to_string();
            break;
        }
    }

    // 2. Strip markdown code fences
    let text = text.trim();
    let text = text.strip_prefix("```json").unwrap_or(text);
    let text = text.strip_prefix("```").unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text);
    let text = text.trim();

    // 3. If it doesn't start with { or [, try to find the first JSON structure
    if !text.starts_with('{') && !text.starts_with('[') {
        if let Some(pos) = text.find('{').or_else(|| text.find('[')) {
            return text[pos..].to_string();
        }
    }

    text.to_string()
}

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AutoPipelineRequest {
    /// Provider for extraction: "gemini", "heimdall", "ollama", "openai"
    pub provider: Option<String>,
    /// Model to use for extraction
    pub model: Option<String>,
    /// Prompt version for extraction
    pub prompt_version: Option<String>,
    /// Optional run label for benchmarking
    pub run_label: Option<String>,
    /// Skip steps that have already been completed
    pub skip_completed: Option<bool>,
    /// Max chunks to process (default: all)
    pub max_chunks: Option<usize>,
    /// Enable PageIndex Semantic Tree Generation
    pub enable_pageindex: Option<bool>,
    /// Skip KG extraction step (use existing KG data)
    pub skip_kg: Option<bool>,
    /// Skip Chunk Embedding step
    pub skip_embedding: Option<bool>,
    /// Skip QA Generation step
    pub skip_qa: Option<bool>,
}



// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn auto_pipeline_routes() -> Router<DbPool> {
    Router::new()
        .route(
            "/{id}/auto-pipeline",
            axum::routing::post(run_auto_pipeline),
        )
        .route("/{id}/pipeline-status", get(get_pipeline_status))
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// POST /api/v1/sources/{id}/auto-pipeline — Run full pipeline
async fn run_auto_pipeline(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(source_id): Path<i64>,
    Json(req): Json<AutoPipelineRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let run_id = Uuid::new_v4().to_string();
    let _skip_completed = req.skip_completed.unwrap_or(true);
    let router = mimir_core_ai::services::llm_router::LlmRouter::new(pool.clone(), &tenant_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Router error: {}", e)})),
            )
        })?;
    let resolved_slot = router.config.resolve_slot(
        "pipeline_generator",
        req.provider.as_deref(),
        req.model.as_deref(),
    );
    let provider = resolved_slot.provider;
    let model = resolved_slot.model;
    let prompt_version = req.prompt_version.unwrap_or_else(|| "v1.0".into());
    let run_label = req.run_label.clone();
    let max_chunks = req.max_chunks.unwrap_or(10000);
    let enable_pageindex = req.enable_pageindex.unwrap_or(false);
    let skip_kg = req.skip_kg.unwrap_or(false);
    let skip_embedding = req.skip_embedding.unwrap_or(false);
    let skip_qa = req.skip_qa.unwrap_or(false);

    // Verify source exists and belongs to tenant
    let source: Option<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM data_sources WHERE id = ? AND tenant_id = ?")
            .bind(source_id)
            .bind(&tenant_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
            })?;

    let (_, source_name) = source.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Source not found"})),
        )
    })?;

    // Create pipeline run record
    let _ = sqlx::query(
        "INSERT INTO pipeline_runs (id, source_id, tenant_id, status, provider, model, prompt_version, run_label, started_at) VALUES (?, ?, ?, 'running', ?, ?, ?, ?, NOW())"
    )
    .bind(&run_id)
    .bind(source_id)
    .bind(&tenant_id)
    .bind(&provider)
    .bind(&model)
    .bind(&prompt_version)
    .bind(&run_label)
    .execute(&pool)
    .await;

    info!(
        "🚀 Auto-pipeline started: run={} source={} ({}) provider={}/{}",
        run_id, source_id, source_name, provider, model
    );

    // Spawn background pipeline
    let pool_clone = pool.clone();
    let run_id_clone = run_id.clone();
    let tenant_clone = tenant_id.clone();
    let provider_clone = provider.clone();
    let model_clone = model.clone();
    let prompt_version_clone = prompt_version.clone();
    let run_label_clone = run_label.clone();

    tokio::spawn(async move {
        let provider = provider_clone;
        let model = model_clone;
        let _prompt_version = prompt_version_clone;
        let _run_label = run_label_clone;
        let mut total_steps_ok = 0;
        let mut pipeline_error: Option<String> = None;

        // ─── Step 1: Check/Count Chunks ──────────────────────────────────
        let step1_start = std::time::Instant::now();
        log_step(&pool_clone, &run_id_clone, 1, "chunk_check", "running").await;

        let chunks: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
            "SELECT id, content, token_count FROM chunks WHERE source_id = ? LIMIT ?",
        )
        .bind(source_id)
        .bind(max_chunks as i64)
        .fetch_all(&pool_clone)
        .await
        .unwrap_or_default();

        let chunk_count = chunks.len() as i64;
        if chunk_count == 0 {
            log_step_result(
                &pool_clone,
                &run_id_clone,
                1,
                "skipped",
                0,
                step1_start.elapsed().as_millis() as i64,
                Some("No chunks found — run sync first"),
            )
            .await;
            finish_run(
                &pool_clone,
                &run_id_clone,
                "failed",
                Some("No chunks found"),
            )
            .await;
            return;
        }
        log_step_result(
            &pool_clone,
            &run_id_clone,
            1,
            "completed",
            chunk_count,
            step1_start.elapsed().as_millis() as i64,
            None,
        )
        .await;
        total_steps_ok += 1;
        info!("  Step 1/5: ✅ {} chunks found", chunk_count);

        // Resolve embedding model and configs early for entire pipeline scope
        let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool_clone.clone());
        let tenant_config = iam.get_tenant_config(&tenant_clone).await.ok();
        let llm_config = tenant_config
            .as_ref()
            .and_then(|c| c.llm_config.as_ref())
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let embed_model = llm_config.resolve_slot("embedding", None, None).model;

        // C3: Extract tenant-specific prompt customizations for pipeline injection
        let tenant_system_prompt = tenant_config
            .as_ref()
            .and_then(|c| c.system_prompt.as_deref())
            .unwrap_or("")
            .to_string();
        let tenant_qa_rules = tenant_config
            .as_ref()
            .and_then(|c| c.qa_rules.as_ref())
            .map(|r| serde_json::to_string_pretty(&r.0).unwrap_or_default())
            .unwrap_or_default();

        // ─── Step 2: Embed Chunks → Qdrant ───────────────────────────────
        if pipeline_error.is_none() && !skip_embedding {
            let step2_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 2, "embed_chunks", "running").await;


        let qdrant = QdrantService::new();
        // Ensure collection exists with hybrid schema (dense + sparse)
        if let Err(e) = qdrant.init_collection("source_chunks", 1024).await {
            warn!("Collection init warning: {}", e);
        }
        let batch_size = 64;
        let mut embedded = 0i64;
        let mut embed_error = false;

        for chunk_batch in chunks.chunks(batch_size) {
            let texts: Vec<String> = chunk_batch
                .iter()
                .map(|(_, content, _)| content.clone())
                .collect();
            match embed_texts(&texts, &embed_model).await {
                Ok(vectors) => {
                    let mut points = Vec::new();
                    for (i, (chunk_id, content, _)) in chunk_batch.iter().enumerate() {
                        let sparse = mimir_core_ai::services::bm25::text_to_sparse_vector(content);
                        points.push(json!({
                            "id": *chunk_id as u64,
                            "vector": {
                                "dense": vectors[i],
                                "bm25": {
                                    "indices": sparse.indices,
                                    "values": sparse.values,
                                }
                            },
                            "payload": {
                                "content": content,
                                "chunk_id": chunk_id,
                                "source_id": source_id,
                                "tenant_id": tenant_clone,
                                "is_active": true,
                            }
                        }));
                    }
                    let body = json!({ "points": points });
                    match qdrant.upsert_points("source_chunks", body).await {
                        Ok(_) => embedded += points.len() as i64,
                        Err(e) => {
                            error!("Qdrant upsert error: {}", e);
                            embed_error = true;
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Embedding error: {}", e);
                    embed_error = true;
                    break;
                }
            }
        }

        if embed_error {
            log_step_result(
                &pool_clone,
                &run_id_clone,
                2,
                "failed",
                embedded,
                step2_start.elapsed().as_millis() as i64,
                Some("Embedding failed"),
            )
            .await;
            pipeline_error = Some("Embedding failed".into());
        } else {
            log_step_result(
                &pool_clone,
                &run_id_clone,
                2,
                "completed",
                embedded,
                step2_start.elapsed().as_millis() as i64,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 2/5: ✅ {} chunks embedded", embedded);

            // Record embedding pipeline step
            let _ = sqlx::query(
                "INSERT IGNORE INTO pipeline_steps (run_id, source_id, step_name, status, created_at) VALUES (?, ?, 'embedding', 'completed', NOW())"
            )
            .bind(&run_id_clone)
            .bind(source_id)
            .execute(&pool_clone)
            .await;
        } // Close else
        } else if skip_embedding && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 2, "embed_chunks", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                2,
                "skipped",
                0,
                0,
                Some("Skipped by user"),
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 2/5: ⏭️ Chunk embedding skipped by user");
        }

        // ─── Step 2.5: PageIndex Generation (Optional) ────────────────────
        if pipeline_error.is_none() && enable_pageindex {
            let step25_start = std::time::Instant::now();
            log_step(
                &pool_clone,
                &run_id_clone,
                25,
                "pageindex_generation",
                "running",
            )
            .await;

            let mut full_text = String::new();
            for (_id, content, chunk_index) in &chunks {
                let idx = chunk_index.unwrap_or(0);
                full_text.push_str(&format!("\n--- [Page/Chunk {}] ---\n{}\n", idx, content));
            }

            let api_base = infer_api_base(&provider);
            let api_key = resolve_api_key_with_config(&provider, Some(&llm_config));

            let max_index = chunks.last().and_then(|c| c.2).unwrap_or(0);
            match generate_tree(
                &pool_clone,
                &tenant_clone,
                source_id,
                &full_text,
                &api_key,
                &api_base,
                &model,
                &provider,
                max_index,
            )
            .await
            {
                Ok(_) => {
                    log_step_result(
                        &pool_clone,
                        &run_id_clone,
                        25,
                        "completed",
                        1,
                        step25_start.elapsed().as_millis() as i64,
                        None,
                    )
                    .await;
                    total_steps_ok += 1;
                    info!("  Step 2.5: ✅ PageIndex Semantic Tree generated");
                }
                Err(e) => {
                    log_step_result(
                        &pool_clone,
                        &run_id_clone,
                        25,
                        "failed",
                        0,
                        step25_start.elapsed().as_millis() as i64,
                        Some(&e.to_string()),
                    )
                    .await;
                    // Don't fail the whole pipeline for this optional step
                    warn!("  Step 2.5: ⚠️ PageIndex generation failed: {}", e);
                }
            }
        }

        // ─── Step 3: KG Extraction ───────────────────────────────────────
        if pipeline_error.is_none() && !skip_kg {
            let step3_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 3, "kg_extraction", "running").await;

            let api_base = infer_api_base(&provider);
            let api_key = resolve_api_key_with_config(&provider, Some(&llm_config));
            let mut kg_entities = 0i64;
            let mut kg_relations = 0i64;

            // Log step in kg_extraction_runs to fix graph history UI
            let kg_run_id = Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT IGNORE INTO kg_extraction_runs (id, source_id, tenant_id, status, started_at) VALUES (?, ?, ?, 'running', NOW())"
            )
            .bind(&kg_run_id)
            .bind(source_id)
            .bind(&tenant_clone)
            .execute(&pool_clone)
            .await;

            // Connect to Neo4j for dual-write (graceful degradation)
            let neo4j_config = mimir_core_ai::services::neo4j::Neo4jConfig::from_env();
            let neo4j_svc =
                mimir_core_ai::services::neo4j::Neo4jService::try_new(&neo4j_config).await;
            if neo4j_svc.is_some() {
                info!("  Step 3: Neo4j connected — dual-write enabled");
            } else {
                warn!("  Step 3: Neo4j unavailable — SQL-only mode");
            }

            for (_chunk_id, content, _) in &chunks {
                let system_prompt =
                    mimir_core_ai::services::entity_extractor::build_extraction_system_prompt();
                let user_prompt =
                    mimir_core_ai::services::entity_extractor::build_extraction_user_prompt(
                        content, 20,
                    );
                let combined_prompt = format!("{}\n\n{}", system_prompt, user_prompt);

                let start = std::time::Instant::now();
                let result = call_llm_api_with_logging(
                    &api_key,
                    &api_base,
                    &model,
                    &combined_prompt,
                    Some(&pool_clone),
                    Some(&tenant_clone),
                    Some(&provider),
                    Some("auto_pipeline_kg"),
                )
                .await;
                let _latency = start.elapsed().as_millis() as i64;

                if let Ok((response_text, _tokens)) = result {
                    // Parse KG response — strip <think> tags from Qwen-style models
                    let clean = clean_llm_json(&response_text);
                    match serde_json::from_str::<Value>(&clean) {
                        Ok(parsed) => {
                            if let Some(entities) = parsed["entities"].as_array() {
                                for ent in entities {
                                    let ent_name = ent["name"].as_str().unwrap_or("");
                                    let ent_type = ent["type"].as_str().unwrap_or("Concept");
                                    let ent_props = ent.get("properties").map(|p| p.to_string());

                                    // MariaDB insert
                                    let _ = sqlx::query(
                                    "INSERT IGNORE INTO kg_entities (tenant_id, source_id, chunk_id, name, entity_type, properties) VALUES (?, ?, ?, ?, ?, ?)"
                                )
                                .bind(&tenant_clone).bind(source_id).bind(_chunk_id)
                                .bind(ent_name)
                                .bind(ent_type)
                                .bind(&ent_props)
                                .execute(&pool_clone).await;

                                    // Neo4j dual-write
                                    if let Some(ref neo4j) = neo4j_svc {
                                        let _ = neo4j
                                            .upsert_entity(
                                                &tenant_clone,
                                                ent_name,
                                                ent_type,
                                                ent_props.as_deref(),
                                                Some(source_id),
                                                Some(*_chunk_id),
                                            )
                                            .await;
                                    }
                                    kg_entities += 1;
                                }
                            }
                            if let Some(relations) = parsed["relations"].as_array() {
                                for rel in relations {
                                    // Look up from/to entity IDs by name
                                    let from_name = rel["from"].as_str().unwrap_or("");
                                    let to_name = rel["to"].as_str().unwrap_or("");
                                    let rel_type = rel["type"].as_str().unwrap_or("");

                                    let from_id: Option<(i64,)> = sqlx::query_as(
                                    "SELECT id FROM kg_entities WHERE tenant_id = ? AND name = ? LIMIT 1"
                                ).bind(&tenant_clone).bind(from_name)
                                .fetch_optional(&pool_clone).await.unwrap_or(None);
                                    let to_id: Option<(i64,)> = sqlx::query_as(
                                    "SELECT id FROM kg_entities WHERE tenant_id = ? AND name = ? LIMIT 1"
                                ).bind(&tenant_clone).bind(to_name)
                                .fetch_optional(&pool_clone).await.unwrap_or(None);
                                    if let (Some((fid,)), Some((tid,))) = (from_id, to_id) {
                                        // MariaDB insert
                                        let _ = sqlx::query(
                                        "INSERT IGNORE INTO kg_relations (tenant_id, source_id, from_entity_id, to_entity_id, relation_type) VALUES (?, ?, ?, ?, ?)"
                                    )
                                    .bind(&tenant_clone).bind(source_id)
                                    .bind(fid).bind(tid)
                                    .bind(rel_type)
                                    .execute(&pool_clone).await;

                                        // Neo4j dual-write
                                        if let Some(ref neo4j) = neo4j_svc {
                                            let _ = neo4j
                                                .upsert_relation(
                                                    &tenant_clone,
                                                    from_name,
                                                    to_name,
                                                    rel_type,
                                                    None,
                                                    Some(source_id),
                                                )
                                                .await;
                                        }
                                        kg_relations += 1;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "KG parse failed: {}. Raw(200): {:?} Clean(200): {:?}",
                                e,
                                &response_text[..response_text.len().min(200)],
                                &clean[..clean.len().min(200)]
                            );
                        }
                    }
                }
            }

            log_step_result(
                &pool_clone,
                &run_id_clone,
                3,
                "completed",
                kg_entities,
                step3_start.elapsed().as_millis() as i64,
                None,
            )
            .await;

            // Update kg_run status
            let _ = sqlx::query(
                "UPDATE kg_extraction_runs SET status = 'completed', entities_found = ?, relations_found = ?, chunks_processed = ?, finished_at = NOW() WHERE id = ?"
            )
            .bind(kg_entities)
            .bind(kg_relations)
            .bind(chunks.len() as i64)
            .bind(&kg_run_id)
            .execute(&pool_clone)
            .await;

            total_steps_ok += 1;
            info!(
                "  Step 3/5: ✅ {} entities, {} relations extracted (Neo4j={})",
                kg_entities,
                kg_relations,
                neo4j_svc.is_some()
            );
        } else if skip_kg && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 3, "kg_extraction", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                3,
                "skipped",
                0,
                0,
                Some("Skipped by user"),
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 3/5: ⏭️ KG extraction skipped by user");
        }

        // ─── Step 4: QA Extraction ───────────────────────────────────────
        if pipeline_error.is_none() && !skip_qa {
            let step4_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 4, "qa_extraction", "running").await;

            let api_base = infer_api_base(&provider);
            let api_key = resolve_api_key_with_config(&provider, Some(&llm_config));
            let mut qa_count = 0i64;

            // Create pipeline_steps entry for QA tracking (needed as FK for qa_results)
            let _ = sqlx::query(
                "INSERT INTO pipeline_steps (run_id, file_name, chunk_index, status, step_type, tenant_id) VALUES (?, ?, 0, 'COMPLETED', 'GENERATE', ?)"
            )
            .bind(&run_id_clone)
            .bind(format!("auto_pipeline_qa_{}", run_id_clone))
            .bind(&tenant_clone)
            .execute(&pool_clone)
            .await;

            let step_id: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM pipeline_steps WHERE run_id = ? AND step_type = 'GENERATE' ORDER BY id DESC LIMIT 1"
            )
            .bind(&run_id_clone)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            let step_id = step_id.map(|s| s.0).unwrap_or(0);

            // Fetch chunks already QA'd to skip them
            let existing_qas: Vec<i64> = sqlx::query_scalar(
                "SELECT DISTINCT chunk_id FROM qa_results WHERE source_id = ? AND chunk_id IS NOT NULL"
            )
            .bind(source_id)
            .fetch_all(&pool_clone)
            .await
            .unwrap_or_default();

            for (chunk_id, content, _) in &chunks {
                if existing_qas.contains(chunk_id) {
                    continue;
                }

                // C3: Inject tenant-specific context into QA generation prompt
                let system_context = if !tenant_system_prompt.is_empty() {
                    format!("\nDomain Context: {}\n", tenant_system_prompt)
                } else {
                    String::new()
                };
                let extra_rules = if !tenant_qa_rules.is_empty() {
                    format!("\nAdditional Rules from Admin:\n{}\n", tenant_qa_rules)
                } else {
                    String::new()
                };
                let prompt = format!("You are a QA Generator.{} Generate 2-5 high-quality question-answer pairs from the given text.\n\nReturn STRICT JSON array:\n[\n  {{\"question\": \"...\", \"answer\": \"...\"}}\n]\n\nRules:\n1. Keep answers concise and factual\n2. Only generate questions answerable from the text\n3. Prefer why/how/what questions over yes/no\n4. Cover different aspects of the text\n5. Return ONLY the JSON array{}\n\nText:\n{}", system_context, extra_rules, content);

                let start = std::time::Instant::now();
                let result = call_llm_api_with_logging(
                    &api_key,
                    &api_base,
                    &model,
                    &prompt,
                    Some(&pool_clone),
                    Some(&tenant_clone),
                    Some(&provider),
                    Some("auto_pipeline_qa"),
                )
                .await;
                let _latency = start.elapsed().as_millis() as i64;

                if let Ok((response_text, _tokens)) = result {
                    let clean = clean_llm_json(&response_text);
                    if let Ok(qa_pairs) = serde_json::from_str::<Vec<Value>>(&clean) {
                        for qa in &qa_pairs {
                            let _ = sqlx::query(
                                "INSERT INTO qa_results (tenant_id, step_id, question, answer, context, source_id, chunk_id) VALUES (?, ?, ?, ?, ?, ?, ?)"
                            )
                            .bind(&tenant_clone).bind(step_id)
                            .bind(qa["question"].as_str().unwrap_or(""))
                            .bind(qa["answer"].as_str().unwrap_or(""))
                            .bind(content.chars().take(500).collect::<String>())
                            .bind(source_id)
                            .bind(chunk_id)
                            .execute(&pool_clone).await;
                            qa_count += 1;
                        }
                    }
                }
            }

            log_step_result(
                &pool_clone,
                &run_id_clone,
                4,
                "completed",
                qa_count,
                step4_start.elapsed().as_millis() as i64,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 4/5: ✅ {} QA pairs generated", qa_count);
        } else if skip_qa && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 4, "qa_extraction", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                4,
                "skipped",
                0,
                0,
                Some("Skipped by user"),
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 4/5: ⏭️ QA extraction skipped by user");
        }

        // ─── Step 5: Index QA into Qdrant ────────────────────────────────
        if pipeline_error.is_none() && !skip_qa {
            let step5_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 5, "qa_indexing", "running").await;

            let qa_rows: Vec<(i64, String, String)> = sqlx::query_as(
                "SELECT id, question, answer FROM qa_results WHERE tenant_id = ? AND source_id = ? ORDER BY id"
            )
            .bind(&tenant_clone)
            .bind(source_id)
            .fetch_all(&pool_clone)
            .await
            .unwrap_or_default();

            if qa_rows.is_empty() {
                log_step_result(
                    &pool_clone,
                    &run_id_clone,
                    5,
                    "skipped",
                    0,
                    0,
                    Some("No QA pairs to index"),
                )
                .await;
            } else {
                let iam =
                    mimir_core_ai::services::iam::IamService::new_with_env(pool_clone.clone());
                let tc = iam.get_tenant_config(&tenant_clone).await.ok();
                let lc = tc
                    .as_ref()
                    .and_then(|c| c.llm_config.as_ref())
                    .map(|c| c.0.clone())
                    .unwrap_or_default();
                let embed_model = lc.resolve_slot("embedding", None, None).model;
                let qdrant = QdrantService::new();

                let mut indexed = 0i64;
                for batch in qa_rows.chunks(64) {
                    let texts: Vec<String> = batch
                        .iter()
                        .map(|(_, q, a)| format!("{}\n{}", q, a))
                        .collect();

                    match embed_texts(&texts, &embed_model).await {
                        Ok(vectors) => {
                            let mut points = Vec::new();
                            for (i, (qa_id, question, answer)) in batch.iter().enumerate() {
                                let text_content = format!("{}\n{}", question, answer);
                                let sparse = mimir_core_ai::services::bm25::text_to_sparse_vector(&text_content);
                                points.push(json!({
                                    "id": *qa_id as u64,
                                    "vector": {
                                        "dense": vectors[i].clone(),
                                        "bm25": {
                                            "indices": sparse.indices,
                                            "values": sparse.values,
                                        }
                                    },
                                    "payload": {
                                        "question": question,
                                        "answer": answer,
                                        "qa_id": qa_id,
                                        "source_id": source_id,
                                        "tenant_id": tenant_clone,
                                        "is_active": true,
                                    }
                                }));
                            }
                            let body = json!({ "points": points });
                            if let Ok(_) = qdrant.upsert_points("golden_qa", body).await {
                                indexed += points.len() as i64;
                            }
                        }
                        Err(e) => warn!("QA embedding error: {}", e),
                    }
                }

                log_step_result(
                    &pool_clone,
                    &run_id_clone,
                    5,
                    "completed",
                    indexed,
                    step5_start.elapsed().as_millis() as i64,
                    None,
                )
                .await;
                total_steps_ok += 1;
                info!("  Step 5/5: ✅ {} QA pairs indexed to Qdrant", indexed);
            }
        } else if skip_qa && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 5, "qa_indexing", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                5,
                "skipped",
                0,
                0,
                Some("Skipped by user"),
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 5/5: ⏭️ QA indexing skipped by user");
        }

        // ─── Finish pipeline ─────────────────────────────────────────────
        let final_status = if pipeline_error.is_some() {
            "failed"
        } else {
            "completed"
        };
        finish_run(
            &pool_clone,
            &run_id_clone,
            final_status,
            pipeline_error.as_deref(),
        )
        .await;
        info!(
            "🏁 Auto-pipeline {} finished: {} steps completed, status={}",
            run_id_clone, total_steps_ok, final_status
        );
    });

    Ok(Json(json!({
        "pipeline_run_id": run_id,
        "source_id": source_id,
        "source_name": source_name,
        "provider": provider,
        "model": model,
        "status": "running",
        "message": "Auto-pipeline started in background. Check /pipeline-status for progress."
    })))
}

/// GET /api/v1/sources/{id}/pipeline-status — Get latest pipeline status
async fn get_pipeline_status(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(source_id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Get latest run
    let run: Option<(String, String, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, status, provider, model, error_message, run_label FROM pipeline_runs WHERE source_id = ? AND tenant_id = ? ORDER BY started_at DESC LIMIT 1"
    )
    .bind(source_id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match run {
        Some((run_id, status, provider, model, error_msg, run_label)) => {
            let steps: Vec<(u8, String, String, i64, i64, Option<String>)> = sqlx::query_as(
                "SELECT step_number, step_name, status, item_count, latency_ms, error_message FROM pipeline_run_steps WHERE run_id = ? ORDER BY step_number"
            )
            .bind(&run_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            Ok(Json(json!({
                "run_id": run_id,
                "source_id": source_id,
                "status": status,
                "provider": provider,
                "model": model,
                "error": error_msg,
                "run_label": run_label,
                "steps": steps.iter().map(|(num, name, status, count, lat, err)| json!({
                    "step": num, "name": name, "status": status,
                    "count": count, "latency_ms": lat, "error": err
                })).collect::<Vec<_>>()
            })))
        }
        None => Ok(Json(
            json!({"source_id": source_id, "status": "no_runs", "steps": []}),
        )),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────────

async fn log_step(pool: &DbPool, run_id: &str, step: u8, name: &str, status: &str) {
    let _ = sqlx::query(
        "INSERT INTO pipeline_run_steps (run_id, step_number, step_name, status) VALUES (?, ?, ?, ?)"
    )
    .bind(run_id).bind(step).bind(name).bind(status)
    .execute(pool).await;
}

async fn log_step_result(
    pool: &DbPool,
    run_id: &str,
    step: u8,
    status: &str,
    count: i64,
    latency_ms: i64,
    error: Option<&str>,
) {
    let _ = sqlx::query(
        "UPDATE pipeline_run_steps SET status = ?, item_count = ?, latency_ms = ?, error_message = ? WHERE run_id = ? AND step_number = ?"
    )
    .bind(status).bind(count).bind(latency_ms).bind(error)
    .bind(run_id).bind(step)
    .execute(pool).await;
}

async fn finish_run(pool: &DbPool, run_id: &str, status: &str, error: Option<&str>) {
    let _ = sqlx::query(
        "UPDATE pipeline_runs SET status = ?, error_message = ?, finished_at = NOW() WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(run_id)
    .execute(pool)
    .await;
}

/// Resolve API key from tenant config (if available), then environment fallback
fn resolve_api_key_with_config(
    provider: &str,
    config: Option<&mimir_core_ai::models::iam::LlmConfig>,
) -> String {
    // Tier 1: Tenant-specific keys from LlmConfig
    if let Some(cfg) = config {
        let tenant_key = match provider {
            "google" | "gemini" => cfg.google_api_key.clone(),
            "openai" => cfg.openai_api_key.clone(),
            "azure" => cfg.azure_api_key.clone(),
            "heimdall" => cfg.heimdall_api_key.clone(),
            _ => cfg.heimdall_api_key.clone(),
        };
        if let Some(key) = tenant_key {
            if !key.is_empty() {
                return key;
            }
        }
    }

    // Tier 2: Environment variable fallback
    match provider {
        "google" | "gemini" => std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "heimdall" => std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
        _ => std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
    }
}
