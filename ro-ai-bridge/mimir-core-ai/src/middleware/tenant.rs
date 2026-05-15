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
    config: Option<Extension<Arc<Config>>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Try to read tenant_id from X-Tenant-Id header first
    let header_tenant = req.headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if let Some(tenant_id) = header_tenant {
        req.extensions_mut().insert(TenantContext {
            user_id: tenant_id.clone(),
            tenant_id,
            role: "viewer".to_string(),
        });
        return Ok(next.run(req).await);
    }

    // Fallback: Try to extract from JWT token
    if let Some(Extension(cfg)) = config {
        let auth_header = req.headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .filter(|s| s.starts_with("Bearer "))
            .map(|s| s.to_string());

        if let Some(auth_val) = auth_header {
            let token = &auth_val[7..]; // Remove "Bearer " prefix
            if let Ok(claims) = jsonwebtoken::decode::<TenantClaims>(
                token,
                &jsonwebtoken::DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
                &jsonwebtoken::Validation::default(),
            ) {
                let tenant_id = claims.claims.tenant_id.clone();
                req.extensions_mut().insert(TenantContext {
                    user_id: claims.claims.sub.clone(),
                    tenant_id,
                    role: claims.claims.role.clone(),
                });
                return Ok(next.run(req).await);
            }
        }
    }

    // If both header and JWT are missing, use default
    req.extensions_mut().insert(TenantContext {
        user_id: "unknown".to_string(),
        tenant_id: "default_tenant".to_string(),
        role: "viewer".to_string(),
    });

    Ok(next.run(req).await)
}
