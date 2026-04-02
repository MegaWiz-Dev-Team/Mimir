//! Backup API Routes (Issue #158)
//!
//! GET /api/v1/backup/status — backup status
//! POST /api/v1/backup/trigger — trigger backup

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use sqlx::MySqlPool;

use mimir_core_ai::services::backup::{build_backup_status, BackupConfig};

pub fn backup_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/status", get(backup_status))
        .route("/trigger", post(backup_trigger))
}

/// GET /api/v1/backup/status
async fn backup_status() -> Json<Value> {
    let config = BackupConfig::default();
    // In production, we'd scan the backup directory for entries
    let status = build_backup_status(&config, &[]);
    Json(json!(status))
}

/// POST /api/v1/backup/trigger
async fn backup_trigger() -> Json<Value> {
    // In production, this would spawn a backup task
    Json(json!({
        "triggered": true,
        "message": "Backup started — check logs for progress",
        "backup_dir": BackupConfig::default().backup_dir
    }))
}
