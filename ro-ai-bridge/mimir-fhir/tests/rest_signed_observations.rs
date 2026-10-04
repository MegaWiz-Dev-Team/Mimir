//! Observations of a signed `DiagnosticReport` cannot be changed through REST
//! (contract api-v1 v1.2 §3a): the signature covers their versions.
#![cfg(feature = "rest-axum")]

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::{Extension, Router};
use http_body_util::BodyExt;
use mimir_fhir::datatypes::{CodeableConcept, Reference};
use mimir_fhir::resources::{
    DiagnosticReport, DiagnosticReportStatus, Observation, ObservationStatus,
};
use mimir_fhir::rest::{router, Agent, RestConfig, SharedStore, FHIR_JSON};
use mimir_fhir::store::Store;
use serde_json::{json, Value};
use tower::ServiceExt;

const ENGINE: &str = "Device/nott-engine";

fn app(s: &SharedStore) -> Router {
    router(s.clone(), RestConfig::default()).layer(Extension(Agent("PractitionerRole/t1".into())))
}

/// An Observation and a report listing it; returns (observation id, report id).
fn seed(s: &SharedStore, status: DiagnosticReportStatus) -> (String, String) {
    let mut st = s.lock().unwrap();
    let mut o = Observation::new(ObservationStatus::Final, CodeableConcept::from_text("AHI"));
    o.subject = Some(Reference::literal("Patient/p1"));
    let o = st.create(o, ENGINE).unwrap();
    let oid = o.id.unwrap().to_string();
    let mut r = DiagnosticReport::new(status, CodeableConcept::from_text("Sleep study"));
    r.subject = Some(Reference::literal("Patient/p1"));
    r.result
        .push(Reference::literal(format!("Observation/{oid}")));
    let r = st.create(r, ENGINE).unwrap();
    (oid, r.id.unwrap().to_string())
}

async fn put_observation(s: &SharedStore, oid: &str) -> (StatusCode, Value) {
    let current = s.lock().unwrap().read::<Observation>(oid).unwrap().unwrap();
    let mut body = serde_json::to_value(current).unwrap();
    body["valueString"] = json!("edited after the fact");
    let req = Request::put(format!("/Observation/{oid}"))
        .header(header::CONTENT_TYPE, FHIR_JSON)
        .header(header::IF_MATCH, "W/\"1\"")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app(s).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn writes(s: &SharedStore) -> usize {
    s.lock()
        .unwrap()
        .audit_chain()
        .unwrap()
        .iter()
        .filter(|e| e.resource_type != "AuditEvent")
        .count()
}

#[tokio::test]
async fn an_observation_of_a_preliminary_report_can_still_be_edited() {
    let s = Arc::new(Mutex::new(Store::in_memory().unwrap()));
    let (oid, _) = seed(&s, DiagnosticReportStatus::Preliminary);
    let (status, body) = put_observation(&s, &oid).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(s.lock().unwrap().verify().unwrap().is_ok());
}

#[tokio::test]
async fn an_observation_of_a_signed_report_is_frozen() {
    for signed in [
        DiagnosticReportStatus::Final,
        DiagnosticReportStatus::Amended,
        DiagnosticReportStatus::Corrected,
        DiagnosticReportStatus::Appended,
    ] {
        let s = Arc::new(Mutex::new(Store::in_memory().unwrap()));
        let (oid, rid) = seed(&s, signed);
        let before = writes(&s);
        let (status, body) = put_observation(&s, &oid).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{signed:?}: {body}"
        );
        assert_eq!(body["issue"][0]["code"], "business-rule");
        assert!(
            body["issue"][0]["diagnostics"]
                .as_str()
                .unwrap()
                .contains(&format!("DiagnosticReport/{rid}")),
            "{body}"
        );
        assert_eq!(writes(&s), before, "nothing written");
        assert!(s.lock().unwrap().verify().unwrap().is_ok());
    }
}

#[tokio::test]
async fn an_observation_no_report_lists_is_not_affected() {
    let s = Arc::new(Mutex::new(Store::in_memory().unwrap()));
    seed(&s, DiagnosticReportStatus::Final);
    let lone = {
        let mut st = s.lock().unwrap();
        let o = Observation::new(ObservationStatus::Final, CodeableConcept::from_text("ODI"));
        st.create(o, ENGINE).unwrap().id.unwrap().to_string()
    };
    let (status, body) = put_observation(&s, &lone).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
