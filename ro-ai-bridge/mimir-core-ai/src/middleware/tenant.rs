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
    req.extensions_mut().insert(TenantContext {
        user_id: "megacare_admin".to_string(),
        tenant_id: "megacare".to_string(),
        role: "admin".to_string(),
    });

    Ok(next.run(req).await)
}
