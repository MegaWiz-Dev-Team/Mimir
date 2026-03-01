//! Budget & Alerts — per-model token budget config, usage alerts, benchmark reports
//!
//! Endpoints:
//! - GET    /api/v1/settings/llm-budget    — get budget config
//! - PUT    /api/v1/settings/llm-budget    — save budget config
//! - GET    /api/v1/llm-usage/alerts       — get usage alerts
//! - GET    /api/v1/llm-usage/benchmark    — benchmark report

use crate::routes::tenant::extract_tenant_id;use axum::{
    routing::{get, put},
    Router, Json,
    extract::{State, Query},
    http::{StatusCode, HeaderMap},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{info, error};

use mimir_core_ai::services::db::DbPool;

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct BudgetConfig {
    pub id: i64,
    pub tenant_id: String,
    pub model_id: String,
    pub daily_token_limit: i64,
    pub alert_threshold_pct: i32,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct SaveBudgetRequest {
    pub budgets: Vec<BudgetEntry>,
}

#[derive(Debug, Deserialize)]
pub struct BudgetEntry {
    pub model_id: String,
    pub daily_token_limit: i64,
    pub alert_threshold_pct: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UsageAlert {
    pub alert_type: String,
    pub model_id: String,
    pub message: String,
    pub severity: String,
    pub current_value: f64,
    pub threshold: f64,
}

#[derive(Debug, Serialize)]
pub struct BenchmarkEntry {
    pub model_id: String,
    pub provider: String,
    pub total_calls: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub avg_tokens_per_call: f64,
    pub total_tokens: i64,
    pub estimated_cost: f64,
}

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn budget_settings_routes() -> Router<DbPool> {
    Router::new()
        .route("/llm-budget", get(get_budget).put(save_budget))
}

pub fn budget_usage_routes() -> Router<DbPool> {
    Router::new()
        .route("/alerts", get(get_alerts))
        .route("/benchmark", get(get_benchmark))
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/v1/settings/llm-budget — Get all budget configs
async fn get_budget(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<BudgetConfig>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let budgets = sqlx::query_as::<_, BudgetConfig>(
        "SELECT * FROM llm_budget_configs WHERE tenant_id = ? ORDER BY model_id"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(budgets))
}

/// PUT /api/v1/settings/llm-budget — Save budget configs (upsert)
async fn save_budget(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<SaveBudgetRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let mut saved = 0;

    for entry in &payload.budgets {
        let threshold = entry.alert_threshold_pct.unwrap_or(80);

        sqlx::query(
            r#"INSERT INTO llm_budget_configs (tenant_id, model_id, daily_token_limit, alert_threshold_pct)
            VALUES (?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE daily_token_limit = VALUES(daily_token_limit), alert_threshold_pct = VALUES(alert_threshold_pct)"#
        )
        .bind(tenant_id)
        .bind(&entry.model_id)
        .bind(entry.daily_token_limit)
        .bind(threshold)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

        saved += 1;
    }

    info!("Saved {} budget configs", saved);
    Ok(Json(json!({"status": "ok", "saved": saved})))
}

/// GET /api/v1/llm-usage/alerts — Get current usage alerts
async fn get_alerts(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<UsageAlert>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let mut alerts: Vec<UsageAlert> = vec![];

    // Check budget limits
    let budgets = sqlx::query_as::<_, BudgetConfig>(
        "SELECT * FROM llm_budget_configs WHERE tenant_id = ? AND daily_token_limit > 0"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for budget in &budgets {
        let usage: (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(total_tokens), 0) FROM llm_usage_logs WHERE tenant_id = ? AND model_id = ? AND DATE(created_at) = CURDATE()"
        )
        .bind(tenant_id)
        .bind(&budget.model_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        let usage_pct = if budget.daily_token_limit > 0 {
            (usage.0 as f64 / budget.daily_token_limit as f64) * 100.0
        } else {
            0.0
        };

        if usage_pct >= 100.0 {
            alerts.push(UsageAlert {
                alert_type: "budget_exceeded".into(),
                model_id: budget.model_id.clone(),
                message: format!("Daily token limit exceeded for {} ({}/{})", budget.model_id, usage.0, budget.daily_token_limit),
                severity: "critical".into(),
                current_value: usage_pct,
                threshold: 100.0,
            });
        } else if usage_pct >= budget.alert_threshold_pct as f64 {
            alerts.push(UsageAlert {
                alert_type: "budget_warning".into(),
                model_id: budget.model_id.clone(),
                message: format!("Token usage at {:.0}% for {} ({}/{})", usage_pct, budget.model_id, usage.0, budget.daily_token_limit),
                severity: "warning".into(),
                current_value: usage_pct,
                threshold: budget.alert_threshold_pct as f64,
            });
        }
    }

    // Check for error rate spikes (>10% in last hour)
    let error_stats: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"SELECT model_id,
            SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END) as errors,
            COUNT(*) as total
        FROM llm_usage_logs
        WHERE tenant_id = ? AND created_at >= DATE_SUB(NOW(), INTERVAL 1 HOUR)
        GROUP BY model_id
        HAVING total > 5"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (model_id, errors, total) in &error_stats {
        let error_rate = (*errors as f64 / *total as f64) * 100.0;
        if error_rate > 10.0 {
            alerts.push(UsageAlert {
                alert_type: "error_rate".into(),
                model_id: model_id.clone(),
                message: format!("High error rate for {}: {:.0}% ({}/{})", model_id, error_rate, errors, total),
                severity: "warning".into(),
                current_value: error_rate,
                threshold: 10.0,
            });
        }
    }

    // Check for latency spikes (avg > 10s in last hour)
    let latency_stats: Vec<(String, f64)> = sqlx::query_as(
        r#"SELECT model_id, AVG(latency_ms) as avg_latency
        FROM llm_usage_logs
        WHERE tenant_id = ? AND created_at >= DATE_SUB(NOW(), INTERVAL 1 HOUR) AND status = 'success'
        GROUP BY model_id
        HAVING avg_latency > 10000"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (model_id, avg_latency) in &latency_stats {
        alerts.push(UsageAlert {
            alert_type: "latency_spike".into(),
            model_id: model_id.clone(),
            message: format!("High average latency for {}: {:.0}ms", model_id, avg_latency),
            severity: "warning".into(),
            current_value: *avg_latency,
            threshold: 10000.0,
        });
    }

    Ok(Json(alerts))
}

/// GET /api/v1/llm-usage/benchmark — Model benchmark report
async fn get_benchmark(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<BenchmarkEntry>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Get benchmark data per model
    let stats: Vec<(String, String, i64, i64, f64, i64, i64)> = sqlx::query_as(
        r#"SELECT
            model_id,
            provider,
            COUNT(*) as total_calls,
            SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as success_count,
            AVG(latency_ms) as avg_latency,
            SUM(total_tokens) as total_tokens,
            AVG(total_tokens) as avg_tokens
        FROM llm_usage_logs
        WHERE tenant_id = ?
        GROUP BY model_id, provider
        ORDER BY total_calls DESC"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let mut benchmarks: Vec<BenchmarkEntry> = vec![];

    for (model_id, provider, total, success, avg_lat, total_tok, avg_tok) in &stats {
        // Get p50 and p95 latency using ordered subquery
        let latencies: Vec<(i32,)> = sqlx::query_as(
            "SELECT latency_ms FROM llm_usage_logs WHERE tenant_id = ? AND model_id = ? AND status = 'success' ORDER BY latency_ms"
        )
        .bind(tenant_id)
        .bind(model_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let p50 = if !latencies.is_empty() {
            latencies[latencies.len() / 2].0 as f64
        } else { 0.0 };

        let p95 = if !latencies.is_empty() {
            let idx = ((latencies.len() as f64) * 0.95) as usize;
            latencies[idx.min(latencies.len() - 1)].0 as f64
        } else { 0.0 };

        // Estimate cost (reuse logic from llm_usage)
        let cost_per_1k = estimate_cost(model_id, *total_tok);

        benchmarks.push(BenchmarkEntry {
            model_id: model_id.clone(),
            provider: provider.clone(),
            total_calls: *total,
            success_rate: if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 },
            avg_latency_ms: *avg_lat,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            avg_tokens_per_call: *avg_tok as f64,
            total_tokens: *total_tok,
            estimated_cost: cost_per_1k,
        });
    }

    Ok(Json(benchmarks))
}

/// Rough cost estimation per model
fn estimate_cost(model_id: &str, total_tokens: i64) -> f64 {
    let per_1m = match model_id {
        m if m.starts_with("gpt-4") => 30.0,
        m if m.starts_with("gpt-3.5") => 2.0,
        m if m.starts_with("gemini-2.5-pro") => 10.0,
        m if m.starts_with("gemini") => 0.5,
        _ => 0.0, // Local models: free
    };
    (total_tokens as f64 / 1_000_000.0) * per_1m
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost() {
        assert_eq!(estimate_cost("llama3.2", 1_000_000), 0.0);
        assert!((estimate_cost("gemini-2.0-flash", 1_000_000) - 0.5).abs() < 0.01);
        assert!((estimate_cost("gpt-4o", 1_000_000) - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_budget_entry_deserialization() {
        let json = r#"{"model_id":"llama3.2","daily_token_limit":100000}"#;
        let entry: BudgetEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.model_id, "llama3.2");
        assert_eq!(entry.daily_token_limit, 100000);
        assert_eq!(entry.alert_threshold_pct, None);
    }
}
