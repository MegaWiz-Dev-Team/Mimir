//! HttpTyrSink → Tyr forwarding test (hermetic, via wiremock — no live Tyr).

use mimir_lab::{AuditEvent, AuditSink, HttpTyrSink};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn http_tyr_sink_forwards_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/audit"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let sink = HttpTyrSink::spawn(format!("{}/audit", server.uri()), Some("Bearer t".into()));
    sink.record(&AuditEvent {
        action: "analytics.query",
        tenant_id: Some("asgard_analytics".into()),
        actor: Some("analyst-sql".into()),
        target: Some("SELECT * FROM sales".into()),
        outcome: "ok",
        detail: Some("rows=10 truncated=false".into()),
    });

    // drain is async — wait for the POST to land
    let mut reqs = Vec::new();
    for _ in 0..100 {
        reqs = server.received_requests().await.unwrap();
        if !reqs.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(reqs.len(), 1, "expected exactly one POST to Tyr");

    let body: serde_json::Value = serde_json::from_slice(&reqs[0].body).unwrap();
    assert_eq!(body["asgard:action"], "analytics.query");
    assert_eq!(body["asgard:tenant_id"], "asgard_analytics");
    assert_eq!(body["asgard:outcome"], "ok");
    assert_eq!(body["asgard:component"], "mimir-lab");
    assert_eq!(body["asgard:actor"], "analyst-sql");
    assert!(body["asgard:emitted_at"].is_string());
}
