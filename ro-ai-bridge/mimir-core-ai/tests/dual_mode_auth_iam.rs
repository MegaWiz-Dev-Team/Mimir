//! Integration tests for `services::iam_jwt::JwtValidator` + dual-mode middleware.
//!
//! Builds a minimal axum Router that applies `dual_mode_auth_middleware`
//! to a single test route, then drives requests through it via
//! `tower::ServiceExt::oneshot`. Each test covers one failure or success
//! mode that the middleware is expected to handle for `/api/v1/iam/*`
//! traffic in production.
//!
//! Run with:
//!   cargo test -p mimir-core-ai --test dual_mode_auth_iam

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
    middleware::from_fn,
    routing::get,
    Router,
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use mimir_core_ai::middleware::dual_mode_auth::{dual_mode_auth_middleware, AuthState};
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::iam_jwt::JwtValidator;
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// === Test RSA keypair (same as iam_jwt_validator.rs / Heimdall fixture) ===
const TEST_PRIVATE_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCuNazhOcfCwP6Q\n\
ORwbKB76yrzpMgjs4QO/OAgJoqWwoBedTlJv3aUmTc1qdD3d6/c7uq8jgJkuMq+7\n\
P/JGjDoUYUYBnaewfHwOSh0vOoZnRPHzp5tM524P9Atpqc1rDU++M1DoZd4P2/4r\n\
Vz0j1OEqS/DgqmiCPOYIaeoFMmiFc8YmBwQ3flu025z/4Yk70zB23h6c80Wu/2Zi\n\
lOcxJoZ5bnhaGi+a+dF19aSKeHW24UogY3dLJeRbzCUn/Fmdl2gIJ+8Oo/gvNaCA\n\
tWOgUpdVFIwKmqCdENMI/tPHrLdFc+z1i7S6UFRFK5D77Y8H4O/YPTarXQgiKkPP\n\
gHi23pgVAgMBAAECggEAKOvl1LAQawCPq4wmvIBTqqCl+Hcu3onKqge855qDpjYs\n\
5eAogCuF6DX9aySsBa2wkSC8lC/Yi6APZIJUZFr7J59j5OxGIDBVqbuiGF58hNSO\n\
qyfzVIDGe0vdxG/FF4D0et6uAjEvlPUdwsuGypxuWdOl3PhafUFV3aMXfCoHoFUZ\n\
00Vc/ngeehjMwiBkDb3h4czaWQnwIO86rshoDdfbQpWCMABSyXqLv9pxioR4mOq4\n\
GHjdtRH+Pt6mDVes2HEfo/hv2ZUmlhN/yO7zSmVFWV4Dy4yTpmmXoc7URHayhut3\n\
SMKOWEBNHzfQ4ESHuYRiH6ZiGR8W4w8+UZa+x6iE0QKBgQD2O2BHb7KXurOf4pMc\n\
PsUR1v2DuTMDb8diSm1E7Rt0z6LflCc1vHT4YLDeOvVuKmgWpPodDXUJjgvHCOkF\n\
6u2lAuz/50uEN2ZkWMQ9lVuMXq1VOWlZySKZvsf78Xaccf8aVF8NFYK7OLy6r3YH\n\
rqec0YUJA3A25zLo0iH5ipDdvQKBgQC1Ht9fWfc8R0MBZdMGYAaNHW7f63Q1yzCs\n\
FWBRTUQDrdLNEoCC/VG3pKj30RQ1tqxizDBnBYwD+MxVcOeSdHpzNNrz6MtZtwDv\n\
Gu8XVjYuJdlQQCl7hr3XDcrRXPNyQtCOFG0zJnnhMczQ7RDq6nAITUcLhBGzc/xp\n\
qvpmFt8tOQKBgA1yYqikDfHBTWvu2K/TMbnurruR0ppecVoJzHvWIwi3CiMBmT6T\n\
AyRJS39nYt3YTQTnj40knf6elkARWYBsOvwm88Bp5jLbP6k9O8JNNMmupfKghwNT\n\
O6N/yrYUkrCqfQ74CpTRVulYiN39FQoIXLjwrD44xNkKuToDt71D9vNVAoGBAJm0\n\
jnn7/m3gSAPqptBVM5oULWDID4ILYs3XAjtc5+h7Xlb8aaVAV1YS3fYZMB55XQgn\n\
Irh7I5zHSpkDzPIj+TrF0z6FA/Wp8Zf48oiKeEZnhmmtWcbjzT2xDbrpOAxymUzK\n\
FvX+pBYxThDL7rx9of/ZnP4v4Vm6h64hFIkIxfM5AoGAD5kkcMLv6ofBYNW3LxFL\n\
ulRy+olN705X1sJZdQqDvjr4tT5083aetElhKRjky9FtaO0i00ZqF/AAVG7nwori\n\
8a9cwk4aLt1LduHzbYhhqRn1EHIsEMa81dgXdA34fC29oOBJu/JHhgcB0BIkzzQe\n\
xGYc5U6M/Zw9SyZb+QBKlzo=\n\
-----END PRIVATE KEY-----\n";

const TEST_JWK_N: &str = "rjWs4TnHwsD-kDkcGyge-sq86TII7OEDvzgICaKlsKAXnU5Sb92lJk3NanQ93ev3O7qvI4CZLjKvuz_yRow6FGFGAZ2nsHx8DkodLzqGZ0Tx86ebTOduD_QLaanNaw1PvjNQ6GXeD9v-K1c9I9ThKkvw4KpogjzmCGnqBTJohXPGJgcEN35btNuc_-GJO9Mwdt4enPNFrv9mYpTnMSaGeW54WhovmvnRdfWkinh1tuFKIGN3SyXkW8wlJ_xZnZdoCCfvDqP4LzWggLVjoFKXVRSMCpqgnRDTCP7Tx6y3RXPs9Yu0ulBURSuQ--2PB-Dv2D02q10IIipDz4B4tt6YFQ";
const TEST_JWK_E: &str = "AQAB";
const TEST_KID: &str = "mimir-test-key-1";
const TEST_HS256_SECRET: &str = "test_jwt_secret_for_dual_mode_middleware";

fn now() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
}

/// Build an `AuthState` for the middleware. Tests construct this directly
/// instead of going through `from_env()` so they can inject a specific
/// validator (or omit it for the no-validator path).
fn test_auth_state(jwt_secret: &str, validator: Option<JwtValidator>) -> Arc<AuthState> {
    Arc::new(AuthState::new(jwt_secret.to_string(), validator))
}

/// Handler that just returns the TenantContext that the middleware inserted.
/// 200 with the tenant_id in the body so we can assert on it.
async fn ok_handler(Extension(ctx): Extension<TenantContext>) -> String {
    format!("ok:{}:{}:{}", ctx.user_id, ctx.tenant_id, ctx.role)
}

/// Build the test router. Layer order matters: `from_fn` is the auth
/// middleware (outer), then the route handler (inner). The single
/// `Extension<Arc<AuthState>>` is what the middleware needs.
fn build_app(auth_state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/ok", get(ok_handler))
        .layer(from_fn(dual_mode_auth_middleware))
        .layer(Extension(auth_state))
}

async fn start_oidc_and_jwks_server() -> MockServer {
    let server = MockServer::start().await;
    let issuer = server.uri();
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer.clone(),
            "jwks_uri": format!("{}/keys", issuer),
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "keys": [{
                "kty": "RSA",
                "kid": TEST_KID,
                "use": "sig",
                "alg": "RS256",
                "n": TEST_JWK_N,
                "e": TEST_JWK_E,
            }]
        })))
        .mount(&server)
        .await;
    server
}

fn mint_rs256(claims: serde_json::Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KID.to_string());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    encode(&header, &claims, &key).unwrap()
}

fn mint_hs256(secret: &str, claims: serde_json::Value) -> String {
    let header = Header::new(Algorithm::HS256);
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&header, &claims, &key).unwrap()
}

async fn send_with_header(app: Router, header_value: Option<&str>) -> StatusCode {
    let mut req = Request::builder().uri("/ok");
    if let Some(v) = header_value {
        req = req.header("authorization", v);
    }
    let response = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    response.status()
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn missing_authorization_header_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let status = send_with_header(app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_bearer_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    // Missing the "Bearer " prefix
    let status = send_with_header(app, Some("just-some-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn garbage_token_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let status = send_with_header(app, Some("Bearer not.a.jwt")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hs256_valid_token_returns_200() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let token = mint_hs256(
        TEST_HS256_SECRET,
        json!({
            "iss": "mimir-auth",
            "sub": "user-123",
            "tenant_id": "asgard_medical",
            "role": "admin",
            "exp": now() + 3600,
        }),
    );
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn hs256_wrong_secret_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let token = mint_hs256(
        "completely-different-secret",
        json!({
            "iss": "mimir-auth",
            "sub": "user-123",
            "tenant_id": "asgard_medical",
            "role": "admin",
            "exp": now() + 3600,
        }),
    );
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hs256_wrong_issuer_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let token = mint_hs256(
        TEST_HS256_SECRET,
        json!({
            "iss": "some-other-issuer",
            "sub": "user-123",
            "tenant_id": "asgard_medical",
            "role": "admin",
            "exp": now() + 3600,
        }),
    );
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hs256_expired_returns_401() {
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    let token = mint_hs256(
        TEST_HS256_SECRET,
        json!({
            "iss": "mimir-auth",
            "sub": "user-123",
            "tenant_id": "asgard_medical",
            "role": "admin",
            "exp": now() - 3600,
        }),
    );
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rs256_valid_token_returns_200() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".to_string());
    let app = build_app(test_auth_state(TEST_HS256_SECRET, Some(validator)));
    let token = mint_rs256(json!({
        "sub": "machine-user@yggdrasil",
        "iss": issuer,
        "aud": "mimir",
        "exp": now() + 3600,
        "scope": "search:read",
        "urn:zitadel:iam:org:id": "asgard_insurance",
        "roles": ["admin"],
    }));
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn rs256_no_validator_configured_returns_401() {
    // JwtValidator is None — JWT mode off. RS256 token must be rejected.
    let app = build_app(test_auth_state(TEST_HS256_SECRET, None));
    // Mint a syntactically-valid RS256 token (signature ignored — middleware
    // bails before validation because validator is None).
    let token = mint_rs256(json!({
        "sub": "x",
        "iss": "https://anything",
        "aud": "mimir",
        "exp": now() + 3600,
    }));
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rs256_expired_returns_401() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".to_string());
    let app = build_app(test_auth_state(TEST_HS256_SECRET, Some(validator)));
    let token = mint_rs256(json!({
        "sub": "x",
        "iss": issuer,
        "aud": "mimir",
        "exp": now() - 3600,
    }));
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rs256_wrong_audience_returns_401() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".to_string());
    let app = build_app(test_auth_state(TEST_HS256_SECRET, Some(validator)));
    let token = mint_rs256(json!({
        "sub": "x",
        "iss": issuer,
        "aud": "heimdall",  // wrong target
        "exp": now() + 3600,
    }));
    let status = send_with_header(app, Some(&format!("Bearer {}", token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
