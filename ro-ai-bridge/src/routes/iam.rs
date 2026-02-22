use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Extension, Json, Router,
};
use sqlx::MySqlPool;

use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::models::iam::{CreateUserRequest, UpdateUserPasswordRequest, UpdateUserRoleRequest, UpdateTenantRequest};
use mimir_core_ai::services::iam::IamService;

pub fn iam_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/users", get(get_users).post(create_user))
        .route("/users/{id}/role", patch(update_user_role))
        .route("/users/{id}/password", patch(update_user_password))
        .route("/users/{id}", delete(delete_user))
        .route("/tenants", get(get_tenants))
        .route("/tenants/{id}", patch(update_tenant))
        .route_layer(middleware::from_fn(tenant_auth_middleware))
}

fn check_admin(tenant_ctx: &TenantContext) -> Result<(), StatusCode> {
    if tenant_ctx.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

async fn get_users(
    State(pool): State<MySqlPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.get_users().await {
        Ok(users) => Ok(Json(users)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_tenants(
    State(pool): State<MySqlPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.get_tenants().await {
        Ok(tenants) => Ok(Json(tenants)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn create_user(
    State(pool): State<MySqlPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.create_user(payload).await {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_user_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.update_user_role(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_user_password(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateUserPasswordRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.update_user_password(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    let iam_service = IamService::new(pool);
    match iam_service.delete_user(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn update_tenant(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Json(payload): Json<UpdateTenantRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    check_admin(&tenant_ctx)?;
    
    // An admin should only be able to update their own tenant in a normal setup, 
    // but a superadmin could update any. For now, since Project Mimir is 
    // designed for single organization admin usage right now, we allow updating
    // the specified tenant id if they are an admin.
    
    let iam_service = IamService::new(pool);
    match iam_service.update_tenant(&id, payload).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
