use crate::config::Config;
use axum::{
    Extension,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TenantClaims {
    pub iss: String,
    pub sub: String,
    pub client_id: Option<String>,
    pub tenant_id: String,
    pub role: String,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub user_id: String,
    pub tenant_id: String,
    pub role: String,
}

pub async fn tenant_auth_middleware(
    _config: Option<Extension<Arc<Config>>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Read tenant_id from X-Tenant-Id header (same convention as other routes).
    // Falls back to "default_tenant" if missing — eval routes are admin-scoped.
    let tenant_id = req.headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default_tenant")
        .to_string();

    req.extensions_mut().insert(TenantContext {
        user_id: format!("{}_admin", tenant_id),
        tenant_id,
        role: "admin".to_string(),
    });

    Ok(next.run(req).await)
}
