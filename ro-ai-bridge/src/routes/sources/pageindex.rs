use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, error};

use mimir_core_ai::services::db::DbPool;
use crate::routes::tenant::extract_tenant_id;
use crate::routes::sources::config::{call_llm_api_with_logging, infer_api_base};

#[derive(Debug, Deserialize)]
pub struct PageIndexRequest {
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PageIndexNode {
    pub node_id: String,
    pub title: String,
    pub start_index: usize,
    pub end_index: usize,
    pub summary: String,
    pub nodes: Option<Vec<PageIndexNode>>,
}

fn resolve_api_key(provider: &str) -> String {
    match provider.to_lowercase().as_str() {
        "gemini" | "google" => std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "heimdall" => std::env::var("HEIMDALL_API_KEY")
            .unwrap_or_else(|_| "hml-REDACTED".to_string()),
        _ => "ollama".to_string(),
    }
}

pub async fn extract_pageindex_route(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(source_id): Path<i64>,
    Json(payload): Json<PageIndexRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    // Verify source
    let source: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE id = ? AND tenant_id = ?"
    )
    .bind(source_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let (_, source_name) = source.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Source not found or access denied"})))
    })?;

    let provider = payload.provider.unwrap_or_else(|| "gemini".to_string());
    let model = payload.model.unwrap_or_else(|| "gemini-2.5-flash".to_string());
    
    let api_base = infer_api_base(&provider);
    let api_key = resolve_api_key(&provider);

    // Fetch chunks
    let chunks: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, content, chunk_index FROM chunks WHERE source_id = ? ORDER BY chunk_index ASC"
    )
    .bind(source_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if chunks.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "No chunks found."}))));
    }

    info!("Starting PageIndex Extraction: source={} provider={} model={}", source_id, provider, model);

    // Prepare full text representation
    // In a production scenario with gemini-2.5-pro or 3.1-flash-lite, we can pass up to 1M or 2M tokens.
    // We will concatenate the chunks and ask for a semantic tree.
    let mut full_text = String::new();
    for (_id, content, c_index) in &chunks {
        let idx = c_index.unwrap_or(0);
        full_text.push_str(&format!("\n--- [Page/Chunk {}] ---\n{}\n", idx, content));
    }

    let provider_clone = provider.clone();
    let model_clone = model.clone();

    tokio::spawn(async move {
        match generate_tree(&pool, &tenant_id, source_id, &full_text, &api_key, &api_base, &model_clone, &provider_clone).await {
            Ok(_) => info!("PageIndex successfully generated for source {}", source_id),
            Err(e) => error!("PageIndex generation failed for source {}: {}", source_id, e),
        }
    });

    Ok(Json(json!({
        "success": true,
        "message": format!("PageIndex generation task started for {}", source_name),
        "source_id": source_id,
        "provider": provider,
        "model": model,
    })))
}

pub async fn generate_tree(
    pool: &DbPool,
    tenant_id: &str,
    source_id: i64,
    content: &str,
    api_key: &str,
    api_base: &str,
    model: &str,
    provider: &str,
) -> anyhow::Result<()> {
    let system_prompt = r#"You are a document structuring assistant. Analyze the provided document, which is divided into sections annotated with [Page/Chunk X]. 
Your objective is to generate a hierarchical "PageIndex" semantic tree that organizes the document's content logically based on its major headings and conceptual flow.

Rules:
1. Identify the logical structure (e.g., Chapters, Sections, Subsections).
2. For each node, extract a clear `title`.
3. Provide a concise `summary` (1-2 sentences) of what the node covers.
4. Specify the `start_index` and `end_index` using the [Page/Chunk X] annotations.
5. Generate a unique `node_id` strings (e.g., "0001", "0002") sequentially.
6. Provide child nodes under `nodes` array if applicable. Max depth: 3 levels.
    
Output EXCLUSIVELY a JSON object with this shape:
{
  "node_id": "root",
  "title": "Document Root",
  "start_index": 0,
  "end_index": 100,
  "summary": "Main document summary",
  "nodes": [
     { "node_id": "0001", "title": "Chap 1", "start_index": 0, "end_index": 5, "summary": "...", "nodes": [] }
  ]
}
Do not include markdown blocks or any other text before/after the JSON."#;

    let user_prompt = format!("{}\n\nDocument Context:\n{}", system_prompt, content);

    // Send to LLM
    let (response, _tokens) = call_llm_api_with_logging(
        api_key,
        api_base,
        model,
        &user_prompt,
        Some(pool),
        Some(tenant_id),
        Some(provider),
        Some("extract_pageindex"),
    ).await?;

    let cleaned = response.trim_start_matches("```json").trim_end_matches("```").trim();
    
    // Validate JSON
    let _parsed: Value = serde_json::from_str(cleaned)
        .map_err(|e| anyhow::anyhow!("Failed to parse PageIndex JSON: {}", e))?;

    // Store in database
    let _ = sqlx::query(
        "UPDATE data_sources SET pageindex_tree = ? WHERE id = ?"
    )
    .bind(cleaned)
    .bind(source_id)
    .execute(pool)
    .await?;

    Ok(())
}
