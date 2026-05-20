//! Insurance underwriting RAG + report routes.
//!
//! `POST /api/v1/insurance/rag/search` is what the Asgard Underwriter app
//! (`asgard-underwriter/iris`) calls when assessing policy fit for a given
//! ICD-10 / medical-history query. Returns top-k chunks from the insurance
//! products collection (Prudential PDFs + web-scraped product pages) ranked
//! by BGE-M3 cosine similarity.
//!
//! ## Qdrant namespace split
//!
//! Medical KBs (PrimeKG, ICD-10, LOINC, clinical-wisdom) live in
//! `asgard-infra` Qdrant, pointed at by `QDRANT_URL`. Insurance product
//! chunks live in the `asgard` ns Qdrant. We read `INSURANCE_QDRANT_URL`
//! first; if unset, fall back to `QDRANT_URL` (works for single-cluster
//! dev deployments where everything lives in one Qdrant).
//!
//! ## Tenant filter
//!
//! Hardcoded `tenant_id=asgard_insurance` in the Qdrant payload filter.
//! Cross-tenant data leakage is impossible because the filter applies
//! at retrieval time — even if the JWT context is missing, no medical-
//! tenant chunks could ever match.

use axum::{routing::post, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

pub fn insurance_routes() -> Router<DbPool> {
    Router::new()
        .route("/rag/search", post(rag_search))
        .route("/report/generate", post(generate_report))
}

#[derive(Deserialize)]
pub struct RagSearchRequest {
    pub query: String,
    /// Optional override for top-k. Defaults to 5 — the underwriter UI
    /// shows ~3-5 policy citations per decision.
    #[serde(default)]
    pub k: Option<u32>,
}

#[derive(Serialize)]
pub struct RagSearchResponse {
    pub status: String,
    pub results: Vec<Value>,
}

const INSURANCE_COLLECTION: &str = "insurance_products_001";
const INSURANCE_TENANT_ID: &str = "asgard_insurance";
const DEFAULT_K: u32 = 5;
const DEFAULT_SCORE_FLOOR: f64 = 0.40;

/// Semantic search over the insurance product chunks (Prudential PDFs +
/// web-scraped product pages). Embed via Heimdall BGE-M3, retrieve via
/// Qdrant, filter by `tenant_id=asgard_insurance`.
async fn rag_search(Json(payload): Json<RagSearchRequest>) -> Json<RagSearchResponse> {
    let query = payload.query.trim();
    let k = payload.k.unwrap_or(DEFAULT_K).clamp(1, 20) as i64;

    if query.is_empty() {
        return Json(RagSearchResponse {
            status: "error".into(),
            results: vec![json!({"error": "query must not be empty"})],
        });
    }

    info!(query = %query, k = k, "insurance.rag_search");

    match embed_and_qdrant_insurance(query, k).await {
        Ok(hits) => Json(RagSearchResponse {
            status: "success".into(),
            results: hits,
        }),
        Err(e) => {
            warn!(error = %e, "insurance.rag_search failed");
            Json(RagSearchResponse {
                status: "error".into(),
                results: vec![json!({"error": e})],
            })
        }
    }
}

async fn embed_and_qdrant_insurance(text: &str, k: i64) -> Result<Vec<Value>, String> {
    let heimdall_url = std::env::var("HEIMDALL_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080/v1".into());
    let heimdall_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_default();
    // Insurance Qdrant URL — separate from the medical-KB cluster.
    // Falls back to QDRANT_URL for single-cluster deployments.
    let qdrant_url = std::env::var("INSURANCE_QDRANT_URL")
        .or_else(|_| std::env::var("QDRANT_URL"))
        .unwrap_or_else(|_| "http://localhost:6333".into());
    let embed_model = std::env::var("EMBED_MODEL")
        .unwrap_or_else(|_| "BAAI/bge-m3".into());
    let score_floor: f64 = std::env::var("INSURANCE_SCORE_FLOOR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SCORE_FLOOR);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // 1. Embed query via Heimdall (BGE-M3 1024-d, matches the collection
    //    vector size — the ingest script uses the same model)
    let embed_resp: Value = client
        .post(format!(
            "{}/embeddings",
            heimdall_url.trim_end_matches('/')
        ))
        .bearer_auth(&heimdall_key)
        .json(&json!({"model": embed_model, "input": text}))
        .send()
        .await
        .map_err(|e| format!("heimdall embed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("heimdall embed status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("heimdall embed parse: {e}"))?;

    let vector: Vec<f32> = embed_resp
        .pointer("/data/0/embedding")
        .and_then(|v| v.as_array())
        .ok_or("missing data[0].embedding")?
        .iter()
        .filter_map(|v| v.as_f64().map(|x| x as f32))
        .collect();

    // 2. Qdrant search with tenant filter. The ingest writes
    //    `tenant_id` into payload (top-level + nested in metadata);
    //    we filter on the top-level key for performance (no JSON-path
    //    descent needed at search time).
    let qd: Value = client
        .post(format!(
            "{}/collections/{}/points/search",
            qdrant_url.trim_end_matches('/'),
            INSURANCE_COLLECTION
        ))
        .json(&json!({
            "vector": vector,
            "limit": k,
            "with_payload": true,
            "score_threshold": score_floor,
            "filter": {
                "must": [
                    {"key": "tenant_id", "match": {"value": INSURANCE_TENANT_ID}}
                ]
            }
        }))
        .send()
        .await
        .map_err(|e| format!("qdrant search: {e}"))?
        .error_for_status()
        .map_err(|e| format!("qdrant search status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("qdrant search parse: {e}"))?;

    let hits = qd
        .get("result")
        .and_then(|v| v.as_array())
        .ok_or("missing result array")?;

    // Reshape into the response the underwriter expects: `source`,
    // `content`, `relevance_score` (kept from the legacy mock so iris
    // doesn't need to change deserializers). Extra fields surfaced so
    // the UI can render product context (insurer_id, document_kind).
    Ok(hits
        .iter()
        .map(|h| {
            let p = h.get("payload").cloned().unwrap_or(json!({}));
            let meta = p.get("metadata").cloned().unwrap_or(json!({}));
            let product_name = meta
                .get("product_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let document_kind = meta
                .get("document_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let source_file = meta
                .get("source_file")
                .and_then(|v| v.as_str())
                .or_else(|| meta.get("source_url").and_then(|v| v.as_str()))
                .unwrap_or("");
            let source_label = if !product_name.is_empty() {
                if !document_kind.is_empty() {
                    format!("{product_name} ({document_kind})")
                } else {
                    product_name.to_string()
                }
            } else {
                source_file.to_string()
            };
            json!({
                "source": source_label,
                "content": p.get("content"),
                "relevance_score": h.get("score"),
                "source_id": p.get("source_id"),
                "insurer_id": p.get("insurer_id"),
                "product_type": p.get("product_type"),
                "language": p.get("language"),
                "document_kind": document_kind,
                "source_file": source_file,
            })
        })
        .collect())
}

#[derive(Deserialize)]
pub struct GenerateReportRequest {
    pub decision: String,
    pub reasoning: String,
    pub conditions: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct GenerateReportResponse {
    pub status: String,
    pub report_id: String,
    pub recorded_decision: String,
}

/// Mock core system to generate a formal underwriting decision report (eBao stub)
async fn generate_report(Json(payload): Json<GenerateReportRequest>) -> Json<GenerateReportResponse> {
    info!(
        "Generating Underwriting Report. Decision: {} - Reasoning: {} - Conditions: {:?}",
        payload.decision, payload.reasoning, payload.conditions
    );

    let report_id = Uuid::new_v4().to_string();

    Json(GenerateReportResponse {
        status: "report_generated".into(),
        report_id,
        recorded_decision: payload.decision,
    })
}
