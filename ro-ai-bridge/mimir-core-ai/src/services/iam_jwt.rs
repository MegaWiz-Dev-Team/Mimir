//! Yggdrasil JWT validation (Sprint 52) — pre-draft for SSO session.
//!
//! Mirrors the pattern shipped in Heimdall gateway 0.6.0
//! (`gateway/src/auth_jwt.rs`). Adapted for Mimir conventions:
//! - `anyhow::Result` instead of a custom error enum
//! - `JWT_AUDIENCE = "mimir"` (S1 vote, captured in memory
//!   `asgard_jwt_auth_pattern`)
//! - Coexists with the existing HS256 flow in [`super::iam::IamService`];
//!   wiring choice (alg-based dispatch vs iss-based dispatch) is left to
//!   the SSO session — see `IAM_JWT_DRAFT.md` for guidance.
//!
//! Claims layout is intentionally identical to Heimdall's so a single
//! `TenantContext` extractor can serve both services unchanged.
//!
//! ## Required deps (not yet in workspace)
//! ```toml
//! # ro-ai-bridge/Cargo.toml [workspace.dependencies]
//! moka = { version = "0.12", features = ["future"] }
//! wiremock = "0.6"  # dev-dep, JWKS mock in tests
//! ```
//! `jsonwebtoken = "10.3"` (with `rust_crypto` feature) is already in the
//! workspace and used by `iam.rs`.
//!
//! Refs: memory/asgard_jwt_auth_pattern.md, Yggdrasil/docs/heimdall-key-gen.md

use anyhow::{anyhow, Result};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use moka::future::Cache;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

/// Cross-service Claims contract (identical to Heimdall's `auth_jwt::Claims`).
///
/// Keep this struct shape in lockstep with Heimdall + future Bifrost/Eir/Hermodr
/// validators. The whole point is that a shared `TenantContext` extractor
/// (via `axum::Extension<Claims>`) Just Works in every service.
#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub exp: usize,
    #[serde(default)]
    pub aud: Option<serde_json::Value>,
    #[serde(default)]
    pub scope: Option<String>,
    /// Zitadel tenant org id — maps directly to Asgard tenant
    /// (e.g. `asgard_medical`, `asgard_insurance`).
    #[serde(default, rename = "urn:zitadel:iam:org:id")]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub roles: Option<serde_json::Value>,
}

impl Claims {
    /// Convenience: pick the "primary" role for the legacy TenantContext shape.
    /// Yggdrasil `roles` claim may be:
    /// - an array of strings: `["admin","editor"]`
    /// - a map keyed by project: `{"<project-id>": {"admin": {...}}}`
    /// - missing
    ///
    /// We prefer the first string we find. Callers needing all roles should
    /// read `claims.roles` directly.
    pub fn primary_role(&self) -> Option<String> {
        let roles = self.roles.as_ref()?;
        if let Some(arr) = roles.as_array() {
            return arr.iter().find_map(|v| v.as_str().map(String::from));
        }
        if let Some(map) = roles.as_object() {
            for v in map.values() {
                if let Some(inner_map) = v.as_object() {
                    if let Some(first_key) = inner_map.keys().next() {
                        return Some(first_key.clone());
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OidcDiscovery {
    jwks_uri: String,
}

pub struct JwtValidator {
    issuer: String,
    audience: String,
    http_client: reqwest::Client,
    jwks_cache: Cache<String, Arc<JwkSet>>,
    discovery_cache: Cache<String, Arc<OidcDiscovery>>,
}

impl JwtValidator {
    /// Build from explicit values. Prefer [`Self::from_env`] in app bootstrap.
    pub fn new(issuer: String, audience: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client builder");
        Self {
            issuer,
            audience,
            http_client,
            jwks_cache: Cache::builder()
                .time_to_live(Duration::from_secs(3600))
                .max_capacity(8)
                .build(),
            discovery_cache: Cache::builder()
                .time_to_live(Duration::from_secs(3600))
                .max_capacity(8)
                .build(),
        }
    }

    /// Returns `Some(validator)` iff both `YGGDRASIL_ISSUER` and
    /// `JWT_AUDIENCE` are set; otherwise JWT mode is off.
    pub fn from_env() -> Option<Self> {
        let issuer = std::env::var("YGGDRASIL_ISSUER").ok().filter(|s| !s.is_empty())?;
        let audience = std::env::var("JWT_AUDIENCE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "mimir".to_string());
        Some(Self::new(issuer, audience))
    }

    /// Validate a bearer token. Returns the parsed claims on success.
    pub async fn validate(&self, token: &str) -> Result<Claims> {
        let header = decode_header(token).map_err(|e| anyhow!("invalid jwt header: {e}"))?;
        // Refuse non-RS256 — this path is for Yggdrasil only.
        // HS256 (legacy IamService::generate_jwt) is handled by a different
        // code path; do NOT silently fall back here.
        if header.alg != Algorithm::RS256 {
            return Err(anyhow!(
                "jwt alg {:?} not handled by Yggdrasil validator",
                header.alg
            ));
        }
        let kid = header.kid.ok_or_else(|| anyhow!("jwt header missing kid"))?;

        let jwks_uri = self.resolve_jwks_uri().await?;
        let jwks = self.fetch_jwks(&jwks_uri).await?;

        let jwk = jwks
            .find(&kid)
            .ok_or_else(|| anyhow!("jwks has no key for kid={kid}"))?;
        let key = DecodingKey::from_jwk(jwk).map_err(|e| anyhow!("jwk → key: {e}"))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        let data = decode::<Claims>(token, &key, &validation)
            .map_err(|e| anyhow!("jwt decode/verify failed: {e}"))?;

        Ok(data.claims)
    }

    /// Quick heuristic for dispatch: caller can peek at the alg without
    /// running full validation. Returns true if the header parses and uses
    /// RS256 — i.e. it *might* be a Yggdrasil token. False for HS256 or
    /// garbage.
    pub fn is_likely_yggdrasil_token(token: &str) -> bool {
        decode_header(token)
            .map(|h| h.alg == Algorithm::RS256)
            .unwrap_or(false)
    }

    async fn resolve_jwks_uri(&self) -> Result<String> {
        if let Some(d) = self.discovery_cache.get(&self.issuer).await {
            return Ok(d.jwks_uri.clone());
        }
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        );
        let discovery: OidcDiscovery = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("oidc discovery http error: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow!("oidc discovery status: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow!("oidc discovery json: {e}"))?;
        let jwks_uri = discovery.jwks_uri.clone();
        self.discovery_cache
            .insert(self.issuer.clone(), Arc::new(discovery))
            .await;
        Ok(jwks_uri)
    }

    async fn fetch_jwks(&self, jwks_uri: &str) -> Result<Arc<JwkSet>> {
        if let Some(jwks) = self.jwks_cache.get(jwks_uri).await {
            return Ok(jwks);
        }
        let jwks: JwkSet = self
            .http_client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| anyhow!("jwks http error: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow!("jwks status: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow!("jwks json: {e}"))?;
        let arc = Arc::new(jwks);
        self.jwks_cache
            .insert(jwks_uri.to_string(), arc.clone())
            .await;
        Ok(arc)
    }

    #[cfg(test)]
    async fn seed_discovery(&self, jwks_uri: String) {
        self.discovery_cache
            .insert(self.issuer.clone(), Arc::new(OidcDiscovery { jwks_uri }))
            .await;
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────
//
// Test RSA keypair is intentionally byte-identical to the one used in
// Heimdall's `gateway/src/auth_jwt.rs`. This means a token signed by the
// test private key validates identically across services — useful when
// the SSO session writes cross-service integration tests later.

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // === Test RSA keypair (TEST ONLY — same as Heimdall test fixture) ===
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
        let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes())
            .expect("test private pem");
        encode(&header, &claims, &key).expect("sign token")
    }

    async fn start_jwks_server() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(jwks_response()))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn validates_well_formed_token() {
        let server = start_jwks_server().await;
        let issuer = server.uri();
        let validator = JwtValidator::new(issuer.clone(), "mimir".into());
        validator
            .seed_discovery(format!("{}/keys", server.uri()))
            .await;

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
        let server = start_jwks_server().await;
        let issuer = server.uri();
        let validator = JwtValidator::new(issuer.clone(), "mimir".into());
        validator
            .seed_discovery(format!("{}/keys", server.uri()))
            .await;

        let token = sign_token(
            json!({
                "sub": "u", "iss": issuer, "aud": "mimir",
                "exp": now() - 3600,
            }),
            TEST_KID,
        );
        assert!(validator.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let server = start_jwks_server().await;
        let issuer = server.uri();
        let validator = JwtValidator::new(issuer.clone(), "mimir".into());
        validator
            .seed_discovery(format!("{}/keys", server.uri()))
            .await;

        let token = sign_token(
            json!({
                "sub": "u", "iss": issuer, "aud": "heimdall",
                "exp": now() + 3600,
            }),
            TEST_KID,
        );
        assert!(validator.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_issuer() {
        let server = start_jwks_server().await;
        let validator =
            JwtValidator::new("https://expected.example.com".into(), "mimir".into());
        validator
            .seed_discovery(format!("{}/keys", server.uri()))
            .await;

        let token = sign_token(
            json!({
                "sub": "u",
                "iss": "https://wrong.example.com",
                "aud": "mimir",
                "exp": now() + 3600,
            }),
            TEST_KID,
        );
        assert!(validator.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn jwks_is_cached_across_calls() {
        let server = start_jwks_server().await;
        let issuer = server.uri();
        let validator = JwtValidator::new(issuer.clone(), "mimir".into());
        validator
            .seed_discovery(format!("{}/keys", server.uri()))
            .await;

        let token = sign_token(
            json!({
                "sub": "u", "iss": issuer, "aud": "mimir",
                "exp": now() + 3600,
            }),
            TEST_KID,
        );
        validator.validate(&token).await.unwrap();
        validator.validate(&token).await.unwrap();
        let received = server.received_requests().await.unwrap();
        let jwks_hits = received.iter().filter(|r| r.url.path() == "/keys").count();
        assert_eq!(jwks_hits, 1, "JWKS endpoint should be hit exactly once (cache)");
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
}
