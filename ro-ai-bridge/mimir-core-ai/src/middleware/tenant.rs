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
    let auth_header = req.headers().get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let token = if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            auth.trim_start_matches("Bearer ")
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        return Err(StatusCode::UNAUTHORIZED);
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
