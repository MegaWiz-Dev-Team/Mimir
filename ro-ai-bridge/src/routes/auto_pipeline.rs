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
use dashmap::DashMap;
use once_cell::sync::Lazy;
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

static PIPELINE_TASKS: Lazy<DashMap<String, tokio::task::JoinHandle<()>>> = Lazy::new(DashMap::new);

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn batch_pipeline_routes() -> Router<DbPool> {
    Router::new()
        .route("/batch-pipeline", axum::routing::post(run_batch_pipeline))
        .route("/pipeline-overview", get(get_pipeline_overview))
        .route("/{id}/pipeline-status", get(get_pipeline_status))
        .route("/{id}/pipeline-cancel", axum::routing::post(cancel_pipeline_run))
}

/// Sweep orphaned pipeline runs stuck in 'running' from a previous pod lifecycle.
/// Call this once at application startup to prevent zombie runs from blocking new pipelines.
pub async fn recover_orphaned_pipeline_runs(pool: &DbPool) {
    let result = sqlx::query(
        "UPDATE pipeline_runs SET status = 'failed', error_message = 'Orphaned: server restarted', finished_at = NOW() WHERE status = 'running' AND started_at < NOW() - INTERVAL 10 MINUTE"
    )
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            let count = r.rows_affected();
            if count > 0 {
                warn!("🧹 Recovered {} orphaned pipeline run(s) from previous server lifecycle", count);
            } else {
                info!("✅ No orphaned pipeline runs found");
            }
        }
        Err(e) => {
            error!("Failed to recover orphaned pipeline runs: {}", e);
        }
    }
}



// ─── Handlers ───────────────────────────────────────────────────────────────────

pub async fn run_pipeline_for_source(
    pool: DbPool,
    tenant_id: String,
    source_id: i64,
    req: std::sync::Arc<BatchPipelineRequest>,
) -> Result<(), String> {
    let run_id = Uuid::new_v4().to_string();
    let router = mimir_core_ai::services::llm_router::LlmRouter::new(pool.clone(), &tenant_id)
        .await
        .map_err(|e| format!("Router error: {}", e))?;
    let resolved_slot = if let (Some(ref p), Some(ref m)) = (req.provider.as_deref(), req.model.as_deref()) {
        mimir_core_ai::models::iam::LlmSlot {
            provider: p.to_string(),
            model: m.to_string()
        }
    } else {
        router.config.resolve_slot(
            "pipeline_generator",
            Some(&router.default_provider),
            Some(&router.default_model),
        )
    };
    let provider = resolved_slot.provider;
    let model = resolved_slot.model;
    let prompt_version = "v1.0".to_string();
    let run_label = None::<String>;
    let max_chunks = 10000;
    
    let enable_pageindex = req.enable_pageindex.unwrap_or(false);
    let skip_kg = !req.enable_kg.unwrap_or(true);
    let skip_embedding = !req.enable_embedding.unwrap_or(true);
    let skip_qa = !req.enable_qa.unwrap_or(true);

    // Verify source exists and belongs to tenant
    let source: Option<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM data_sources WHERE id = ? AND tenant_id = ?")
            .bind(source_id)
            .bind(&tenant_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

    let (_, source_name) = source.ok_or_else(|| "Source not found".to_string())?;

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

    let handle_run_id = run_id_clone.clone();
    let handle = tokio::spawn(async move {
        let provider = provider_clone;
        let model = model_clone;
        let _prompt_version = prompt_version_clone;
        let _run_label = run_label_clone;
        let mut total_steps_ok = 0;
        let mut pipeline_error: Option<String> = None;

        // ─── Preload steps to make them visible as pending ───────────────
        let steps_to_preload = vec![
            (1, "chunk_check"),
            (2, "embed_chunks"),
            (3, "pageindex_generation"),
            (4, "kg_extraction"),
            (5, "qa_extraction"),
            (6, "auto_qc_filter"),
            (7, "qa_indexing"),
            (8, "graph_intelligence"),
        ];
        for (step_num, step_name) in steps_to_preload {
            log_step(&pool_clone, &run_id_clone, step_num, step_name, "pending").await;
        }

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
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 2/5: ⏭️ Chunk embedding skipped by user");
        }

        // ─── Step 2.5: PageIndex Generation (Optional) ────────────────────
        if pipeline_error.is_none() && enable_pageindex {
            let (existing_tree,): (Option<String>,) = sqlx::query_as("SELECT pageindex_tree FROM data_sources WHERE id = ?")
                .bind(source_id)
                .fetch_one(&pool_clone)
                .await
                .unwrap_or((None,));

            let should_skip = existing_tree.map(|t| t.len() > 15).unwrap_or(false);

            if should_skip {
                log_step(&pool_clone, &run_id_clone, 3, "pageindex_generation", "running").await;
                log_step_result(
                    &pool_clone,
                    &run_id_clone,
                    3,
                    "skipped",
                    0,
                    0,
                    Some("PageIndex already exists"),
                )
                .await;
                total_steps_ok += 1;
                info!("  Step 3/7: ⏭️ PageIndex generation skipped (already exists)");
            } else {
                let step25_start = std::time::Instant::now();
                log_step(
                    &pool_clone,
                    &run_id_clone,
                    3,
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

            let _max_index = chunks.last().and_then(|c| c.2).unwrap_or(0);
            match generate_tree(
                &pool_clone,
                &tenant_clone,
                source_id,
                &api_key,
                &api_base,
                &model,
                &provider,
            )
            .await
            {
                Ok(_) => {
                    log_step_result(
                        &pool_clone,
                        &run_id_clone,
                        3,
                        "completed",
                        1,
                        step25_start.elapsed().as_millis() as i64,
                        None,
                    )
                    .await;
                    total_steps_ok += 1;
                    info!("  Step 3/7: ✅ PageIndex Semantic Tree generated");
                }
                Err(e) => {
                    log_step_result(
                        &pool_clone,
                        &run_id_clone,
                        3,
                        "failed",
                        0,
                        step25_start.elapsed().as_millis() as i64,
                        Some(&e.to_string()),
                    )
                    .await;
                    // Don't fail the whole pipeline for this optional step
                    warn!("  Step 3/7: ⚠️ PageIndex generation failed: {}", e);
                }
            }
            } // end else
        } else if !enable_pageindex && pipeline_error.is_none() {
            log_step_result(
                &pool_clone,
                &run_id_clone,
                3,
                "skipped",
                0,
                0,
                None,
            )
            .await;
            info!("  Step 3/8: ⏭️ PageIndex generation disabled");
        }

        // ─── Step 4: KG Extraction ───────────────────────────────────────
        if pipeline_error.is_none() && !skip_kg {
            let step3_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 4, "kg_extraction", "running").await;

            let existing_kg_run: Option<(i64,)> = sqlx::query_as(
                "SELECT ps.id FROM pipeline_steps ps JOIN pipeline_runs pr ON ps.run_id = pr.id WHERE pr.source_id = ? AND ps.step_name = 'kg_extraction' AND ps.status IN ('completed', 'completed (cached)') LIMIT 1"
            )
            .bind(source_id)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            if existing_kg_run.is_some() {
                log_step_result(
                    &pool_clone,
                    &run_id_clone,
                    4,
                    "completed (cached)",
                    chunks.len() as i64,
                    0,
                    Some("KG extraction already exists"),
                )
                .await;
                total_steps_ok += 1;
                info!("  Step 4/8: ⏭️ KG extraction skipped (already exists)");
            } else {
                let api_base = infer_api_base(&provider);
                let api_key = resolve_api_key_with_config(&provider, Some(&llm_config));
                let mut kg_entities = 0i64;
                let mut kg_relations = 0i64;

                // Connect to Neo4j for dual-write (graceful degradation)
                let neo4j_config = mimir_core_ai::services::neo4j::Neo4jConfig::from_env();
                let neo4j_svc =
                    mimir_core_ai::services::neo4j::Neo4jService::try_new(&neo4j_config).await;
                if neo4j_svc.is_some() {
                    info!("  Step 4: Neo4j connected — dual-write enabled");
                } else {
                    warn!("  Step 4: Neo4j unavailable — SQL-only mode");
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

                                        let existing_entity: Option<(i64, Option<Vec<u8>>)> = sqlx::query_as(
                                            "SELECT id, properties FROM kg_entities WHERE tenant_id = ? AND LOWER(name) = LOWER(?) LIMIT 1"
                                        )
                                        .bind(&tenant_clone)
                                        .bind(&ent_name)
                                        .fetch_optional(&pool_clone).await.unwrap_or(None);

                                        if let Some((existing_id, raw_props)) = existing_entity {
                                            // Handle Semantic Duplicate: append chunk_id to properties.found_in_chunks
                                            let mut props: Value = raw_props.as_ref()
                                                .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
                                                .and_then(|p| serde_json::from_str(&p).ok())
                                                .unwrap_or(serde_json::json!({}));
                                                
                                            let mut chunks = vec![];
                                            if let Some(arr) = props.get("found_in_chunks").and_then(|v| v.as_array()) {
                                                for v in arr {
                                                    if let Some(c) = v.as_i64() {
                                                        chunks.push(c);
                                                    }
                                                }
                                            }
                                            if !chunks.contains(_chunk_id) {
                                                chunks.push(*_chunk_id);
                                            }
                                            if let Some(obj) = props.as_object_mut() {
                                                obj.insert("found_in_chunks".to_string(), serde_json::json!(chunks));
                                            }
                                            
                                            // Update existing
                                            let _ = sqlx::query("UPDATE kg_entities SET properties = ? WHERE id = ?")
                                                .bind(props.to_string())
                                                .bind(existing_id)
                                                .execute(&pool_clone).await;
                                        } else {
                                            // MariaDB insert
                                            let _ = sqlx::query(
                                                "INSERT IGNORE INTO kg_entities (tenant_id, source_id, chunk_id, name, entity_type, properties) VALUES (?, ?, ?, ?, ?, ?)"
                                            )
                                            .bind(&tenant_clone).bind(source_id).bind(_chunk_id)
                                            .bind(ent_name)
                                            .bind(ent_type)
                                            .bind(&ent_props)
                                            .execute(&pool_clone).await;
                                            
                                            kg_entities += 1;
                                        }

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
                    4,
                    "completed",
                    kg_entities,
                    step3_start.elapsed().as_millis() as i64,
                    None,
                )
                .await;

                total_steps_ok += 1;
                info!(
                    "  Step 4/8: ✅ {} entities, {} relations extracted (Neo4j={})",
                    kg_entities,
                    kg_relations,
                    neo4j_svc.is_some()
                );
            }
        } else if skip_kg && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 4, "kg_extraction", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                4,
                "skipped",
                0,
                0,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 4/7: ⏭️ KG extraction skipped by user");
        }

        // ─── Step 5: QA Extraction ───────────────────────────────────────
        if pipeline_error.is_none() && !skip_qa {
            let step4_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 5, "qa_extraction", "running").await;

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

            let existing_qas: Vec<i64> = sqlx::query_scalar(
                "SELECT DISTINCT chunk_id FROM qa_results WHERE source_id = ? AND chunk_id IS NOT NULL"
            )
            .bind(source_id)
            .fetch_all(&pool_clone)
            .await
            .unwrap_or_default();

            if existing_qas.len() >= chunks.len() && !chunks.is_empty() {
                log_step_result(
                    &pool_clone,
                    &run_id_clone,
                    5,
                    "completed (cached)",
                    chunks.len() as i64,
                    0,
                    Some("QA extraction completely cached"),
                )
                .await;
                total_steps_ok += 1;
                total_steps_ok += 1;
                info!("  Step 5/7: ⏭️ QA extraction completely skipped (already cached)");
            } else {

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
                        
                        // Update chunk badge in the UI (Knowledge tab)
                        let _ = sqlx::query("UPDATE chunks SET metadata_json = JSON_SET(COALESCE(metadata_json, '{}'), '$.qa_status', 'completed') WHERE id = ?")
                            .bind(chunk_id)
                            .execute(&pool_clone).await;
                    }
                }
            }

            log_step_result(
                &pool_clone,
                &run_id_clone,
                5,
                "completed",
                qa_count,
                step4_start.elapsed().as_millis() as i64,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 5/7: ✅ {} QA pairs generated", qa_count);
            }
        } else if skip_qa && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 5, "qa_extraction", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                5,
                "skipped",
                0,
                0,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 5/7: ⏭️ QA extraction skipped by user");
        }

        // ─── Step 6: Auto QC Filter ───────────────────────────────────────
        if pipeline_error.is_none() && !skip_qa {
            let step6_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 6, "auto_qc_filter", "running").await;

            let previous_qc: Option<(i64,)> = sqlx::query_as(
                "SELECT ps.id FROM pipeline_steps ps JOIN pipeline_runs pr ON ps.run_id = pr.id WHERE pr.source_id = ? AND ps.step_name = 'auto_qc_filter' AND ps.status IN ('completed', 'completed (cached)') LIMIT 1"
            )
            .bind(source_id)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            if previous_qc.is_some() {
                log_step_result(&pool_clone, &run_id_clone, 6, "completed (cached)", 0, 0, Some("QC Filter successfully cached")).await;
                total_steps_ok += 1;
                info!("  Step 6/8: ⏭️ Auto QC Filter skipped (already filtered)");
            } else {
                let qa_rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
                    "SELECT id, question, answer, context FROM qa_results WHERE tenant_id = ? AND source_id = ? ORDER BY id"
                )
                .bind(&tenant_clone)
                .bind(source_id)
                .fetch_all(&pool_clone)
                .await
                .unwrap_or_default();

            if qa_rows.is_empty() {
                log_step_result(&pool_clone, &run_id_clone, 6, "skipped", 0, 0, Some("No QA pairs to QC")).await;
            } else {
                let api_base = infer_api_base(&provider);
                let api_key = resolve_api_key_with_config(&provider, Some(&llm_config));
                let mut filtered_count = 0i64;
                
                // Batch size of 10 pairs
                let batch_size = 10;
                for batch in qa_rows.chunks(batch_size) {
                    let mut batch_json = Vec::new();
                    for (id, q, a, _c) in batch {
                        batch_json.push(serde_json::json!({"id": id, "question": q, "answer": a}));
                    }
                    
                    let context_str = batch.first().and_then(|b| b.3.clone()).unwrap_or_default();
                    let prompt = format!("You are an AI Judge evaluating QA pairs.\nEvaluate the given QA pairs.\nReturn a strict JSON array of objects `[{{\"id\": 123, \"pass\": true/false, \"reason\": \"...\"}}]`.\nA pair passes if it is factually grounded in the context and makes grammatical sense.\n\nContext:\n{}\n\nQA Pairs to Evaluate:\n{}", context_str, serde_json::to_string_pretty(&batch_json).unwrap_or_default());
                    
                    let result = call_llm_api_with_logging(
                        &api_key, &api_base, &model, &prompt,
                        Some(&pool_clone), Some(&tenant_clone), Some(&provider), Some("auto_pipeline_qc")
                    ).await;
                    
                    if let Ok((response_text, _tokens)) = result {
                        let clean = clean_llm_json(&response_text);
                        if let Ok(evaluations) = serde_json::from_str::<Vec<serde_json::Value>>(&clean) {
                            for eval in evaluations {
                                if let (Some(id), Some(pass)) = (eval["id"].as_i64(), eval["pass"].as_bool()) {
                                    if !pass {
                                        let _ = sqlx::query("DELETE FROM qa_results WHERE id = ?").bind(id).execute(&pool_clone).await;
                                        filtered_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                
                log_step_result(
                    &pool_clone, &run_id_clone, 6, "completed", filtered_count, step6_start.elapsed().as_millis() as i64, None
                ).await;
                total_steps_ok += 1;
                info!("  Step 6/8: ✅ QC Filter rejected {} bad QA pairs out of {}", filtered_count, qa_rows.len());
            }
            }
        } else if skip_qa && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 6, "auto_qc_filter", "running").await;
            log_step_result(&pool_clone, &run_id_clone, 6, "skipped", 0, 0, None).await;
            total_steps_ok += 1;
            info!("  Step 6/8: ⏭️ Auto QC Filter skipped");
        }

        // ─── Step 7: Index QA into Qdrant ────────────────────────────────
        if pipeline_error.is_none() && !skip_qa {
            let step7_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 7, "qa_indexing", "running").await;

            let previous_idx: Option<(i64,)> = sqlx::query_as(
                "SELECT ps.id FROM pipeline_steps ps JOIN pipeline_runs pr ON ps.run_id = pr.id WHERE pr.source_id = ? AND ps.step_name = 'qa_indexing' AND ps.status IN ('completed', 'completed (cached)') LIMIT 1"
            )
            .bind(source_id)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            if previous_idx.is_some() {
                log_step_result(&pool_clone, &run_id_clone, 7, "completed (cached)", 0, 0, Some("QA Indexing successfully cached")).await;
                total_steps_ok += 1;
                info!("  Step 7/8: ⏭️ QA Indexing skipped (already indexed)");
            } else {
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
                    7,
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
                if let Err(e) = qdrant.init_collection("golden_qa", 1024).await {
                    warn!("Collection init warning for golden_qa: {}", e);
                }

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
                    7,
                    "completed",
                    indexed,
                    step7_start.elapsed().as_millis() as i64,
                    None,
                )
                .await;
                total_steps_ok += 1;
                info!("  Step 7/8: ✅ {} QA pairs indexed to Qdrant", indexed);
            }
            } 
        } else if skip_qa && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 7, "qa_indexing", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                7,
                "skipped",
                0,
                0,
                None,
            )
            .await;
            total_steps_ok += 1;
            info!("  Step 7/8: ⏭️ QA indexing skipped by user");
        }
        // ─── Step 8: Graph Intelligence ──────────────────────────────────
        if pipeline_error.is_none() && !skip_kg {
            let step8_start = std::time::Instant::now();
            log_step(&pool_clone, &run_id_clone, 8, "graph_intelligence", "running").await;

            let previous_gi: Option<(i64,)> = sqlx::query_as(
                "SELECT ps.id FROM pipeline_steps ps JOIN pipeline_runs pr ON ps.run_id = pr.id WHERE pr.source_id = ? AND ps.step_name = 'graph_intelligence' AND ps.status IN ('completed', 'completed (cached)') LIMIT 1"
            )
            .bind(source_id)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            if previous_gi.is_some() {
                log_step_result(&pool_clone, &run_id_clone, 8, "completed (cached)", 0, 0, Some("Graph Intelligence successfully cached")).await;
                total_steps_ok += 1;
                info!("  Step 8/8: ⏭️ Graph Intelligence skipped (already optimized)");
            } else {
                if let Ok(graph_router) = mimir_core_ai::services::llm_router::LlmRouter::new(pool_clone.clone(), &tenant_clone).await {
                    match mimir_core_ai::services::graph_analytics::generate_graph_insights(&pool_clone, &tenant_clone, &graph_router, Some(&provider), Some(&model)).await {
                    Ok(insights) => {
                        let insights_count = insights.len() as i64;
                        log_step_result(
                            &pool_clone,
                            &run_id_clone,
                            8,
                            "completed",
                            insights_count,
                            step8_start.elapsed().as_millis() as i64,
                            None,
                        )
                        .await;
                        total_steps_ok += 1;
                        info!("  Step 8/8: ✅ Graph Intelligence generated {} insights", insights_count);
                        if let Some(first) = insights.first() {
                            info!("    -> Graph Insight [{}]: {}", first.question_type, first.question);
                        }
                    }
                    Err(e) => {
                        log_step_result(
                            &pool_clone,
                            &run_id_clone,
                            8,
                            "failed",
                            0,
                            step8_start.elapsed().as_millis() as i64,
                            Some(&e),
                        )
                        .await;
                        warn!("  Step 8/8: ⚠️ Graph Intelligence failed: {}", e);
                    }
                }
            } else {
                warn!("  Step 8/8: ⚠️ Failed to initialize LlmRouter for Graph Analytics");
            }
            }
        } else if skip_kg && pipeline_error.is_none() {
            log_step(&pool_clone, &run_id_clone, 8, "graph_intelligence", "running").await;
            log_step_result(
                &pool_clone,
                &run_id_clone,
                8,
                "skipped",
                0,
                0,
                None,
            ).await;
            total_steps_ok += 1;
            info!("  Step 8/8: ⏭️ Graph Intelligence skipped");
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
        PIPELINE_TASKS.remove(&handle_run_id);
    });

    PIPELINE_TASKS.insert(run_id.clone(), handle);
    Ok(())
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
        "INSERT INTO pipeline_run_steps (run_id, step_number, step_name, status) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE status = VALUES(status), step_name = VALUES(step_name)"
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
        _ => std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
    }
}

// ─── Batch Engine ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BatchPipelineRequest {
    pub source_ids: Option<Vec<i64>>,
    pub process_all: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub embedding_provider: Option<String>,
    pub embedding_model: Option<String>,
    pub enable_embedding: Option<bool>,
    pub enable_kg: Option<bool>,
    pub enable_qa: Option<bool>,
    pub enable_pageindex: Option<bool>,
}

/// POST /api/v1/batch-pipeline
pub async fn run_batch_pipeline(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(req): Json<BatchPipelineRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let batch_run_id = Uuid::new_v4().to_string();
    info!("Starting unified batch pipeline {}, req={:?}", batch_run_id, req);
    
    // Determine target sources
    let mut target_sources = vec![];
    if let Some(ids) = &req.source_ids {
        if !ids.is_empty() {
            target_sources = ids.clone();
        }
    }
    
    // Fallback: If empty but process_all is true
    if target_sources.is_empty() && req.process_all.unwrap_or(false) {
        let sources: Vec<(i64,)> = sqlx::query_as("SELECT id FROM data_sources WHERE tenant_id = ?")
            .bind(&tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();
        target_sources = sources.into_iter().map(|s| s.0).collect();
    }
    
    let req_arc = std::sync::Arc::new(req);
    
    for src_id in target_sources {
        let pool_c = pool.clone();
        let tenant_c = tenant_id.clone();
        let req_c = req_arc.clone();
        
        // Error handling for synchronous setup part of run_pipeline_for_source
        if let Err(e) = run_pipeline_for_source(pool_c, tenant_c, src_id, req_c).await {
            error!("Batch setup failed for source {}: {}", src_id, e);
        }
    }

    Ok(Json(json!({ 
        "message": "Batch engine triggered", 
        "batch_run_id": batch_run_id, 
        "status": "running" 
    })))
}

/// GET /api/v1/pipeline-overview
pub async fn get_pipeline_overview(
    _headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Fetch all sources with some basic info
    let sources_query = sqlx::query_as::<_, (i64, String)>(
        "SELECT id, name FROM data_sources ORDER BY id DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    let step_averages_rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT step_name, CAST(SUM(latency_ms) AS SIGNED), CAST(SUM(item_count) AS SIGNED) FROM pipeline_run_steps WHERE status = 'completed' AND latency_ms > 0 AND item_count > 0 GROUP BY step_name"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut step_avg_ms: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    step_avg_ms.insert("chunk_check".to_string(), 5);
    step_avg_ms.insert("embed_chunks".to_string(), 20);
    step_avg_ms.insert("pageindex_generation".to_string(), 15000);
    step_avg_ms.insert("kg_extraction".to_string(), 7500);
    step_avg_ms.insert("qa_extraction".to_string(), 7000);
    step_avg_ms.insert("auto_qc_filter".to_string(), 100);
    step_avg_ms.insert("qa_indexing".to_string(), 20);
    step_avg_ms.insert("graph_intelligence".to_string(), 1000);

    for (name, lat, count) in step_averages_rows {
        if count > 0 {
            step_avg_ms.insert(name, lat / count);
        }
    }

    let mut pending_sources_count = 0;
    let mut total_pending_ms = 0i64;

    let mut sources_out = vec![];
    for (id, name) in sources_query {
        let (chunks,): (i64,) = sqlx::query_as("SELECT count(*) FROM chunks WHERE source_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

        let (entities,): (i64,) = sqlx::query_as("SELECT count(*) FROM kg_entities WHERE source_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

        let (relations,): (i64,) = sqlx::query_as("SELECT count(*) FROM kg_relations WHERE source_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

        let (qa_pairs,): (i64,) = sqlx::query_as("SELECT count(*) FROM qa_results WHERE source_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

        let latest_run_opt: Option<(String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, status, UNIX_TIMESTAMP(started_at), UNIX_TIMESTAMP(finished_at) FROM pipeline_runs WHERE source_id = ? ORDER BY started_at DESC LIMIT 1"
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        let mut status = "never_run".to_string();
        let mut steps_out = vec![];
        let mut estimate_human: Option<String> = None;
        let mut actual_duration_ms: Option<i64> = None;
        let mut p_id: Option<String> = None;

        if let Some((run_id, run_status, start_ts, finish_ts)) = latest_run_opt {
            status = run_status;
            p_id = Some(run_id.clone());
            
            if status == "running" || status == "completed" || status == "failed" {
                if let (Some(st), Some(ft)) = (start_ts, finish_ts) {
                    actual_duration_ms = Some((ft - st) * 1000);
                }
            }
            let steps = sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT step_name, status, error_message FROM pipeline_run_steps WHERE run_id = ? ORDER BY step_number"
            )
            .bind(run_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            let mut source_estimate_ms = 0i64;

            for (step_name, mut step_status, err) in steps {
                if step_status == "skipped" {
                    let has_data = match step_name.as_str() {
                        "kg_extraction" | "graph_intelligence" => entities > 0,
                        "qa_extraction" | "auto_qc_filter" | "qa_indexing" => qa_pairs > 0,
                        "embed_chunks" | "pageindex_generation" => chunks > 0,
                        _ => false,
                    };
                    if has_data {
                        step_status = "completed (cached)".to_string();
                    }
                }
                
                let mut step_est_human = None;
                if status == "running" && (step_status == "pending" || step_status == "running") && chunks > 0 {
                    let avg = *step_avg_ms.get(&step_name).unwrap_or(&5000);
                    let items = match step_name.as_str() {
                        "pageindex_generation" | "graph_intelligence" => 1,
                        _ => chunks,
                    };
                    let step_ms = items * avg;
                    source_estimate_ms += step_ms;
                    
                    let s = step_ms / 1000;
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    step_est_human = Some(if h > 0 {
                        if m > 0 { format!("{}h {}m", h, m) } else { format!("{}h", h) }
                    } else if m > 0 {
                        format!("{}m", m)
                    } else {
                        "<1m".to_string()
                    });
                }

                steps_out.push(json!({
                    "name": step_name,
                    "status": step_status,
                    "error": err,
                    "estimate_human": step_est_human
                }));
            }
            
            if status == "running" && source_estimate_ms > 0 {
                let s = source_estimate_ms / 1000;
                let h = s / 3600;
                let m = (s % 3600) / 60;
                estimate_human = Some(if h > 0 {
                    if m > 0 { format!("{}h {}m", h, m) } else { format!("{}h", h) }
                } else if m > 0 {
                    format!("{}m", m)
                } else {
                    "<1m".to_string()
                });
                total_pending_ms += source_estimate_ms;
            }
        }
        if status != "completed" {
            pending_sources_count += 1;
        }

        sources_out.push(json!({
            "source_id": id,
            "name": name,
            "chunks": chunks,
            "kg": { "entities": entities, "relations": relations },
            "pipeline": { "status": status, "id": p_id },
            "steps": steps_out,
            "estimate_human": estimate_human,
            "actual_duration_ms": actual_duration_ms
        }));
    }
    let total_estimate_secs = total_pending_ms / 1000;
    let hours = total_estimate_secs / 3600;
    let minutes = (total_estimate_secs % 3600) / 60;
    
    let total_estimate_human = if total_estimate_secs == 0 {
        "0m".to_string()
    } else if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        "<1m".to_string()
    };

    Ok(Json(json!({ 
        "pending_sources": pending_sources_count,
        "total_sources": sources_out.len(),
        "total_estimate_human": total_estimate_human,
        "default_provider": "heimdall",
        "default_model": "mlx-community/gemma-4-31b-it-4bit",
        "sources": sources_out 
    })))
}

// ─── Cancel Pipeline Run ──────────────────────────────────────────────────────────

async fn cancel_pipeline_run(
    Path(run_id): Path<String>,
    State(pool): State<DbPool>,
) -> impl axum::response::IntoResponse {
    // 1. Abort the underlying task if it's currently running in memory
    if let Some((_, handle)) = PIPELINE_TASKS.remove(&run_id) {
        handle.abort();
        info!("🛑 Aborted in-memory task for pipeline run={}", run_id);
    } else {
        warn!("⚠️ Pipeline task {} not found in memory, it may have already finished or stopped.", run_id);
    }

    // 2. Mark as failed in the database
    let res = sqlx::query(
        "UPDATE pipeline_runs SET status = 'failed', error_message = 'Cancelled by user', finished_at = NOW() WHERE id = ?"
    )
    .bind(&run_id)
    .execute(&pool)
    .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({ "message": "Pipeline run forcefully cancelled." })))
        }
        Ok(_) => {
            (StatusCode::OK, Json(json!({ "message": "Pipeline forcefully marked as cancelled. No matching running record found." })))
        }
        Err(e) => {
            error!("Failed to update cancellation: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
        }
    }
}
