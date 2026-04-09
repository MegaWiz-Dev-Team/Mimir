use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info};

use crate::routes::sources::config::{call_llm_api_with_logging, infer_api_base};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;

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
        "heimdall" => std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
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

    let api_base = infer_api_base(&provider);
    let api_key = resolve_api_key(&provider);

    // Fetch chunks
    let chunks: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, content, chunk_index FROM chunks WHERE source_id = ? ORDER BY chunk_index ASC",
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

    if chunks.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No chunks found."})),
        ));
    }

    info!(
        "Starting PageIndex Extraction: source={} provider={} model={}",
        source_id, provider, model
    );

    let provider_clone = provider.clone();
    let model_clone = model.clone();

    tokio::spawn(async move {
        match generate_tree(
            &pool,
            &tenant_id,
            source_id,
            &api_key,
            &api_base,
            &model_clone,
            &provider_clone,
        )
        .await
        {
            Ok(_) => info!("PageIndex successfully generated for source {}", source_id),
            Err(e) => error!(
                "PageIndex generation failed for source {}: {}",
                source_id, e
            ),
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
    api_key: &str,
    api_base: &str,
    model: &str,
    provider: &str,
) -> anyhow::Result<()> {
    // 1. Fetch chunks internally
    let chunks: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, content, chunk_index FROM chunks WHERE source_id = ? ORDER BY chunk_index ASC",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?;

    if chunks.is_empty() {
        return Ok(());
    }

    let max_index = chunks.last().and_then(|c| c.2).unwrap_or(0);

    // 2. Decide Strategy (Map-Reduce vs Full Context)
    let final_content = if chunks.len() <= 10 {
        // Fast path for small docs (< 10 chunks)
        let mut text = String::new();
        for (_id, content, c_index) in &chunks {
            let idx = c_index.unwrap_or(0);
            text.push_str(&format!("\n--- [Page/Chunk {}] ---\n{}\n", idx, content));
        }
        text
    } else {
        // Map-Reduce path for large docs
        let batch_size = 8;
        let mut mapped_summaries = String::new();
        
        info!("Document has {} chunks. Using Map-Reduce for PageIndex.", chunks.len());

        for (i, chunk_group) in chunks.chunks(batch_size).enumerate() {
            let mut batch_text = String::new();
            for (_id, content, c_index) in chunk_group {
                let idx = c_index.unwrap_or(0);
                batch_text.push_str(&format!("\n--- [Page/Chunk {}] ---\n{}\n", idx, content));
            }
            
            let map_system = "You are a document outliner. Analyze the provided text segment and extract the main headings, topics, and a brief description. IMPORTANT: You MUST preserve the `[Page/Chunk X]` references for each topic so we know exactly where it is located. Be extremely concise. Use bullet points.";
            let prompt = format!("{}\n\nDocument Segment:\n{}", map_system, batch_text);
            
            info!("Running Map Step {}/{}", i + 1, (chunks.len() as f32 / batch_size as f32).ceil() as i32);
            let (summary, _) = call_llm_api_with_logging(
                api_key, api_base, model, &prompt,
                Some(pool), Some(tenant_id), Some(provider), Some("pageindex_map")
            ).await?;
            
            mapped_summaries.push_str(&format!("\n=== Segment {} Outline ===\n{}\n", i + 1, summary));
        }
        
        mapped_summaries
    };

    let system_prompt = format!(
        r#"You are a document structuring assistant. Analyze the provided document, which is divided into sections annotated with [Page/Chunk X]. 
Your objective is to generate a hierarchical "PageIndex" semantic tree that organizes the document's content logically based on its major headings and conceptual flow.

Rules:
1. Identify the HIGH-LEVEL logical structure (e.g., Chapters, Major Sections).
2. For each node, extract a clear `title`.
3. Provide a concise `summary` (1-2 sentences) of what the node covers.
4. Specify the `start_index` and `end_index` using the [Page/Chunk X] annotations.
5. Generate a unique `node_id` strings (e.g., "0001", "0002") sequentially.
6. Provide child nodes under `nodes` array if applicable. Max depth: 2 levels.
7. CRITICAL: The root node's `start_index` MUST be 0 and `end_index` MUST be {}.
8. CRITICAL: Every chunk index from 0 to {} MUST be covered by at least one node. Do not skip any sections.
9. To fit within token limits, condense your output: do NOT create hundreds of nodes. Group vast ranges of chunks (e.g., 20-50 chunks per leaf node) into high-level concepts. Avoid creating a node for every small subsection.
10. Child node ranges must be strictly within their parent's range and `start_index` must be <= `end_index`.
11. CRITICAL: Write the `title` and `summary` in the exact same language as the original source text (e.g., Thai).
    
Output EXCLUSIVELY a JSON object with this shape:
{{
  "node_id": "root",
  "title": "Document Root",
  "start_index": 0,
  "end_index": {},
  "summary": "Main document summary",
  "nodes": [
     {{ "node_id": "0001", "title": "Chap 1", "start_index": 0, "end_index": 5, "summary": "...", "nodes": [] }}
  ]
}}
Do not include markdown blocks or any other text before/after the JSON.
CRITICAL: Output ONLY valid JSON. All JSON keys MUST be quoted strings.
Do NOT include <think>, <reasoning>, or XML-like tags."#,
        max_index, max_index, max_index
    );

    let user_prompt = format!("{}\n\nDocument Context:\n{}", system_prompt, final_content);

    info!("Running Final Reduce Step for PageIndex (max_index={})", max_index);
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
    )
    .await?;

    let no_think = {
        let s = response.as_str();
        if let Some(end) = s.find("</think>") { s[end + 8..].to_string() } else { s.to_string() }
    };
    let cleaned = no_think.trim_start_matches("```json").trim_start_matches("```")
        .trim_end_matches("```").trim().to_string();
    let json_str = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        cleaned[start..=end].to_string()
    } else { cleaned };
    let mut parsed: Value = serde_json::from_str(&json_str)
        .map_err(|e| {
            let preview: String = json_str.chars().take(500).collect();
            tracing::warn!("PageIndex JSON parse failed. Preview: {}", preview);
            anyhow::anyhow!("Failed to parse PageIndex JSON: {}", e)
        })?;

    // Fix inverted ranges (start_index > end_index) recursively
    fn fix_ranges(node: &mut Value) {
        if let (Some(start), Some(end)) = (node["start_index"].as_i64(), node["end_index"].as_i64())
        {
            if start > end {
                node["start_index"] = serde_json::json!(end);
                node["end_index"] = serde_json::json!(start);
            }
        }
        if let Some(children) = node["nodes"].as_array_mut() {
            for child in children.iter_mut() {
                fix_ranges(child);
            }
        }
    }
    fix_ranges(&mut parsed);

    let fixed_json = serde_json::to_string(&parsed)
        .map_err(|e| anyhow::anyhow!("Failed to serialize fixed PageIndex: {}", e))?;

    // Store in database
    let _ = sqlx::query("UPDATE data_sources SET pageindex_tree = ? WHERE id = ?")
        .bind(&fixed_json)
        .bind(source_id)
        .execute(pool)
        .await?;

    Ok(())
}
