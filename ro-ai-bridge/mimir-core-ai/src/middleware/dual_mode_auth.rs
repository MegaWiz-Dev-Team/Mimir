//! Dual-mode authentication middleware (Sprint 52, Phase 2 Scope A).
//!
//! Replaces the weak `tenant_auth_middleware` for `/api/v1/iam/*` routes only.
//! Other routes (rag_benchmark, icd10, training, monitor) still use the old
//! middleware until they're migrated in a follow-up PR.
//!
//! ## How it works
//!
//! Reads the `Authorization: Bearer <token>` header. Decodes the JWT header
//! to determine `alg` and routes to the matching validator:
//!
//! - **RS256** → [`services::iam_jwt::JwtValidator`] (Yggdrasil RS256 via JWKS)
//! - **HS256** → legacy [`models::iam::TenantClaims`] verifier (internal token
//!   from [`services::iam::IamService::generate_jwt`])
//! - Anything else → 401
//!
//! Both paths build the same [`TenantContext`] and insert it into request
//! extensions, so existing handlers using `Extension<TenantContext>` work
//! unchanged.
//!
//! ## Why dispatch by `alg`, not by `ey`-prefix
//!
//! Heimdall (gateway) uses `ey`-prefix to distinguish JWT vs static API_KEYS.
//! Mimir has no static API_KEYS — both legacy HS256 and Yggdrasil RS256 are
//! JWTs starting with `ey`. The `alg` field in the decoded header is the only
//! reliable signal.
//!
//! ## Failure modes
//!
//! - Missing Authorization header → 401
//! - Malformed bearer (no `Bearer ` prefix) → 401
//! - Garbage token (header decode fails) → 401
//! - RS256 token but no JwtValidator configured → 401 (env not set)
//! - HS256 wrong signature / wrong issuer / expired → 401
//! - RS256 wrong signature / aud / iss / expired → 401
//! - Algorithm other than RS256/HS256 → 401
//!
//! ## Audit + metrics
//!
//! Emits the same shape as Heimdall (Tyr-ingestible):
//! - `tracing::info!(auth_mode = "jwt" | "hs256", sub, tenant, "auth.success")`
//! - `tracing::warn!(auth_mode = "...", error, "auth.failure")`

use crate::config::Config;
use crate::middleware::tenant::{TenantClaims, TenantContext};
use crate::services::iam_jwt::JwtValidator;
use axum::{
    Extension,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use std::sync::Arc;

/// Type alias for the optional JwtValidator extension. Wrapped in `Arc` so
/// the validator (with its JWKS cache) is shared across requests; `Option`
/// so JWT mode can be off when `YGGDRASIL_ISSUER` env is unset (matches the
/// opt-in rollout pattern from Heimdall).
pub type JwtValidatorExt = Extension<Arc<Option<JwtValidator>>>;

/// Drop-in replacement for `tenant_auth_middleware` that actually validates
/// the bearer token instead of trusting the `X-Tenant-Id` header.
///
/// Wiring (in `ro-ai-bridge/src/main.rs` after constructing the router):
/// ```ignore
/// let jwt_validator = JwtValidator::from_env();
/// let validator_ext = Arc::new(jwt_validator);
/// // ...
/// .nest("/api/v1/iam", iam_routes())   // iam_routes uses dual_mode_auth internally
/// .layer(Extension(validator_ext))
/// ```
pub async fn dual_mode_auth_middleware(
    Extension(config): Extension<Arc<Config>>,
    validator: Option<JwtValidatorExt>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            tracing::warn!(auth_mode = "missing", "auth.failure");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Peek at the alg field without verifying the signature.
    let header = match decode_header(token) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(auth_mode = "unknown", error = %e, "auth.failure");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let ctx = match header.alg {
        Algorithm::RS256 => validate_yggdrasil(token, validator).await,
        Algorithm::HS256 => validate_legacy_hs256(token, &config.jwt_secret),
        other => {
            tracing::warn!(
                auth_mode = "rejected",
                alg = ?other,
                "auth.failure: unsupported alg"
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
    }?;

    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

async fn validate_yggdrasil(
    token: &str,
    validator: Option<JwtValidatorExt>,
) -> Result<TenantContext, StatusCode> {
    let validator_ext = validator.ok_or_else(|| {
        tracing::warn!(
            auth_mode = "jwt",
            error = "JwtValidator not configured (YGGDRASIL_ISSUER unset)",
            "auth.failure"
        );
        StatusCode::UNAUTHORIZED
    })?;

    let validator_opt: &Option<JwtValidator> = &validator_ext.0;
    let validator = validator_opt.as_ref().ok_or_else(|| {
        tracing::warn!(
            auth_mode = "jwt",
            error = "JwtValidator None (YGGDRASIL_ISSUER unset)",
            "auth.failure"
        );
        StatusCode::UNAUTHORIZED
    })?;

    match validator.validate(token).await {
        Ok(claims) => {
            // Compute fields first so we can both log and move-into-TenantContext
            // without partial-move conflicts on claims.
            let user_id = claims.sub.clone();
            let tenant_id = claims.tenant_id.clone().unwrap_or_default();
            let role = claims
                .primary_role()
                .unwrap_or_else(|| "viewer".to_string());
            let scope_log = claims.scope.as_deref().unwrap_or("-").to_string();

            tracing::info!(
                auth_mode = "jwt",
                sub = %user_id,
                tenant = %tenant_id,
                scope = %scope_log,
                "auth.success"
            );
            Ok(TenantContext {
                user_id,
                tenant_id,
                role,
            })
        }
        Err(e) => {
            tracing::warn!(auth_mode = "jwt", error = %e, "auth.failure");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

fn validate_legacy_hs256(token: &str, jwt_secret: &str) -> Result<TenantContext, StatusCode> {
    let mut validation = Validation::new(Algorithm::HS256);
    // Legacy `IamService::generate_jwt` sets `iss: "mimir-auth"`.
    validation.set_issuer(&["mimir-auth"]);
    // Legacy claims don't carry `aud`. Skip audience check.
    validation.validate_aud = false;

    match decode::<TenantClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => {
            let claims = data.claims;
            tracing::info!(
                auth_mode = "hs256",
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
        Err(e) => {
            tracing::warn!(auth_mode = "hs256", error = %e, "auth.failure");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
