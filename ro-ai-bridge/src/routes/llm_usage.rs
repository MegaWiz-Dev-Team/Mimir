use axum::{
    routing::get,
    Router, Json,
    extract::{State, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use mimir_core_ai::services::db::DbPool;

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LlmUsageLog {
    pub id: i64,
    pub tenant_id: String,
    pub model_id: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub caller: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    pub latency_ms: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Serialize)]
pub struct LlmUsageSummary {
    pub total_calls: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub avg_latency_ms: f64,
    pub estimated_cost_usd: f64,
    pub models: Vec<ModelUsageSummary>,
}

#[derive(Debug, Serialize)]
pub struct ModelUsageSummary {
    pub model_id: String,
    pub provider: String,
    pub total_calls: i64,
    pub total_tokens: i64,
    pub avg_latency_ms: f64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedUsageLogs {
    pub logs: Vec<LlmUsageLog>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize)]
pub struct UsageQueryParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub status: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SummaryQueryParams {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn llm_usage_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(get_llm_usage))
        .route("/summary", get(get_llm_usage_summary))
}

// ─── GET /api/v1/llm-usage ──────────────────────────────────────────────────────

async fn get_llm_usage(
    State(pool): State<DbPool>,
    Query(params): Query<UsageQueryParams>,
) -> Result<Json<PaginatedUsageLogs>, (StatusCode, Json<Value>)> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100).max(1);
    let offset = (page - 1) * per_page;

    // Build dynamic WHERE clause
    let mut conditions = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref model) = params.model_id {
        conditions.push("model_id = ?");
        bind_values.push(model.clone());
    }
    if let Some(ref provider) = params.provider {
        conditions.push("provider = ?");
        bind_values.push(provider.clone());
    }
    if let Some(ref status) = params.status {
        conditions.push("status = ?");
        bind_values.push(status.clone());
    }
    if let Some(ref date_from) = params.date_from {
        conditions.push("created_at >= ?");
        bind_values.push(date_from.clone());
    }
    if let Some(ref date_to) = params.date_to {
        conditions.push("created_at <= ?");
        bind_values.push(date_to.clone());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM llm_usage_logs {}", where_clause);
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    for val in &bind_values {
        count_query = count_query.bind(val);
    }
    let total: i64 = count_query
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // Data query
    let data_sql = format!(
        "SELECT id, tenant_id, model_id, provider, endpoint, caller, input_tokens, output_tokens, total_tokens, latency_ms, status, error_message, created_at FROM llm_usage_logs {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut data_query = sqlx::query_as::<_, LlmUsageLog>(&data_sql);
    for val in &bind_values {
        data_query = data_query.bind(val);
    }
    data_query = data_query.bind(per_page).bind(offset);

    let logs: Vec<LlmUsageLog> = data_query
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(PaginatedUsageLogs {
        logs,
        total,
        page,
        per_page,
    }))
}

// ─── GET /api/v1/llm-usage/summary ──────────────────────────────────────────────

async fn get_llm_usage_summary(
    State(pool): State<DbPool>,
    Query(params): Query<SummaryQueryParams>,
) -> Result<Json<LlmUsageSummary>, (StatusCode, Json<Value>)> {
    let mut conditions = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref date_from) = params.date_from {
        conditions.push("created_at >= ?");
        bind_values.push(date_from.clone());
    }
    if let Some(ref date_to) = params.date_to {
        conditions.push("created_at <= ?");
        bind_values.push(date_to.clone());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Overall aggregation
    let agg_sql = format!(
        "SELECT COUNT(*) as total_calls, COALESCE(SUM(input_tokens), 0) as total_input, COALESCE(SUM(output_tokens), 0) as total_output, COALESCE(SUM(total_tokens), 0) as total_tok, COALESCE(AVG(latency_ms), 0) as avg_lat FROM llm_usage_logs {}",
        where_clause
    );
    let mut agg_query = sqlx::query_as::<_, (i64, i64, i64, i64, f64)>(&agg_sql);
    for val in &bind_values {
        agg_query = agg_query.bind(val);
    }
    let (total_calls, total_input_tokens, total_output_tokens, total_tokens, avg_latency_ms) = agg_query
        .fetch_one(&pool)
        .await
        .unwrap_or((0, 0, 0, 0, 0.0));

    // Per-model aggregation
    let model_sql = format!(
        "SELECT model_id, provider, COUNT(*) as total_calls, COALESCE(SUM(total_tokens), 0) as total_tok, COALESCE(AVG(latency_ms), 0) as avg_lat FROM llm_usage_logs {} GROUP BY model_id, provider ORDER BY total_calls DESC",
        where_clause
    );
    let mut model_query = sqlx::query_as::<_, (String, String, i64, i64, f64)>(&model_sql);
    for val in &bind_values {
        model_query = model_query.bind(val);
    }
    let model_rows = model_query
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let models: Vec<ModelUsageSummary> = model_rows
        .into_iter()
        .map(|(model_id, provider, calls, tokens, lat)| {
            let cost = estimate_cost(&model_id, &provider, tokens);
            ModelUsageSummary {
                model_id,
                provider,
                total_calls: calls,
                total_tokens: tokens,
                avg_latency_ms: lat,
                estimated_cost_usd: cost,
            }
        })
        .collect();

    let estimated_cost_usd = models.iter().map(|m| m.estimated_cost_usd).sum();

    Ok(Json(LlmUsageSummary {
        total_calls,
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        avg_latency_ms,
        estimated_cost_usd,
        models,
    }))
}

// ─── Cost Estimation ─────────────────────────────────────────────────────────────

/// Estimate cost in USD based on model and token count.
/// Uses approximate per-1M-token pricing.
fn estimate_cost(model_id: &str, _provider: &str, total_tokens: i64) -> f64 {
    let per_million = match model_id {
        m if m.contains("gpt-4o") => 5.0,
        m if m.contains("gpt-4") => 30.0,
        m if m.contains("gpt-3.5") => 0.5,
        m if m.contains("gemini-2.5-pro") => 1.25,
        m if m.contains("gemini-2.5-flash") => 0.15,
        m if m.contains("gemini-2.0-flash") => 0.10,
        m if m.contains("gemini") => 0.50,
        m if m.contains("claude-3.5") => 3.0,
        m if m.contains("claude-3") => 15.0,
        // Local models (Ollama) are free
        m if m.contains("llama") => 0.0,
        m if m.contains("mistral") => 0.0,
        m if m.contains("qwen") => 0.0,
        _ => 0.0,
    };
    (total_tokens as f64 / 1_000_000.0) * per_million
}

// ─── Helper: Insert LLM Usage Log ────────────────────────────────────────────────

/// Public helper to insert a usage log entry from anywhere in the application.
pub async fn insert_llm_usage_log(
    pool: &DbPool,
    tenant_id: &str,
    model_id: &str,
    provider: &str,
    endpoint: Option<&str>,
    caller: Option<&str>,
    input_tokens: i32,
    output_tokens: i32,
    total_tokens: i32,
    latency_ms: i32,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO llm_usage_logs (tenant_id, model_id, provider, endpoint, caller, input_tokens, output_tokens, total_tokens, latency_ms, status, error_message) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(tenant_id)
    .bind(model_id)
    .bind(provider)
    .bind(endpoint)
    .bind(caller)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(total_tokens)
    .bind(latency_ms)
    .bind(status)
    .bind(error_message)
    .execute(pool)
    .await?;

    Ok(())
}
