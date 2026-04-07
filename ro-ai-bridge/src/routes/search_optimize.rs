//! Search Optimize Route — AI-powered query optimization
//!
//! - POST /api/search/optimize — Generate optimized search query variants
//!
//! ISO 29110 — Task 2.2: AI Query Optimizer API

use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{error, info, warn};

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::llm_router::LlmRouter;

// ── Request / Response Types ──────────────────────────

/// Request body for POST /api/search/optimize
#[derive(Debug, Deserialize)]
pub struct OptimizeRequest {
    /// The original user query to optimize.
    pub query: String,
    /// Tenant ID override.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Optimization strategy: "expand", "rephrase", "decompose", "all" (default).
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Number of suggestions to generate. Default: 5, Max: 10.
    #[serde(default = "default_count")]
    pub count: usize,
    /// Override Provider
    pub provider: Option<String>,
    /// Override Model ID
    pub model_id: Option<String>,
}

fn default_strategy() -> String {
    "all".to_string()
}
fn default_count() -> usize {
    5
}

/// Response body for POST /api/search/optimize
#[derive(Debug, Serialize)]
pub struct OptimizeResponse {
    /// The original query submitted by the user.
    pub original_query: String,
    /// AI-generated optimized query variants.
    pub suggestions: Vec<QuerySuggestion>,
    /// Time spent generating suggestions.
    pub latency_ms: u64,
    /// Which LLM model was used.
    pub model_used: String,
}

/// A single optimized query suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySuggestion {
    /// The optimized query string.
    pub query: String,
    /// Strategy used: "keyword_expansion", "synonym", "decomposition", "semantic_rephrase"
    pub strategy: String,
    /// Why this version might retrieve better results.
    pub explanation: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f32,
}

// ── Constants ─────────────────────────────────────────

const MAX_SUGGESTION_COUNT: usize = 10;

/// System prompt template for the optimizer agent.
const OPTIMIZER_SYSTEM_PROMPT: &str = r#"You are a Search Query Optimizer for a RAG knowledge base containing medical, technical, and general documents.

Given a user's search query, generate {count} optimized search variants.
Each variant should use a DIFFERENT strategy from this list:
- keyword_expansion: Add related technical terms and domain-specific keywords
- synonym: Replace key terms with medical/domain synonyms and alternate phrasings
- decomposition: Break a complex question into focused sub-queries
- semantic_rephrase: Rephrase the question for better semantic embedding match

Return ONLY a valid JSON array. No markdown, no explanation outside JSON.
Format: [{"query": "...", "strategy": "...", "explanation": "...", "confidence": 0.0-1.0}]

Rules:
- Each suggestion must use a different strategy
- Confidence reflects how much improvement you expect (0.5 = moderate, 0.9 = high)
- Keep queries concise and searchable
- Preserve the original intent"#;

// ── Route Registration ───────────────────────────────

pub fn search_optimize_routes() -> Router<DbPool> {
    Router::new().route("/api/search/optimize", post(optimize_handler))
}

// ── Handler ──────────────────────────────────────────

/// POST /api/search/optimize — Generate AI-optimized query variants
async fn optimize_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<OptimizeRequest>,
) -> impl IntoResponse {
    let start = Instant::now();

    // Validate query
    let query = payload.query.trim().to_string();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Query must not be empty"
            })),
        )
            .into_response();
    }

    let count = payload.count.min(MAX_SUGGESTION_COUNT).max(1);

    // Resolve tenant
    let tenant_id = tenant_ctx
        .as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or(payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    // Build the system prompt with count
    let system_prompt = OPTIMIZER_SYSTEM_PROMPT.replace("{count}", &count.to_string());

    info!(
        event = "optimize",
        query = %query,
        count = count,
        strategy = %payload.strategy,
        "✨ /api/search/optimize"
    );

    // Resolve LLM client via LlmRouter
    let router = match LlmRouter::new(pool.clone(), &tenant_id).await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to init LlmRouter, using fallback");
            let latency_ms = start.elapsed().as_millis() as u64;
            return (
                StatusCode::OK,
                Json(OptimizeResponse {
                    original_query: query.clone(),
                    suggestions: fallback_suggestions(&query),
                    latency_ms,
                    model_used: "fallback".to_string(),
                }),
            )
                .into_response();
        }
    };

    let (client, model) = match router.resolve_client_with_overrides(
        "chat",
        payload.provider.as_deref(),
        payload.model_id.as_deref(),
    ) {
        Ok(cm) => cm,
        Err(e) => {
            warn!(error = %e, "Failed to resolve chat client, using fallback");
            let latency_ms = start.elapsed().as_millis() as u64;
            return (
                StatusCode::OK,
                Json(OptimizeResponse {
                    original_query: query.clone(),
                    suggestions: fallback_suggestions(&query),
                    latency_ms,
                    model_used: "fallback".to_string(),
                }),
            )
                .into_response();
        }
    };

    info!(model = %model, provider = %client.provider_name(), "Optimizer LLM resolved");

    // Call LLM via UniversalClient
    let user_input = format!("Optimize this search query:\n\n{}", query);
    let suggestions = match client
        .prompt(&model, &system_prompt, &user_input, 2000, 0.7)
        .await
    {
        Ok(raw) => parse_optimizer_response(&raw, count),
        Err(e) => {
            error!(error = %e, "Optimizer LLM call failed, returning fallback");
            fallback_suggestions(&query)
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    info!(
        event = "optimize_complete",
        suggestions_count = suggestions.len(),
        latency_ms = latency_ms,
        "✅ Optimizer completed"
    );

    (
        StatusCode::OK,
        Json(OptimizeResponse {
            original_query: query,
            suggestions,
            latency_ms,
            model_used: model,
        }),
    )
        .into_response()
}

/// Parse and validate the LLM's optimizer response JSON.
pub fn parse_optimizer_response(raw: &str, max_count: usize) -> Vec<QuerySuggestion> {
    // Strip markdown code fences if present
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<Vec<QuerySuggestion>>(cleaned) {
        Ok(mut suggestions) => {
            // Clamp confidence values to 0.0-1.0
            for s in &mut suggestions {
                s.confidence = s.confidence.clamp(0.0, 1.0);
            }
            suggestions.truncate(max_count);
            suggestions
        }
        Err(e) => {
            warn!(error = %e, raw = %raw, "Failed to parse optimizer response");
            vec![]
        }
    }
}

/// Generate basic fallback suggestions when LLM is unavailable.
fn fallback_suggestions(query: &str) -> Vec<QuerySuggestion> {
    vec![QuerySuggestion {
        query: query.to_string(),
        strategy: "original".to_string(),
        explanation: "Original query passed through (optimizer unavailable)".to_string(),
        confidence: 0.5,
    }]
}

// ── Tests (TDD — ISO 29110) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── OptimizeRequest deserialization ───────────────

    #[test]
    fn test_optimize_request_minimal() {
        let json = r#"{"query": "side effects aspirin"}"#;
        let req: OptimizeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "side effects aspirin");
        assert_eq!(req.strategy, "all");
        assert_eq!(req.count, 5);
    }

    #[test]
    fn test_optimize_request_full() {
        let json = r#"{
            "query": "aspirin interaction",
            "tenant_id": "megacare",
            "strategy": "expand",
            "count": 3
        }"#;
        let req: OptimizeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.strategy, "expand");
        assert_eq!(req.count, 3);
        assert_eq!(req.tenant_id, Some("megacare".to_string()));
    }

    // ── OptimizeResponse serialization ───────────────

    #[test]
    fn test_optimize_response_serialization() {
        let resp = OptimizeResponse {
            original_query: "test".to_string(),
            suggestions: vec![QuerySuggestion {
                query: "optimized test query".to_string(),
                strategy: "keyword_expansion".to_string(),
                explanation: "Added domain terms".to_string(),
                confidence: 0.85,
            }],
            latency_ms: 150,
            model_used: "gemini-2.5-flash".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["original_query"], "test");
        assert_eq!(json["suggestions"][0]["strategy"], "keyword_expansion");
        // f32 serialized to JSON has precision variance, use approximate check
        let conf = json["suggestions"][0]["confidence"].as_f64().unwrap();
        assert!(
            (conf - 0.85).abs() < 0.001,
            "Confidence should be ~0.85, got {}",
            conf
        );
        assert_eq!(json["model_used"], "gemini-2.5-flash");
    }

    // ── parse_optimizer_response ─────────────────────

    #[test]
    fn test_parse_valid_response() {
        let raw = r#"[
            {"query": "aspirin NSAID side effects", "strategy": "keyword_expansion", "explanation": "Added NSAID", "confidence": 0.85},
            {"query": "adverse reactions acetylsalicylic acid", "strategy": "synonym", "explanation": "Used chemical name", "confidence": 0.7}
        ]"#;

        let suggestions = parse_optimizer_response(raw, 5);
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].strategy, "keyword_expansion");
        assert_eq!(suggestions[1].strategy, "synonym");
    }

    #[test]
    fn test_parse_response_with_code_fences() {
        let raw = r#"```json
[{"query": "test", "strategy": "expand", "explanation": "expanded", "confidence": 0.8}]
```"#;

        let suggestions = parse_optimizer_response(raw, 5);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].query, "test");
    }

    #[test]
    fn test_parse_response_malformed() {
        let raw = "This is not valid JSON at all";
        let suggestions = parse_optimizer_response(raw, 5);
        assert!(
            suggestions.is_empty(),
            "Malformed response should return empty"
        );
    }

    #[test]
    fn test_parse_response_clamps_confidence() {
        let raw = r#"[
            {"query": "q1", "strategy": "s1", "explanation": "e1", "confidence": 1.5},
            {"query": "q2", "strategy": "s2", "explanation": "e2", "confidence": -0.3}
        ]"#;

        let suggestions = parse_optimizer_response(raw, 5);
        assert_eq!(suggestions[0].confidence, 1.0, "Should clamp to max 1.0");
        assert_eq!(suggestions[1].confidence, 0.0, "Should clamp to min 0.0");
    }

    #[test]
    fn test_parse_response_truncates_to_max() {
        let raw = r#"[
            {"query": "q1", "strategy": "s1", "explanation": "e1", "confidence": 0.9},
            {"query": "q2", "strategy": "s2", "explanation": "e2", "confidence": 0.8},
            {"query": "q3", "strategy": "s3", "explanation": "e3", "confidence": 0.7}
        ]"#;

        let suggestions = parse_optimizer_response(raw, 2);
        assert_eq!(suggestions.len(), 2, "Should truncate to max_count");
    }

    // ── QuerySuggestion ──────────────────────────────

    #[test]
    fn test_query_suggestion_round_trip() {
        let original = QuerySuggestion {
            query: "expanded query".to_string(),
            strategy: "keyword_expansion".to_string(),
            explanation: "More terms".to_string(),
            confidence: 0.88,
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: QuerySuggestion = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.query, original.query);
        assert_eq!(restored.confidence, original.confidence);
    }

    // ── fallback_suggestions ─────────────────────────

    #[test]
    fn test_fallback_returns_original() {
        let suggestions = fallback_suggestions("my query");
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].query, "my query");
        assert_eq!(suggestions[0].strategy, "original");
        assert!(suggestions[0].confidence > 0.0);
    }

    // ── Constants ────────────────────────────────────

    #[test]
    fn test_max_suggestion_count() {
        assert!(
            MAX_SUGGESTION_COUNT <= 20,
            "Max suggestions should be reasonable"
        );
    }

    #[test]
    fn test_system_prompt_has_placeholder() {
        assert!(
            OPTIMIZER_SYSTEM_PROMPT.contains("{count}"),
            "Prompt must have count placeholder"
        );
    }
}
