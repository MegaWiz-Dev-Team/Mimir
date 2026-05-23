//! Integration tests for `/api/v1/agents` list + detail.
//!
//! Tests Mimir's public-read endpoints (the ones extended with JWT,
//! rate-limit, audit, and rag_params whitelist). The admin endpoints
//! (POST/PUT/DELETE/publish/chat/conversations/generate/route) are out of
//! scope — they kept the legacy X-Tenant-Id-only contract.
//!
//! Each test creates its own unique tenant_id + tenant row, seeds agents,
//! then cleans up. Tests are safe to run in parallel.
//!
//! Default DB: `mysql://root:root@127.0.0.1:3306/mimir_test` (override via
//! `DATABASE_URL`).
//!
//! Skip locally with `--skip integration` if the DB isn't reachable.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Extension, Router,
};
use http_body_util::BodyExt;
use mimir_core_ai::middleware::dual_mode_auth::AuthState;
use ro_ai_bridge::routes::agents::agents_routes;
use serde_json::{json, Value};
use sqlx::MySqlPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

// ───────────────────────── Test harness ──────────────────────────────────

fn db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3306/mimir_test".to_string())
}

async fn pool() -> MySqlPool {
    MySqlPool::connect(&db_url())
        .await
        .expect("connect to test DB — set DATABASE_URL if not localhost:3306")
}

struct TestTenant {
    pool: MySqlPool,
    tenant_id: String,
}

impl TestTenant {
    async fn new(pool: MySqlPool) -> Self {
        let tenant_id = format!("mimir_int_{}", Uuid::new_v4().simple());
        sqlx::query("INSERT INTO tenants (id, name, domain) VALUES (?, ?, '')")
            .bind(&tenant_id)
            .bind(&tenant_id)
            .execute(&pool)
            .await
            .expect("seed tenant");
        Self { pool, tenant_id }
    }

    async fn seed_agent(&self, params: SeedAgent<'_>) -> i64 {
        let res = sqlx::query(
            "INSERT INTO agent_configs ( \
                tenant_id, name, display_name, description, system_prompt, \
                model_id, provider, temperature, max_tokens, top_k, \
                use_rag, use_knowledge_graph, use_pageindex, \
                tools, mcp_servers, personality_traits, greeting, avatar_url, \
                rag_params, template_id, api_key, is_published \
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&self.tenant_id)
        .bind(params.name)
        .bind(params.display_name)
        .bind(params.description)
        .bind(params.system_prompt)
        .bind(params.model_id)
        .bind(params.provider)
        .bind(params.temperature)
        .bind(params.max_tokens)
        .bind(params.top_k)
        .bind(params.use_rag as i8)
        .bind(params.use_knowledge_graph as i8)
        .bind(params.use_pageindex as i8)
        .bind(params.tools.map(|v| v.to_string()))
        .bind(params.mcp_servers.map(|v| v.to_string()))
        .bind(params.personality_traits.map(|v| v.to_string()))
        .bind(params.greeting)
        .bind(params.avatar_url)
        .bind(params.rag_params.map(|v| v.to_string()))
        .bind(params.template_id)
        .bind(params.api_key)
        .bind(params.is_published as i8)
        .execute(&self.pool)
        .await
        .expect("seed agent");
        res.last_insert_id() as i64
    }

    /// Build the agents sub-router mounted at `/api/v1/agents`, with an
    /// AuthState that has `jwt_validator = None` so the header-fallback
    /// path is exercised (this matches how a dev box runs without
    /// `YGGDRASIL_ISSUER` set).
    fn router(&self) -> Router {
        let auth = Arc::new(AuthState::new("test_secret_unused".to_string(), None));
        Router::new()
            .nest("/api/v1/agents", agents_routes())
            .with_state(self.pool.clone())
            .layer(Extension(auth))
    }

    async fn cleanup(&self) {
        let _ = sqlx::query("DELETE FROM agent_configs WHERE tenant_id = ?")
            .bind(&self.tenant_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM tenants WHERE id = ?")
            .bind(&self.tenant_id)
            .execute(&self.pool)
            .await;
    }
}

#[derive(Default)]
struct SeedAgent<'a> {
    name: &'a str,
    display_name: Option<&'a str>,
    description: Option<&'a str>,
    system_prompt: &'a str,
    model_id: &'a str,
    provider: &'a str,
    temperature: Option<f64>,
    max_tokens: Option<i32>,
    top_k: Option<i32>,
    use_rag: bool,
    use_knowledge_graph: bool,
    use_pageindex: bool,
    tools: Option<Value>,
    mcp_servers: Option<Value>,
    personality_traits: Option<Value>,
    greeting: Option<&'a str>,
    avatar_url: Option<&'a str>,
    rag_params: Option<Value>,
    template_id: Option<&'a str>,
    api_key: Option<&'a str>,
    is_published: bool,
}

impl<'a> SeedAgent<'a> {
    fn vanilla(name: &'a str) -> Self {
        Self {
            name,
            display_name: Some("Test Agent"),
            description: Some("A test agent"),
            system_prompt: "You are a test agent.",
            model_id: "gemma-4-26b",
            provider: "mlx",
            temperature: Some(0.7),
            max_tokens: Some(2048),
            top_k: Some(5),
            use_rag: true,
            is_published: true,
            ..Default::default()
        }
    }
}

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = router.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            json!({ "_raw": String::from_utf8_lossy(&bytes).to_string() })
        })
    };
    (status, body)
}

fn rand_u8() -> u8 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    ((nanos.wrapping_mul(2654435761)) >> 24) as u8
}

fn get(uri: &str, tenant: &str) -> Request<Body> {
    let fake_ip = format!("10.0.{}.{}", rand_u8(), rand_u8());
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("X-Tenant-Id", tenant)
        .header("X-Forwarded-For", fake_ip)
        .body(Body::empty())
        .unwrap()
}

fn get_from_ip(uri: &str, tenant: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("X-Tenant-Id", tenant)
        .header("X-Forwarded-For", ip)
        .body(Body::empty())
        .unwrap()
}

// ───────────────────────── Tests ─────────────────────────────────────────

#[tokio::test]
async fn t1_list_shape_has_capabilities_and_legacy_fields() {
    let t = TestTenant::new(pool().await).await;
    t.seed_agent(SeedAgent {
        tools: Some(json!(["vector_search", "ocr_extract"])),
        ..SeedAgent::vanilla("vanilla-1")
    })
    .await;

    let (status, body) = send(&t.router(), get("/api/v1/agents", &t.tenant_id)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let agent = &body["agents"][0];
    assert_eq!(agent["name"], "vanilla-1");
    assert_eq!(agent["model_id"], "gemma-4-26b");
    assert_eq!(agent["is_published"], true);
    let caps = &agent["capabilities"];
    assert_eq!(caps["model_id"], "gemma-4-26b");
    assert_eq!(caps["provider"], "mlx");
    assert_eq!(caps["temperature"], 0.7);
    assert_eq!(caps["use_rag"], true);
    assert_eq!(caps["tools"], json!(["vector_search", "ocr_extract"]));
    assert_eq!(caps["mcp_servers"], json!([]));

    t.cleanup().await;
}

#[tokio::test]
async fn t2_list_excludes_persona_and_secrets() {
    let t = TestTenant::new(pool().await).await;
    t.seed_agent(SeedAgent {
        system_prompt: "SECRET PERSONA — do not leak",
        personality_traits: Some(json!(["warm", "professional"])),
        greeting: Some("Hello from test"),
        rag_params: Some(json!({"limit": 10, "secret_key": "hunter2"})),
        api_key: Some("apikey-leak-test"),
        ..SeedAgent::vanilla("vanilla-2")
    })
    .await;

    let (status, body) = send(&t.router(), get("/api/v1/agents", &t.tenant_id)).await;
    assert_eq!(status, StatusCode::OK);
    let raw = body.to_string();
    for forbidden in [
        "SECRET PERSONA",
        "system_prompt",
        "personality_traits",
        "greeting",
        "rag_params",
        "api_key",
        "apikey-leak-test",
        "hunter2",
    ] {
        assert!(
            !raw.contains(forbidden),
            "list response leaked `{forbidden}` — got: {raw}"
        );
    }

    t.cleanup().await;
}

#[tokio::test]
async fn t3_detail_by_id_returns_full_shape() {
    let t = TestTenant::new(pool().await).await;
    let id = t
        .seed_agent(SeedAgent {
            system_prompt: "Full persona text",
            greeting: Some("Hi"),
            personality_traits: Some(json!(["calm"])),
            tools: Some(json!(["graph_search"])),
            ..SeedAgent::vanilla("detail-1")
        })
        .await;

    let (status, body) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id), &t.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "detail-1");
    assert_eq!(body["system_prompt"], "Full persona text");
    assert_eq!(body["greeting"], "Hi");
    assert_eq!(body["personality_traits"], json!(["calm"]));
    assert_eq!(body["capabilities"]["tools"], json!(["graph_search"]));
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());

    t.cleanup().await;
}

#[tokio::test]
async fn t4_detail_by_name_returns_same_row_as_by_id() {
    let t = TestTenant::new(pool().await).await;
    let id = t
        .seed_agent(SeedAgent {
            ..SeedAgent::vanilla("detail-by-name")
        })
        .await;

    let (s_id, body_id) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id), &t.tenant_id),
    )
    .await;
    let (s_name, body_name) = send(
        &t.router(),
        get("/api/v1/agents/detail-by-name", &t.tenant_id),
    )
    .await;

    assert_eq!(s_id, StatusCode::OK);
    assert_eq!(s_name, StatusCode::OK);
    assert_eq!(body_id["id"], body_name["id"]);
    assert_eq!(body_id["name"], body_name["name"]);

    t.cleanup().await;
}

#[tokio::test]
async fn t5_detail_excludes_api_key_and_template_id() {
    let t = TestTenant::new(pool().await).await;
    let id = t
        .seed_agent(SeedAgent {
            api_key: Some("super-secret-key"),
            template_id: Some("internal-template-42"),
            ..SeedAgent::vanilla("excl-1")
        })
        .await;

    let (status, body) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id), &t.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let raw = body.to_string();
    for forbidden in [
        "api_key",
        "super-secret-key",
        "template_id",
        "internal-template-42",
    ] {
        assert!(
            !raw.contains(forbidden),
            "detail response leaked `{forbidden}` — got: {raw}"
        );
    }

    t.cleanup().await;
}

#[tokio::test]
async fn t6_detail_rag_params_whitelist_drops_unknown_keys() {
    let t = TestTenant::new(pool().await).await;
    let id = t
        .seed_agent(SeedAgent {
            rag_params: Some(json!({
                "limit": 10,
                "alpha": 0.5,
                "output_format": "json",
                "secret_key": "hunter2",
                "internal_collection": "private",
            })),
            ..SeedAgent::vanilla("rag-1")
        })
        .await;

    let (status, body) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id), &t.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rag = &body["rag_params"];
    assert_eq!(rag["limit"], 10);
    assert_eq!(rag["alpha"], 0.5);
    assert_eq!(rag["output_format"], "json");
    assert!(rag.get("secret_key").is_none());
    assert!(rag.get("internal_collection").is_none());

    let raw = body.to_string();
    assert!(!raw.contains("hunter2"));
    assert!(!raw.contains("internal_collection"));

    t.cleanup().await;
}

#[tokio::test]
async fn t7_cross_tenant_returns_404_with_neutral_body() {
    let pool = pool().await;
    let owner = TestTenant::new(pool.clone()).await;
    let probe = TestTenant::new(pool.clone()).await;

    let id = owner
        .seed_agent(SeedAgent {
            ..SeedAgent::vanilla("owned-by-A")
        })
        .await;

    let router = probe.router();
    let (status, body) = send(
        &router,
        get(&format!("/api/v1/agents/{}", id), &probe.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({"error": "agent_not_found"}));

    let (status, body) = send(
        &router,
        get("/api/v1/agents/owned-by-A", &probe.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({"error": "agent_not_found"}));

    owner.cleanup().await;
    probe.cleanup().await;
}

#[tokio::test]
async fn t9_null_json_columns_serialize_as_empty_array() {
    let t = TestTenant::new(pool().await).await;
    let id = t
        .seed_agent(SeedAgent {
            tools: None,
            mcp_servers: None,
            personality_traits: None,
            ..SeedAgent::vanilla("null-cols")
        })
        .await;

    let (status, body) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id), &t.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["capabilities"]["tools"], json!([]));
    assert_eq!(body["capabilities"]["mcp_servers"], json!([]));
    assert_eq!(body["personality_traits"], json!([]));

    t.cleanup().await;
}

#[tokio::test]
async fn t11_numeric_name_resolves_by_id() {
    let t = TestTenant::new(pool().await).await;
    let id_a = t
        .seed_agent(SeedAgent {
            ..SeedAgent::vanilla("alpha")
        })
        .await;
    let numeric_name = id_a.to_string();
    t.seed_agent(SeedAgent {
        name: &numeric_name,
        ..SeedAgent::vanilla(&numeric_name)
    })
    .await;

    let (status, body) = send(
        &t.router(),
        get(&format!("/api/v1/agents/{}", id_a), &t.tenant_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id_a);
    assert_eq!(
        body["name"], "alpha",
        "numeric path resolved by ID, not by name"
    );

    t.cleanup().await;
}

#[tokio::test]
async fn t14a_missing_credentials_returns_401() {
    let t = TestTenant::new(pool().await).await;
    let router = t.router();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/agents")
        .header("X-Forwarded-For", "10.0.0.1")
        // intentionally no X-Tenant-Id, no Authorization
        .body(Body::empty())
        .unwrap();
    let (status, _body) = send(&router, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    t.cleanup().await;
}

#[tokio::test]
async fn t14b_header_fallback_works_when_no_jwt_validator() {
    let t = TestTenant::new(pool().await).await;
    t.seed_agent(SeedAgent {
        ..SeedAgent::vanilla("hdr-fb")
    })
    .await;

    let (status, _) = send(&t.router(), get("/api/v1/agents/hdr-fb", &t.tenant_id)).await;
    assert_eq!(status, StatusCode::OK);

    t.cleanup().await;
}

// NOTE: T12 (rate limit) is intentionally omitted here. Mimir's burst is
// hard-coded to 60 in `public_read_routes`, and tower-governor doesn't
// expose a clean way to dial that down per-test without re-exporting the
// router builder with a parameter. The Bifrost suite covers the rate-limit
// behavior of the shared tower-governor config; the Mimir wiring matches
// it. Worth a separate follow-up if we want full coverage here.

// ─────────────────────────────────────────────────────────────────────────
// T15: real-TCP listener tests.
//
// The other tests use `Router::oneshot()`, which short-circuits axum's
// make-service layer — `ConnectInfo<SocketAddr>` is never populated. That
// hid a production-only bug where tower-governor's `SmartIpKeyExtractor`
// returned "Unable To Extract Key!" (500) on requests with no
// `X-Forwarded-For` header (same class as the Bifrost bug fixed in
// commit 03a44ce there, and fixed here in main.rs alongside this test).
// Bind a real ephemeral TCP listener with
// `into_make_service_with_connect_info` so peer-IP fallback is exercised.
// ─────────────────────────────────────────────────────────────────────────

mod tcp_listener_tests {
    use super::*;
    use std::net::SocketAddr;

    async fn spawn_server(t: &TestTenant) -> SocketAddr {
        let router = t.router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().expect("local_addr");
        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, svc).await;
        });
        tokio::task::yield_now().await;
        addr
    }

    fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn t15a_real_listener_no_xff_no_creds_returns_401_not_500() {
        // Regression guard: without ConnectInfo populated SmartIp would
        // 500 with "Unable To Extract Key!". With it, the request reaches
        // the auth middleware and 401s because no creds are present.
        let t = TestTenant::new(pool().await).await;
        let addr = spawn_server(&t).await;

        let resp = http_client()
            .get(format!("http://{addr}/api/v1/agents"))
            .send()
            .await
            .expect("send");

        assert_eq!(
            resp.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "expected 401 with ConnectInfo populated; got {}",
            resp.status()
        );

        t.cleanup().await;
    }

    #[tokio::test]
    async fn t15b_real_listener_no_xff_with_tenant_returns_200() {
        // Header fallback over real TCP — no `X-Forwarded-For`, so SmartIp
        // must reach the peer IP via ConnectInfo for the request to even
        // get past the rate-limit layer.
        let t = TestTenant::new(pool().await).await;
        t.seed_agent(SeedAgent {
            ..SeedAgent::vanilla("real-tcp-target")
        })
        .await;
        let addr = spawn_server(&t).await;

        let resp = http_client()
            .get(format!("http://{addr}/api/v1/agents"))
            .header("X-Tenant-Id", &t.tenant_id)
            .send()
            .await
            .expect("send");

        assert_eq!(
            resp.status(),
            reqwest::StatusCode::OK,
            "expected 200, got {}",
            resp.status()
        );

        t.cleanup().await;
    }
}
