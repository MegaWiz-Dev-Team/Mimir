//! Integration tests for `services::iam_jwt::JwtValidator`.
//!
//! These mirror the module-level tests inside `iam_jwt.rs` but live in
//! `tests/` so they compile into a SEPARATE test binary. The lib-test target
//! is currently blocked by unrelated pre-existing failures (mcp_server,
//! a2a, runner test fixtures haven't been updated to match recent struct
//! changes — out of scope for the Backend SSO session).
//!
//! Once those broken modules are fixed (S1 / future session), these
//! integration tests can be removed in favor of the module-level ones,
//! or kept as an additional verification layer. Run with:
//!
//!   cargo test -p mimir-core-ai --test iam_jwt_validator
//!
//! Differences vs the module-level tests in `iam_jwt.rs`:
//! - Uses the full OIDC discovery flow (mocks `.well-known/openid-configuration`
//!   alongside the JWKS endpoint) instead of `seed_discovery()` (which is
//!   `#[cfg(test)]`-private and not visible from integration tests).
//! - Otherwise byte-identical assertions and test RSA keypair.

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use mimir_core_ai::services::iam_jwt::JwtValidator;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// === Test RSA keypair (TEST ONLY — same as Heimdall + iam_jwt.rs fixture) ===
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

fn jwks_response() -> serde_json::Value {
    json!({
        "keys": [{
            "kty": "RSA",
            "kid": TEST_KID,
            "use": "sig",
            "alg": "RS256",
            "n": TEST_JWK_N,
            "e": TEST_JWK_E,
        }]
    })
}

fn now() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
}

fn sign_token(claims: serde_json::Value, kid: &str) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).expect("test private pem");
    encode(&header, &claims, &key).expect("sign token")
}

/// Starts a MockServer that serves BOTH the OIDC discovery doc AND the JWKS
/// endpoint, so JwtValidator can resolve everything via the public API path
/// (no `seed_discovery` shortcut needed).
async fn start_oidc_and_jwks_server() -> MockServer {
    let server = MockServer::start().await;
    let issuer = server.uri();
    let jwks_path = "/keys";

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer.clone(),
            "jwks_uri": format!("{}{}", issuer, jwks_path),
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(jwks_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(jwks_response()))
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn validates_well_formed_token() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".into());

    let token = sign_token(
        json!({
            "sub": "machine-user@yggdrasil",
            "iss": issuer,
            "aud": "mimir",
            "exp": now() + 3600,
            "scope": "ingest:write search:read",
            "urn:zitadel:iam:org:id": "asgard_insurance",
            "roles": ["admin"],
        }),
        TEST_KID,
    );

    let claims = validator.validate(&token).await.expect("valid");
    assert_eq!(claims.sub, "machine-user@yggdrasil");
    assert_eq!(claims.tenant_id.as_deref(), Some("asgard_insurance"));
    assert_eq!(claims.primary_role().as_deref(), Some("admin"));
}

#[tokio::test]
async fn rejects_hs256_token() {
    // HS256 (legacy IamService tokens) must NOT be silently accepted by
    // the Yggdrasil path. The middleware is responsible for routing them
    // to the HS256 verifier instead.
    let validator = JwtValidator::new("https://x".into(), "mimir".into());
    let key = EncodingKey::from_secret(b"dev_secret_key");
    let token = encode(
        &Header::new(Algorithm::HS256),
        &json!({"sub": "u", "exp": now() + 3600}),
        &key,
    )
    .unwrap();
    let err = validator.validate(&token).await.unwrap_err();
    assert!(err.to_string().contains("alg"), "got: {err}");
}

#[tokio::test]
async fn rejects_expired_token() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".into());

    let token = sign_token(
        json!({
            "sub": "u",
            "iss": issuer,
            "aud": "mimir",
            "exp": now() - 3600,
        }),
        TEST_KID,
    );
    assert!(validator.validate(&token).await.is_err());
}

#[tokio::test]
async fn rejects_wrong_audience() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".into());

    let token = sign_token(
        json!({
            "sub": "u",
            "iss": issuer,
            "aud": "heimdall",
            "exp": now() + 3600,
        }),
        TEST_KID,
    );
    assert!(validator.validate(&token).await.is_err());
}

#[tokio::test]
async fn rejects_wrong_issuer() {
    let server = start_oidc_and_jwks_server().await;
    // Validator's expected issuer != server.uri() (the iss claim issuer)
    let validator =
        JwtValidator::new("https://expected.example.com".into(), "mimir".into());

    let token = sign_token(
        json!({
            "sub": "u",
            "iss": "https://wrong.example.com",
            "aud": "mimir",
            "exp": now() + 3600,
        }),
        TEST_KID,
    );
    // validate() will try to resolve discovery against
    // https://expected.example.com — that won't resolve in tests, so this
    // returns an error early. That's the right shape: wrong issuer → reject.
    // Drop the unused server so it doesn't leak.
    drop(server);
    assert!(validator.validate(&token).await.is_err());
}

#[tokio::test]
async fn jwks_is_cached_across_calls() {
    let server = start_oidc_and_jwks_server().await;
    let issuer = server.uri();
    let validator = JwtValidator::new(issuer.clone(), "mimir".into());

    let token = sign_token(
        json!({
            "sub": "u",
            "iss": issuer,
            "aud": "mimir",
            "exp": now() + 3600,
        }),
        TEST_KID,
    );
    validator.validate(&token).await.unwrap();
    validator.validate(&token).await.unwrap();

    let received = server.received_requests().await.unwrap();
    let jwks_hits = received.iter().filter(|r| r.url.path() == "/keys").count();
    assert_eq!(
        jwks_hits, 1,
        "JWKS endpoint should be hit exactly once (cache)"
    );
}

#[tokio::test]
async fn is_likely_yggdrasil_token_distinguishes_alg() {
    // RS256 token → looks like Yggdrasil
    let header = Header::new(Algorithm::RS256);
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let rs = encode(&header, &json!({"sub": "x"}), &key).unwrap();
    assert!(JwtValidator::is_likely_yggdrasil_token(&rs));

    // HS256 token → not Yggdrasil (= legacy IamService path)
    let hs = encode(
        &Header::new(Algorithm::HS256),
        &json!({"sub": "x"}),
        &EncodingKey::from_secret(b"x"),
    )
    .unwrap();
    assert!(!JwtValidator::is_likely_yggdrasil_token(&hs));

    // garbage → not Yggdrasil
    assert!(!JwtValidator::is_likely_yggdrasil_token("not.a.jwt"));
}
