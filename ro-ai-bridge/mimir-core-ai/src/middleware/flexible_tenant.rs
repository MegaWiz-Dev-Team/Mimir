//! Flexible tenant resolution for read-only / public-facing endpoints.
//!
//! Sits between the legacy [`tenant_auth_middleware`] (trusts `X-Tenant-Id`
//! blindly, no auth) and [`dual_mode_auth_middleware`] (requires a valid JWT,
//! 401 otherwise). Resolves tenant in this order:
//!
//! 1. **JWT** — `Authorization: Bearer <token>`. Dispatches RS256 → Yggdrasil
//!    and HS256 → legacy verifier, same as `dual_mode_auth_middleware`.
//!    Failure to validate **falls through to step 2**, with a WARN log — a
//!    malformed JWT does not auto-401 because callers can also fall back to
//!    the header path (matches Bifrost's flexible auth).
//! 2. **X-Tenant-Id header** — fallback. Emits a WARN log per request so
//!    operators can identify header-only callers and gate them off in prod.
//! 3. Neither → **401 Unauthorized**.
//!
//! When both JWT and header are present, JWT wins; the header is silently
//! ignored to prevent tenant spoofing.
//!
//! Used by `/api/v1/agents` list + detail (Iris and future external read
//! clients). NOT used by `/api/v1/iam/*` — those keep strict JWT.

use crate::middleware::dual_mode_auth::AuthState;
use crate::middleware::tenant::{TenantClaims, TenantContext};
use crate::services::iam_jwt::JwtValidator;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
    Extension,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use std::sync::Arc;

pub async fn flexible_tenant_middleware(
    Extension(auth_state): Extension<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. JWT path
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer_token)
        .filter(|t| !t.is_empty());

    if let Some(token) = bearer {
        match decode_header(token) {
            Ok(header) => match header.alg {
                Algorithm::RS256 => {
                    match validate_yggdrasil(token, auth_state.jwt_validator.as_ref()).await {
                        Ok(ctx) => {
                            req.extensions_mut().insert(ctx);
                            return Ok(next.run(req).await);
                        }
                        Err(e) => {
                            tracing::warn!(
                                auth_mode = "jwt_rs256",
                                error = %e,
                                "auth.failure: JWT present but invalid; falling through to header path"
                            );
                        }
                    }
                }
                Algorithm::HS256 => {
                    match validate_legacy_hs256(token, &auth_state.legacy_jwt_secret) {
                        Ok(ctx) => {
                            req.extensions_mut().insert(ctx);
                            return Ok(next.run(req).await);
                        }
                        Err(e) => {
                            tracing::warn!(
                                auth_mode = "jwt_hs256",
                                error = %e,
                                "auth.failure: JWT present but invalid; falling through to header path"
                            );
                        }
                    }
                }
                other => {
                    tracing::warn!(
                        auth_mode = "jwt_rejected",
                        alg = ?other,
                        "auth.failure: unsupported alg; falling through to header path"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    auth_mode = "jwt_unknown",
                    error = %e,
                    "auth.failure: bearer header undecodable; falling through to header path"
                );
            }
        }
    }

    // 2. Header fallback — only allowed when JWT was not presented OR JWT
    //    validation failed. Either way emit a WARN so operators can see
    //    header-only traffic and consider gating it off in production.
    let header_tenant: Option<String> = req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if let Some(tenant_id) = header_tenant {
        tracing::warn!(
            auth_mode = "header_fallback",
            tenant_id = %tenant_id,
            "auth.fallback: using X-Tenant-Id header (no valid JWT)"
        );
        req.extensions_mut().insert(TenantContext {
            user_id: format!("{}_header", tenant_id),
            tenant_id,
            role: "viewer".to_string(),
        });
        return Ok(next.run(req).await);
    }

    // 3. Neither path → 401
    tracing::warn!(auth_mode = "missing", "auth.failure: no JWT, no X-Tenant-Id");
    Err(StatusCode::UNAUTHORIZED)
}

fn parse_bearer_token(header: &str) -> Option<&str> {
    let (scheme, rest) = header.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("Bearer") {
        Some(rest.trim())
    } else {
        None
    }
}

async fn validate_yggdrasil(
    token: &str,
    validator: Option<&JwtValidator>,
) -> Result<TenantContext, String> {
    let validator = validator.ok_or_else(|| {
        "JwtValidator not configured (YGGDRASIL_ISSUER unset)".to_string()
    })?;
    let claims = validator.validate(token).await.map_err(|e| e.to_string())?;
    let user_id = claims.sub.clone();
    let tenant_id = claims.tenant_id.clone().unwrap_or_default();
    let role = claims.primary_role().unwrap_or_else(|| "viewer".to_string());
    if tenant_id.is_empty() {
        return Err("JWT valid but missing tenant claim".to_string());
    }
    tracing::info!(
        auth_mode = "jwt_rs256",
        sub = %user_id,
        tenant = %tenant_id,
        "auth.success"
    );
    Ok(TenantContext {
        user_id,
        tenant_id,
        role,
    })
}

fn validate_legacy_hs256(token: &str, jwt_secret: &str) -> Result<TenantContext, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["mimir-auth"]);
    validation.validate_aud = false;

    let data = decode::<TenantClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| e.to_string())?;
    let claims = data.claims;
    tracing::info!(
        auth_mode = "jwt_hs256",
        sub = %claims.sub,
        tenant = %claims.tenant_id,
        "auth.success"
    );
    Ok(TenantContext {
        user_id: claims.sub,
        tenant_id: claims.tenant_id,
        role: claims.role,
    })
}
