# Axum Route Handler Example

## Standard Protected Endpoint

```rust
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub permissions: serde_json::Value,
    pub tenant_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// GET /api/iam/roles - List all roles for the tenant
pub async fn list_roles(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<Role>>, (StatusCode, Json<ErrorResponse>)> {
    // 1. Extract tenant (always first step)
    let tenant_id = headers.get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED,
             Json(ErrorResponse { error: "Missing tenant ID".into() }))
        })?;

    // 2. Query with tenant filter (MANDATORY)
    let roles = sqlx::query_as!(Role,
        "SELECT id, name, permissions, tenant_id FROM roles WHERE tenant_id = $1 ORDER BY id",
        tenant_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        // 3. Log internal error, return safe message
        tracing::error!("Failed to fetch roles for tenant {}: {}", tenant_id, e);
        (StatusCode::INTERNAL_SERVER_ERROR,
         Json(ErrorResponse { error: "Failed to fetch roles".into() }))
    })?;

    Ok(Json(roles))
}

/// POST /api/iam/roles - Create a new role
pub async fn create_role(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<Role>), (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = extract_tenant(&headers)?;

    let role = sqlx::query_as!(Role,
        "INSERT INTO roles (name, permissions, tenant_id) VALUES ($1, $2, $3) RETURNING *",
        payload.name, payload.permissions, tenant_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create role: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR,
         Json(ErrorResponse { error: "Failed to create role".into() }))
    })?;

    Ok((StatusCode::CREATED, Json(role)))
}

// Helper to extract tenant — reusable across handlers
fn extract_tenant(headers: &HeaderMap) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    headers.get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED,
             Json(ErrorResponse { error: "Missing tenant ID".into() }))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_roles_returns_seeded_roles() {
        // Arrange
        let pool = setup_test_db().await;
        seed_default_roles(&pool, "test-tenant").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Tenant-Id", "test-tenant".parse().unwrap());

        // Act
        let result = list_roles(State(pool), headers).await;

        // Assert
        assert!(result.is_ok());
        let roles = result.unwrap().0;
        assert!(roles.len() >= 3);
    }

    #[tokio::test]
    async fn test_list_roles_no_tenant_header() {
        let pool = setup_test_db().await;
        let headers = HeaderMap::new(); // empty

        let result = list_roles(State(pool), headers).await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
```
