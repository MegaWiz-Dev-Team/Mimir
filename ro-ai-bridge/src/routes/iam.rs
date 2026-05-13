use crate::config::Config;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch},
    Extension, Json, Router,
};
use sqlx::MySqlPool;
use std::sync::Arc;

use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::models::iam::{
    CreateRoleRequest, CreateTenantRequest, CreateUserRequest, UpdateRoleRequest,
    UpdateTenantConfigRequest, UpdateTenantRequest, UpdateUserPasswordRequest,
    UpdateUserRoleRequest,
};
use mimir_core_ai::services::domain;
use mimir_core_ai::services::iam::IamService;

pub fn iam_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/users", get(get_users).post(create_user))
        .route("/users/{id}/role", patch(update_user_role))
        .route("/users/{id}/password", patch(update_user_password))
        .route("/users/{id}", delete(delete_user))
        .route("/tenants", get(get_tenants).post(create_tenant))
        .route("/tenants/{id}", patch(update_tenant).delete(delete_tenant))
        .route(
            "/tenants/{id}/config",
            get(get_tenant_config).patch(update_tenant_config),
        )
        .route("/tenants/{id}/features", get(get_tenant_features))
        .route("/my-tenants", get(get_my_tenants))
        // Custom Roles — Issue #191
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/{id}", patch(update_role).delete(delete_role))
        .route_layer(middleware::from_fn(tenant_auth_middleware))
}

/// GET /api/v1/iam/tenants/{id}/features
/// Returns all feature flags with enabled/disabled status for the tenant's domain.
async fn get_tenant_features(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    let tenant_domain = iam_service
        .get_tenant_domain(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let features = domain::get_all_features(&tenant_domain);
    let feature_map: serde_json::Value = features
        .into_iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::Bool(v)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into();

    Ok(Json(serde_json::json!({
        "tenant_id": id,
        "domain": tenant_domain,
        "features": feature_map
    })))
}

fn check_admin(tenant_ctx: &TenantContext) -> Result<(), StatusCode> {
    if tenant_ctx.role.to_lowercase() != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

async fn get_users(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.get_users().await {
        Ok(users) => Ok(Json(users)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_tenants(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.get_tenants().await {
        Ok(tenants) => Ok(Json(tenants)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /api/v1/iam/my-tenants
/// Returns only tenants assigned to the current user (from JWT sub claim).
/// No admin check needed — every user can see their own tenants.
async fn get_my_tenants(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.get_my_tenants(&tenant_ctx.user_id).await {
        Ok(tenants) => Ok(Json(tenants)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn create_user(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.create_user(payload).await {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_user_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.update_user_role(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_user_password(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateUserPasswordRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.update_user_password(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.delete_user(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_tenant(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateTenantRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.update_tenant(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn create_tenant(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.create_tenant(payload).await {
        Ok(tenant) => Ok((StatusCode::CREATED, Json(tenant))),
        Err(e) => {
            tracing::error!("Failed to create tenant: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn delete_tenant(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.delete_tenant(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_tenant_config(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.get_tenant_config(&id).await {
        Ok(config) => Ok(Json(config)),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_tenant_config(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateTenantConfigRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;

    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.update_tenant_config(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ─── Custom Roles Handlers — Issue #191 ──────────────────────────────────────

/// GET /api/v1/iam/roles — list all roles for the current tenant
async fn list_roles(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.list_roles(&tenant_ctx.tenant_id).await {
        Ok(roles) => Ok(Json(roles)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// POST /api/v1/iam/roles — create a custom role (admin only)
async fn create_role(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service
        .create_role(&tenant_ctx.tenant_id, payload)
        .await
    {
        Ok(role) => Ok((StatusCode::CREATED, Json(role))),
        Err(e) => {
            if e.to_string().contains("already exists") {
                Err(StatusCode::CONFLICT)
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// PATCH /api/v1/iam/roles/:id — update a custom role's permissions (admin only)
async fn update_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.update_role(&id, payload).await {
        Ok(role) => Ok(Json(role)),
        Err(e) => {
            if e.to_string().contains("built-in") {
                Err(StatusCode::FORBIDDEN)
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// DELETE /api/v1/iam/roles/:id — delete a custom role (admin only)
async fn delete_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(config): Extension<Arc<Config>>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service.delete_role(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("built-in") {
                Err(StatusCode::FORBIDDEN)
            } else if msg.contains("assigned to") {
                Err(StatusCode::BAD_REQUEST)
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
