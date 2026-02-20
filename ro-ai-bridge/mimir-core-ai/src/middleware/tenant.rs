use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
    http::{StatusCode, header},
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

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
    pub tenant_id: String,
    pub role: String,
}

pub async fn tenant_auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Try to get tenant_id from X-Tenant-Id header first (for development/legacy bypassing full JWT)
    let mut tenant_from_header = None;
    if let Some(tenant_header) = req.headers().get("X-Tenant-Id") {
        if let Ok(tenant_str) = tenant_header.to_str() {
            tenant_from_header = Some(tenant_str.to_string());
        }
    }

    if let Some(tenant_id) = tenant_from_header {
        req.extensions_mut().insert(TenantContext { 
            tenant_id,
            role: "admin".to_string(), // Trust header value role as admin for legacy
        });
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers().get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let token = if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            auth.trim_start_matches("Bearer ")
        } else {
            req.extensions_mut().insert(TenantContext {
                tenant_id: "default_tenant".to_string(),
                role: "admin".to_string(), // Default fallback is admin for legacy endpoints
            });
            return Ok(next.run(req).await);
        }
    } else {
        req.extensions_mut().insert(TenantContext {
            tenant_id: "default_tenant".to_string(),
            role: "admin".to_string(),
        });
        return Ok(next.run(req).await);
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev_secret_key".to_string());
    
    // Decode and validate token
    let token_data = match decode::<TenantClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(c) => c,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    req.extensions_mut().insert(TenantContext {
        tenant_id: token_data.claims.tenant_id,
        role: token_data.claims.role,
    });

    Ok(next.run(req).await)
}
