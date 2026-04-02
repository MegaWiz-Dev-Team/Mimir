//! Multi-provider extraction endpoint — KG entities + QA generation
//!
//! POST /api/v1/sources/:id/extract
//!   { extract_type: "kg" | "qa" | "both", provider, model, prompt_version?, run_label? }
//!
//! Supports: heimdall, gemini, ollama, openai — any OpenAI-compatible API.
//! Stores provider, model, prompt_version, run_label, latency_ms on every result.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::routes::sources::config::{
    call_llm_api_with_logging, infer_api_base, resolve_llm_credentials,
};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;

// ─── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExtractRequest {
    /// "kg", "qa", or "both"
    pub extract_type: String,
    /// Provider name: "heimdall", "gemini", "ollama", "openai"
    pub provider: Option<String>,
    /// Model identifier (e.g. "gemini-2.5-flash", "Qwen3.5-9B")
    pub model: Option<String>,
    /// Prompt version to use (e.g. "v1.0"). If None, uses active prompt.
    pub prompt_version: Option<String>,
    /// Label to group benchmark runs (e.g. "benchmark-round-1")
    pub run_label: Option<String>,
    /// Max chunks to process (default: all)
    pub max_chunks: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ExtractResult {
    pub source_id: i64,
    pub extract_type: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub run_label: Option<String>,
    pub chunks_processed: usize,
    pub kg_entities: usize,
    pub kg_relations: usize,
    pub qa_pairs: usize,
    pub total_latency_ms: i64,
    pub errors: Vec<String>,
}

// ─── Default Prompts ───────────────────────────────────────────────────────────

fn default_kg_system_prompt() -> String {
    mimir_core_ai::services::entity_extractor::build_extraction_system_prompt()
}

fn default_qa_system_prompt() -> &'static str {
    r#"You are a QA Generator. Given the following text content, generate 2-5 high-quality question-answer pairs that test understanding of the material.

Return STRICT JSON array:
[
  {"question": "...", "answer": "..."}
]

Rules:
1. Keep answers concise and factual
2. Only generate questions that can be directly answered from the given text
3. Prefer "why", "how", and "what" questions over yes/no
4. Cover different aspects of the text
5. Do NOT include explanations — ONLY the JSON array"#
}

// ─── Handler ───────────────────────────────────────────────────────────────────

/// POST /api/v1/sources/:id/extract
pub async fn extract_source(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(source_id): Path<i64>,
    Json(payload): Json<ExtractRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let extract_type = payload.extract_type.to_lowercase();

    if !["kg", "qa", "both"].contains(&extract_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "extract_type must be 'kg', 'qa', or 'both'"})),
        ));
    }

    // Verify source belongs to tenant
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
            Json(json!({"error": "Source not found or access denied"})),
        )
    })?;

    // Resolve provider + model
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
        payload.provider.as_deref(),
        payload.model.as_deref(),
    );
    let provider = resolved_slot.provider;
    let model = resolved_slot.model;

    let prompt_version = payload
        .prompt_version
        .clone()
        .unwrap_or_else(|| "v1.0".to_string());
    let run_label = payload.run_label.clone();

    // Resolve API credentials
    let api_base = infer_api_base(&provider);
    let api_key = resolve_api_key(&provider);

    // Fetch chunks for this source
    let max_chunks = payload.max_chunks.unwrap_or(usize::MAX);
    let chunks: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, content, token_count FROM chunks WHERE source_id = ? ORDER BY chunk_index ASC",
    )
    .bind(source_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let chunks_to_process: Vec<_> = chunks.into_iter().take(max_chunks).collect();

    if chunks_to_process.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No chunks found for this source. Run sync first."})),
        ));
    }

    let chunk_count = chunks_to_process.len();

    info!(
        "Starting extraction: source={} ({}) type={} provider={} model={} prompt={} chunks={}",
        source_id, source_name, extract_type, provider, model, prompt_version, chunk_count
    );

    // Spawn background extraction
    let pool_clone = pool.clone();
    let tenant_clone = tenant_id.clone();
    let extract_type_clone = extract_type.clone();
    let provider_clone = provider.clone();
    let model_clone = model.clone();
    let prompt_version_clone = prompt_version.clone();
    let run_label_clone = run_label.clone();
    let api_base_clone = api_base.clone();
    let api_key_clone = api_key.clone();

    tokio::spawn(async move {
        let mut total_entities = 0usize;
        let mut total_relations = 0usize;
        let mut total_qa = 0usize;
        let mut errors: Vec<String> = vec![];
        let total_start = std::time::Instant::now();

        for (chunk_id, content, _token_count) in &chunks_to_process {
            if content.trim().len() < 50 {
                continue;
            }

            // KG Extraction
            if extract_type_clone == "kg" || extract_type_clone == "both" {
                match extract_kg_from_chunk(
                    &pool_clone,
                    &tenant_clone,
                    source_id,
                    *chunk_id,
                    content,
                    &api_key_clone,
                    &api_base_clone,
                    &model_clone,
                    &provider_clone,
                    &prompt_version_clone,
                    &run_label_clone,
                )
                .await
                {
                    Ok((entities, relations)) => {
                        total_entities += entities;
                        total_relations += relations;
                    }
                    Err(e) => {
                        errors.push(format!("KG chunk {}: {}", chunk_id, e));
                    }
                }
            }

            // QA Generation
            if extract_type_clone == "qa" || extract_type_clone == "both" {
                match extract_qa_from_chunk(
                    &pool_clone,
                    &tenant_clone,
                    source_id,
                    *chunk_id,
                    content,
                    &api_key_clone,
                    &api_base_clone,
                    &model_clone,
                    &provider_clone,
                    &prompt_version_clone,
                    &run_label_clone,
                )
                .await
                {
                    Ok(count) => {
                        total_qa += count;
                    }
                    Err(e) => {
                        errors.push(format!("QA chunk {}: {}", chunk_id, e));
                    }
                }
            }
        }

        let total_ms = total_start.elapsed().as_millis() as i64;
        info!(
            "Extraction complete: source={} entities={} relations={} qa={} errors={} latency={}ms",
            source_id,
            total_entities,
            total_relations,
            total_qa,
            errors.len(),
            total_ms
        );
    });

    Ok(Json(json!({
        "success": true,
        "message": format!("Extraction started for source {} ({} chunks)", source_id, chunk_count),
        "source_id": source_id,
        "extract_type": extract_type,
        "provider": provider,
        "model": model,
        "prompt_version": prompt_version,
        "run_label": run_label,
        "chunks_count": chunk_count
    })))
}

// ─── KG Extraction ─────────────────────────────────────────────────────────────

async fn extract_kg_from_chunk(
    pool: &DbPool,
    tenant_id: &str,
    source_id: i64,
    chunk_id: i64,
    content: &str,
    api_key: &str,
    api_base: &str,
    model: &str,
    provider: &str,
    prompt_version: &str,
    run_label: &Option<String>,
) -> anyhow::Result<(usize, usize)> {
    let start = std::time::Instant::now();

    let system_prompt = default_kg_system_prompt();
    let user_prompt = format!(
        "{}\n\nExtract entities and relationships from this text:\n\n---\n{}\n---",
        system_prompt, content
    );

    let (response, _tokens) = call_llm_api_with_logging(
        api_key,
        api_base,
        model,
        &user_prompt,
        Some(pool),
        Some(tenant_id),
        Some(provider),
        Some("extract_kg"),
    )
    .await?;

    let latency_ms = start.elapsed().as_millis() as i32;

    // Parse JSON response
    let cleaned = response
        .trim_start_matches("```json")
        .trim_end_matches("```")
        .trim();
    let parsed: Value = serde_json::from_str(cleaned).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse KG JSON: {} — raw: {}",
            e,
            &cleaned[..cleaned.len().min(200)]
        )
    })?;

    let entities = parsed["entities"].as_array().map(|a| a.len()).unwrap_or(0);
    let relations = parsed["relations"].as_array().map(|a| a.len()).unwrap_or(0);

    // Store entities
    if let Some(ents) = parsed["entities"].as_array() {
        for ent in ents {
            let name = ent["name"].as_str().unwrap_or("");
            let etype = ent["type"].as_str().unwrap_or("Concept");
            let props = ent.get("properties").cloned().unwrap_or(json!({}));

            let _ = sqlx::query(
                r#"INSERT INTO kg_entities (tenant_id, source_id, name, entity_type, properties, provider, model, prompt_version, run_label, latency_ms)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                   ON DUPLICATE KEY UPDATE properties = VALUES(properties), provider = VALUES(provider), model = VALUES(model)"#
            )
            .bind(tenant_id)
            .bind(source_id)
            .bind(name)
            .bind(etype)
            .bind(props.to_string())
            .bind(provider)
            .bind(model)
            .bind(prompt_version)
            .bind(run_label)
            .bind(latency_ms)
            .execute(pool)
            .await;
        }
    }

    // Store relations
    if let Some(rels) = parsed["relations"].as_array() {
        for rel in rels {
            let from = rel["from"].as_str().unwrap_or("");
            let to = rel["to"].as_str().unwrap_or("");
            let rtype = rel["type"].as_str().unwrap_or("related_to");

            let _ = sqlx::query(
                "INSERT IGNORE INTO kg_relations (tenant_id, source_id, from_entity, to_entity, relation_type, provider, model, prompt_version, run_label)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(tenant_id)
            .bind(source_id)
            .bind(from)
            .bind(to)
            .bind(rtype)
            .bind(provider)
            .bind(model)
            .bind(prompt_version)
            .bind(run_label)
            .execute(pool)
            .await;
        }
    }

    info!(
        "KG chunk {}: {} entities, {} relations ({}ms)",
        chunk_id, entities, relations, latency_ms
    );
    Ok((entities, relations))
}

// ─── QA Generation ─────────────────────────────────────────────────────────────

async fn extract_qa_from_chunk(
    pool: &DbPool,
    tenant_id: &str,
    source_id: i64,
    chunk_id: i64,
    content: &str,
    api_key: &str,
    api_base: &str,
    model: &str,
    provider: &str,
    prompt_version: &str,
    run_label: &Option<String>,
) -> anyhow::Result<usize> {
    let start = std::time::Instant::now();

    let system_prompt = default_qa_system_prompt();
    let user_prompt = format!(
        "{}\n\nGenerate QA pairs from this content:\n\n{}",
        system_prompt, content
    );

    let (response, _tokens) = call_llm_api_with_logging(
        api_key,
        api_base,
        model,
        &user_prompt,
        Some(pool),
        Some(tenant_id),
        Some(provider),
        Some("extract_qa"),
    )
    .await?;

    let latency_ms = start.elapsed().as_millis() as i32;

    // Parse JSON array
    let cleaned = response
        .trim_start_matches("```json")
        .trim_end_matches("```")
        .trim();
    let qa_pairs: Vec<Value> = serde_json::from_str(cleaned).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse QA JSON: {} — raw: {}",
            e,
            &cleaned[..cleaned.len().min(200)]
        )
    })?;

    // Create pipeline run for provenance tracking
    let run_id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO pipeline_runs (id, status, provider, model) VALUES (?, 'COMPLETED', ?, ?)",
    )
    .bind(&run_id)
    .bind(provider)
    .bind(model)
    .execute(pool)
    .await;
    let _ = sqlx::query("INSERT INTO pipeline_steps (run_id, file_name, status, step_type) VALUES (?, ?, 'COMPLETED', 'GENERATE')")
        .bind(&run_id).bind(format!("chunk_{}", chunk_id)).execute(pool).await;

    let step_id: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM pipeline_steps WHERE run_id = ? LIMIT 1")
            .bind(&run_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    let mut count = 0;
    for qa in &qa_pairs {
        let question = qa["question"].as_str().unwrap_or("");
        let answer = qa["answer"].as_str().unwrap_or("");
        if !question.is_empty() && !answer.is_empty() {
            let _ = sqlx::query(
                r#"INSERT INTO qa_results (step_id, question, answer, context, tenant_id, provider, model, prompt_version, run_label, latency_ms)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
            )
            .bind(step_id.as_ref().map(|s| s.0).unwrap_or(0))
            .bind(question)
            .bind(answer)
            .bind(&content[..content.len().min(500)])
            .bind(tenant_id)
            .bind(provider)
            .bind(model)
            .bind(prompt_version)
            .bind(run_label)
            .bind(latency_ms)
            .execute(pool)
            .await;
            count += 1;
        }
    }

    // Update chunk metadata
    let _ = sqlx::query("UPDATE chunks SET metadata_json = JSON_SET(COALESCE(metadata_json, '{}'), '$.qa_status', 'completed', '$.qa_provider', ?, '$.qa_model', ?) WHERE id = ?")
        .bind(provider).bind(model).bind(chunk_id)
        .execute(pool).await;

    info!("QA chunk {}: {} pairs ({}ms)", chunk_id, count, latency_ms);
    Ok(count)
}

// ─── Helper ────────────────────────────────────────────────────────────────────

fn resolve_api_key(provider: &str) -> String {
    match provider {
        "gemini" | "google" => std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "heimdall" => std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
        _ => "ollama".to_string(),
    }
}
