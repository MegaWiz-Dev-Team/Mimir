//! analytics-api server test (hermetic) — POST /query over a temp parquet dir.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mimir_lab::audit::NoopAuditSink;
use mimir_lab::server::{router, AppState};
use mimir_lab::{ingest, Engine};
use std::sync::Arc;
use tower::ServiceExt;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

async fn post(app: axum::Router, path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

fn app_with_people() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    // materialise people.parquet in the data dir
    let e = Engine::in_memory().unwrap();
    ingest::ingest_csv(&e, &fixture("people.csv"), "people").unwrap();
    ingest::export_parquet(&e, "people", &format!("{}/people.parquet", dir.path().display())).unwrap();

    let state = AppState {
        data_dir: Arc::new(dir.path().to_string_lossy().to_string()),
        audit: Arc::new(NoopAuditSink),
        registry: None,
    };
    (router(state), dir)
}

#[tokio::test]
async fn query_endpoint_returns_rows_over_parquet() {
    let (app, _dir) = app_with_people();
    let (status, v) = post(
        app,
        "/api/v1/analytics/query",
        serde_json::json!({
            "tenant_id": "asgard_analytics",
            "sql": "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY city"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["row_count"], 3);
    assert_eq!(v["columns"][0]["name"], "city");
    assert_eq!(v["rows"][0][0], "Bangkok");
}

#[tokio::test]
async fn query_endpoint_rejects_mutation_with_400() {
    let (app, _dir) = app_with_people();
    let (status, v) = post(
        app,
        "/api/v1/analytics/query",
        serde_json::json!({ "tenant_id": "asgard_analytics", "sql": "DROP TABLE people" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("read-only"));
}

#[tokio::test]
async fn plot_endpoint_returns_echarts_option() {
    let (app, _dir) = app_with_people();
    let (status, v) = post(
        app,
        "/api/v1/analytics/plot",
        serde_json::json!({
            "tenant_id": "asgard_analytics",
            "sql": "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY city",
            "chart_type": "bar", "x": "city", "y": "n"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["echarts"]["series"][0]["type"], "bar");
    assert_eq!(v["echarts"]["xAxis"]["data"][0], "Bangkok");
}
