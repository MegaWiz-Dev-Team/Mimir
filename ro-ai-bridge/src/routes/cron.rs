//! Cron & Schedule Routes (Issue #150)
//!
//! - PUT  /sources/:id/schedule — set refresh interval
//! - GET  /sources/:id/schedule — get schedule info
//! - GET  /cron/status          — cron worker health

use axum::{
    Router,
    routing::{get, put},
    extract::{Path, State, Json},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::MySqlPool;
use serde_json::json;
use chrono::Utc;
use mimir_core_ai::models::sources::SetScheduleRequest;
use mimir_core_ai::services::cron::CronState;

/// Build cron/schedule routes.
///
/// Note: `CronState` must be added as an extension to the router.
pub fn cron_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/sources/{id}/schedule", put(set_schedule).get(get_schedule))
}

pub fn cron_status_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/cron/status", get(get_cron_status))
}

/// PUT /sources/:id/schedule — set refresh_interval_hours
async fn set_schedule(
    State(pool): State<MySqlPool>,
    Path(id): Path<i64>,
    Json(req): Json<SetScheduleRequest>,
) -> impl IntoResponse {
    let interval = req.refresh_interval_hours;

    // If interval is 0 or None, disable scheduling
    let (interval_val, next_refresh): (Option<i32>, Option<chrono::DateTime<Utc>>) = 
        match interval {
            Some(h) if h > 0 => {
                let next = Utc::now() + chrono::Duration::hours(h as i64);
                (Some(h), Some(next))
            }
            _ => (None, None),
        };

    match sqlx::query(
        r#"UPDATE data_sources 
           SET refresh_interval_hours = ?, 
               next_refresh_at = ?,
               refresh_status = 'idle'
           WHERE id = ?"#
    )
    .bind(interval_val)
    .bind(next_refresh)
    .bind(id)
    .execute(&pool)
    .await {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({
                "success": true,
                "refresh_interval_hours": interval_val,
                "next_refresh_at": next_refresh
            }))).into_response()
        }
        Ok(_) => {
            (StatusCode::NOT_FOUND, Json(json!({
                "error": "Source not found"
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": format!("Failed to update schedule: {}", e)
            }))).into_response()
        }
    }
}

/// GET /sources/:id/schedule — get schedule info
async fn get_schedule(
    State(pool): State<MySqlPool>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, ScheduleInfo>(
        r#"SELECT id, name, refresh_interval_hours, last_refreshed_at, 
                  next_refresh_at, refresh_status
           FROM data_sources WHERE id = ?"#
    )
    .bind(id)
    .fetch_optional(&pool)
    .await {
        Ok(Some(info)) => (StatusCode::OK, Json(json!({
            "id": info.id,
            "name": info.name,
            "refresh_interval_hours": info.refresh_interval_hours,
            "last_refreshed_at": info.last_refreshed_at,
            "next_refresh_at": info.next_refresh_at,
            "refresh_status": info.refresh_status,
            "enabled": info.refresh_interval_hours.is_some()
        }))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /cron/status — cron worker health
async fn get_cron_status(
    cron_state: Option<axum::Extension<CronState>>,
) -> impl IntoResponse {
    match cron_state {
        Some(state) => {
            let last_tick = state.last_tick_at.lock().await;
            let active = state.active_jobs.lock().await;
            let total = state.total_runs.lock().await;
            
            (StatusCode::OK, Json(json!({
                "status": "running",
                "last_tick_at": *last_tick,
                "active_jobs": *active,
                "total_runs": *total
            }))).into_response()
        }
        None => {
            (StatusCode::OK, Json(json!({
                "status": "not_started",
                "last_tick_at": null,
                "active_jobs": 0,
                "total_runs": 0
            }))).into_response()
        }
    }
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
struct ScheduleInfo {
    id: i64,
    name: String,
    refresh_interval_hours: Option<i32>,
    last_refreshed_at: Option<chrono::DateTime<chrono::Utc>>,
    next_refresh_at: Option<chrono::DateTime<chrono::Utc>>,
    refresh_status: Option<String>,
}
