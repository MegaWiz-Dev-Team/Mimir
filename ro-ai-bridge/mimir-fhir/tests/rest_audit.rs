//! Nótt Local L1-2: one `AuditEvent` per authenticated `/fhir` request, written into
//! the same hash-chained store; a request whose audit record fails returns no data.
#![cfg(feature = "rest-axum")]

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::{Extension, Router};
use http_body_util::BodyExt;
use mimir_fhir::datatypes::{CodeableConcept, Reference};
use mimir_fhir::resources::{AuditEvent, DiagnosticReport, DiagnosticReportStatus};
use mimir_fhir::rest::{router, Agent, RestConfig, SharedStore, FHIR_JSON};
use mimir_fhir::store::Store;
use serde_json::{json, Value};
use tower::ServiceExt;

const DR: &str = "PractitionerRole/dr1";
const OBSERVER: &str = "Device/nott-local";

fn store() -> SharedStore {
    Arc::new(Mutex::new(Store::in_memory().unwrap()))
}

fn config() -> RestConfig {
    RestConfig {
        audit_observer: Some(OBSERVER.into()),
        ..RestConfig::default()
    }
}

fn app(s: &SharedStore, cfg: RestConfig) -> Router {
    router(s.clone(), cfg).layer(Extension(Agent(DR.into())))
}

async fn send(app: Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn post(uri: &str, body: &Value) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, FHIR_JSON)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn report(patient: &str) -> Value {
    let mut r = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        CodeableConcept::from_text("Sleep study"),
    );
    r.subject = Some(Reference::literal(patient));
    serde_json::to_value(r).unwrap()
}

/// Seed straight into the store (no request, no `AuditEvent`).
fn seed(s: &SharedStore, patient: &str) -> String {
    let r: DiagnosticReport = serde_json::from_value(report(patient)).unwrap();
    let stored = s.lock().unwrap().create(r, "Device/nott").unwrap();
    stored.id.unwrap().to_string()
}

/// `AuditEvent`s as JSON in the order they were written — audit-chain sequence, not
/// `meta.lastUpdated`: two events can share a millisecond.
fn events(s: &SharedStore) -> Vec<Value> {
    let st = s.lock().unwrap();
    st.audit_chain()
        .unwrap()
        .into_iter()
        .filter(|e| e.resource_type == "AuditEvent")
        .map(|e| {
            let ev = st
                .vread::<AuditEvent>(&e.id, e.version_id)
                .unwrap()
                .unwrap();
            serde_json::to_value(ev).unwrap()
        })
        .collect()
}

fn entity_refs(e: &Value) -> Vec<String> {
    e["entity"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| x["what"]["reference"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn assert_verified(s: &SharedStore) {
    assert!(s.lock().unwrap().verify().unwrap().is_ok());
}

#[tokio::test]
async fn read_records_who_what_patient_and_outcome() {
    let s = store();
    let id = seed(&s, "Patient/p1");
    let (status, _) = send(app(&s, config()), get(&format!("/DiagnosticReport/{id}"))).await;
    assert_eq!(status, StatusCode::OK);

    let ev = events(&s);
    assert_eq!(ev.len(), 1);
    let e = &ev[0];
    assert_eq!(e["category"][0]["coding"][0]["code"], "rest");
    assert_eq!(e["code"]["coding"][0]["code"], "read");
    assert_eq!(
        e["code"]["coding"][0]["system"],
        "http://hl7.org/fhir/restful-interaction"
    );
    assert_eq!(e["action"], "R");
    assert_eq!(e["outcome"]["code"]["code"], "0");
    assert_eq!(e["agent"][0]["who"]["reference"], DR);
    assert_eq!(e["agent"][0]["requestor"], true);
    assert_eq!(e["source"]["observer"]["reference"], OBSERVER);
    assert_eq!(e["patient"]["reference"], "Patient/p1");
    assert_eq!(
        entity_refs(e),
        [
            format!("DiagnosticReport/{id}/_history/1"),
            "Patient/p1".to_string()
        ]
    );
    assert_verified(&s);
}

#[tokio::test]
async fn search_across_patients_names_every_patient_but_no_single_patient() {
    let s = store();
    let a = seed(&s, "Patient/p1");
    let b = seed(&s, "Patient/p2");
    let (status, body) = send(
        app(&s, config()),
        get("/DiagnosticReport?status=preliminary"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);

    let e = &events(&s)[0];
    assert_eq!(e["action"], "E");
    assert_eq!(e["code"]["coding"][0]["code"], "search-type");
    assert!(
        e.get("patient").is_none(),
        "two patients: no single patient"
    );
    let refs = entity_refs(e);
    for r in [
        format!("DiagnosticReport/{a}/_history/1"),
        format!("DiagnosticReport/{b}/_history/1"),
        "Patient/p1".into(),
        "Patient/p2".into(),
    ] {
        assert!(refs.contains(&r), "{r} in {refs:?}");
    }
    assert_verified(&s);
}

#[tokio::test]
async fn writes_record_create_and_update_with_the_version_written() {
    let s = store();
    let (status, body) = send(
        app(&s, config()),
        post("/DiagnosticReport", &report("Patient/p1")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().unwrap().to_string();
    let mut v2 = body.clone();
    v2["conclusion"] = json!("แก้ไข");
    let put = Request::put(format!("/DiagnosticReport/{id}"))
        .header(header::CONTENT_TYPE, FHIR_JSON)
        .header(header::IF_MATCH, "W/\"1\"")
        .body(Body::from(v2.to_string()))
        .unwrap();
    assert_eq!(send(app(&s, config()), put).await.0, StatusCode::OK);

    let ev = events(&s);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0]["action"], "C");
    assert_eq!(ev[0]["code"]["coding"][0]["code"], "create");
    assert_eq!(
        entity_refs(&ev[0])[0],
        format!("DiagnosticReport/{id}/_history/1")
    );
    assert_eq!(ev[1]["action"], "U");
    assert_eq!(
        entity_refs(&ev[1])[0],
        format!("DiagnosticReport/{id}/_history/2")
    );
    assert_verified(&s);
}

#[tokio::test]
async fn failed_requests_are_recorded_with_a_failure_outcome() {
    let s = store();
    let (status, _) = send(app(&s, config()), get("/DiagnosticReport/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let e = &events(&s)[0];
    assert_eq!(e["outcome"]["code"]["code"], "4");
    assert_eq!(e["outcome"]["detail"][0]["text"], "HTTP 404");
    assert_eq!(entity_refs(e), ["DiagnosticReport/nope"]);
    assert_verified(&s);
}

#[tokio::test]
async fn unauthenticated_requests_are_not_recorded_here() {
    let s = store();
    let bare = router(s.clone(), config());
    let (status, _) = send(bare, get("/DiagnosticReport/x")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(events(&s).is_empty(), "no agent: the auth layer logs it");
    assert_verified(&s);
}

#[tokio::test]
async fn audit_can_be_turned_off() {
    let s = store();
    let id = seed(&s, "Patient/p1");
    let off = RestConfig {
        audit_observer: None,
        ..RestConfig::default()
    };
    let (status, _) = send(app(&s, off), get(&format!("/DiagnosticReport/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(events(&s).is_empty());
    assert_verified(&s);
}

#[tokio::test]
async fn audit_is_on_by_default() {
    let s = store();
    let id = seed(&s, "Patient/p1");
    let (status, _) = send(
        app(&s, RestConfig::default()),
        get(&format!("/DiagnosticReport/{id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events(&s).len(), 1);
    assert_verified(&s);
}

#[tokio::test]
async fn when_the_audit_record_fails_no_data_is_returned() {
    let s = store();
    let id = seed(&s, "Patient/p1");
    s.lock()
        .unwrap()
        .connection()
        .execute_batch(
            "CREATE TRIGGER audit_down BEFORE INSERT ON resource WHEN NEW.type = 'AuditEvent' \
             BEGIN SELECT RAISE(ABORT, 'audit sink down'); END;",
        )
        .unwrap();
    let (status, body) = send(app(&s, config()), get(&format!("/DiagnosticReport/{id}"))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert!(!body.to_string().contains("Patient/p1"), "{body}");
    assert_verified(&s);
}
