use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use mimir_core_ai::services::db::DbPool;

// ── Models ────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub service_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub id: String,
    pub name: String,
    pub service_type: Option<String>,
    pub description: Option<String>,
}

// ── Helpers ───────────────────────────────────────────

/// Extract tenant_id from the X-Tenant-Id request header.
pub fn extract_tenant_id<'a>(headers: &'a HeaderMap) -> &'a str {
    headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or("default_tenant")
}

/// Middleware layer: reject requests missing X-Tenant-Id header.
/// Returns 401 Unauthorized with a JSON error body.
pub async fn require_tenant_id(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let tenant_header = headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty());

    match tenant_header {
        Some(_) => Ok(next.run(request).await),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Missing required X-Tenant-Id header",
                "code": "TENANT_ID_REQUIRED",
            })),
        )),
    }
}

async fn ensure_tenant_columns(pool: &DbPool) {
    let _ = sqlx::query("ALTER TABLE tenants ADD COLUMN service_type VARCHAR(64)")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE tenants ADD COLUMN description TEXT")
        .execute(pool).await;
    // Fix domain column width (was VARCHAR(20), too short for '{id}.asgard.local')
    let _ = sqlx::query("ALTER TABLE tenants MODIFY COLUMN domain VARCHAR(255) NOT NULL DEFAULT ''")
        .execute(pool).await;
}

// ── Routes ────────────────────────────────────────────

pub fn tenant_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_tenants).post(create_tenant))
        .route("/{id}", get(get_tenant).delete(delete_tenant))
}

// ── Handlers ──────────────────────────────────────────

async fn create_tenant(
    State(pool): State<DbPool>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    ensure_tenant_columns(&pool).await;

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM tenants WHERE id = ?")
            .bind(&req.id)
            .fetch_optional(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, Json(json!({"error": format!("Tenant '{}' already exists", req.id)}))));
    }

    let domain = format!("{}.asgard.local", &req.id);
    sqlx::query("INSERT INTO tenants (id, name, domain, service_type, description) VALUES (?, ?, ?, ?, ?)")
        .bind(&req.id).bind(&req.name).bind(&domain).bind(&req.service_type).bind(&req.description)
        .execute(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok((StatusCode::CREATED, Json(json!({"id": req.id, "name": req.name, "domain": domain, "service_type": req.service_type, "description": req.description}))))
}

async fn list_tenants(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    ensure_tenant_columns(&pool).await;

    let rows: Vec<Tenant> = sqlx::query_as(
        "SELECT id, name, domain, service_type, description FROM tenants ORDER BY created_at",
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(rows.into_iter().map(|t| json!({"id": t.id, "name": t.name, "domain": t.domain, "service_type": t.service_type, "description": t.description})).collect()))
}

async fn get_tenant(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_tenant_columns(&pool).await;

    let tenant: Tenant = sqlx::query_as(
        "SELECT id, name, domain, service_type, description FROM tenants WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": format!("Tenant '{}' not found", id)}))))?;

    Ok(Json(json!({"id": tenant.id, "name": tenant.name, "domain": tenant.domain, "service_type": tenant.service_type, "description": tenant.description})))
}

async fn delete_tenant(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = sqlx::query("DELETE FROM tenants WHERE id = ?")
        .bind(&id).execute(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": format!("Tenant '{}' not found", id)}))));
    }

    Ok(Json(json!({"deleted": id})))
}
