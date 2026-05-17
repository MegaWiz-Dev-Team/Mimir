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
//! ## Why a dedicated `AuthState`, not `Extension<Arc<Config>>`
//!
//! There are TWO `Config` structs in the workspace — `mimir_core_ai::config::Config`
//! (this crate, smaller) and `ro_ai_bridge::config::Config` (binary crate, bigger,
//! adds LLM provider config etc.). The `Extension` extractor matches by exact
//! `TypeId`, so a middleware in `mimir-core-ai` cannot pick up the binary
//! crate's Config Extension that `main.rs` attaches. The legacy
//! `tenant_auth_middleware` papered over this by declaring `Option<Extension<...>>`
//! and ignoring the value. This middleware actually needs `jwt_secret`, so we
//! define a focused `AuthState` dep instead — `main.rs` constructs it from
//! whichever Config it has and attaches as an Extension.
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

/// Focused dependency set for `dual_mode_auth_middleware`.
///
/// Constructed at app bootstrap and attached as an Extension before the
/// middleware layer. Avoids the cross-crate Config type-mismatch trap (see
/// the module-level "Why a dedicated AuthState" note).
///
/// - `legacy_jwt_secret` — shared secret used to validate HS256 tokens issued
///   by `IamService::generate_jwt` (username/password and OIDC code-exchange
///   login flows). Read from `JWT_SECRET` env at startup, owned by Config.
/// - `jwt_validator` — `Some(_)` iff `YGGDRASIL_ISSUER` (and optionally
///   `JWT_AUDIENCE`) env vars are set. When `None`, RS256 tokens get 401 and
///   only legacy HS256 path is active. Opt-in pattern matches Heimdall 0.6.0.
pub struct AuthState {
    pub legacy_jwt_secret: String,
    pub jwt_validator: Option<JwtValidator>,
}

impl AuthState {
    pub fn new(legacy_jwt_secret: String, jwt_validator: Option<JwtValidator>) -> Self {
        Self {
            legacy_jwt_secret,
            jwt_validator,
        }
    }

    /// Convenience for bootstrap: caller passes the HS256 secret, we read the
    /// optional Yggdrasil config from env. Same opt-in rollout as Heimdall.
    pub fn from_env(legacy_jwt_secret: String) -> Self {
        Self {
            legacy_jwt_secret,
            jwt_validator: JwtValidator::from_env(),
        }
    }

    /// True iff Yggdrasil RS256 mode is currently enabled. Useful for the
    /// startup log line in `main.rs` so operators can confirm env wiring.
    pub fn jwt_enabled(&self) -> bool {
        self.jwt_validator.is_some()
    }
}

/// Drop-in replacement for `tenant_auth_middleware` that actually validates
/// the bearer token instead of trusting the `X-Tenant-Id` header.
///
/// Wiring (in `ro-ai-bridge/src/main.rs` after constructing the router):
/// ```ignore
/// let auth_state = Arc::new(AuthState::from_env(config.jwt_secret.clone()));
/// info!(jwt_enabled = auth_state.jwt_enabled(), "auth bootstrap");
/// // ...
/// .nest("/api/v1/iam", iam_routes())   // routes call dual_mode_auth_middleware via route_layer
/// .layer(Extension(auth_state))
/// ```
pub async fn dual_mode_auth_middleware(
    Extension(auth_state): Extension<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    // Parse the Authorization header. Per RFC 6750/7235 the auth scheme name
    // is CASE-INSENSITIVE ("Bearer" / "bearer" / "BEARER" all valid). We also
    // trim whitespace around the token so "Bearer   abc.def.ghi" works.
    let token = match auth_header.and_then(parse_bearer_token) {
        Some(t) if !t.is_empty() => t,
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
        Algorithm::RS256 => validate_yggdrasil(token, auth_state.jwt_validator.as_ref()).await,
        Algorithm::HS256 => validate_legacy_hs256(token, &auth_state.legacy_jwt_secret),
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

/// Parse `Authorization: <scheme> <token>` per RFC 6750/7235.
///
/// Returns `Some(token)` iff the scheme is "Bearer" (case-insensitive) and
/// at least one space follows it. Returns `None` for any other shape so the
/// caller emits a single 401 path. Token is trimmed of leading/trailing
/// whitespace.
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
) -> Result<TenantContext, StatusCode> {
    let validator = validator.ok_or_else(|| {
        tracing::warn!(
            auth_mode = "jwt",
            error = "JwtValidator not configured (YGGDRASIL_ISSUER unset)",
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
