//! REST `/fhir/*` tests (Nótt Local L1-1): read/vread/history/search/create/update
//! over the versioned store, `ETag`/`If-Match`, `OperationOutcome` on every error,
//! fail-closed without an agent. Every test ends with `Store::verify()`.
#![cfg(feature = "rest-axum")]

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::{Extension, Router};
use http_body_util::BodyExt;
use mimir_fhir::datatypes::{CodeableConcept, Reference};
use mimir_fhir::resources::{DiagnosticReport, DiagnosticReportStatus, Task};
use mimir_fhir::rest::{router, Agent, RestConfig, SharedStore, FHIR_JSON};
use mimir_fhir::store::Store;
use serde_json::{json, Value};
use tower::ServiceExt;

const DR: &str = "Practitioner/dr1";
const SCOPE: &str = "https://megawiz.co.th/fhir/CodeSystem/nott-scope";

struct Reply {
    status: StatusCode,
    etag: Option<String>,
    location: Option<String>,
    content_type: Option<String>,
    body: Value,
}

fn store() -> SharedStore {
    Arc::new(Mutex::new(Store::in_memory().unwrap()))
}

fn app(s: &SharedStore) -> Router {
    router(s.clone(), RestConfig::default()).layer(Extension(Agent(DR.into())))
}

async fn send(app: Router, req: Request<Body>) -> Reply {
    let res = app.oneshot(req).await.unwrap();
    let h = |n: header::HeaderName| {
        res.headers()
            .get(n)
            .map(|v| v.to_str().unwrap().to_string())
    };
    let (status, etag, location, content_type) = (
        res.status(),
        h(header::ETAG),
        h(header::LOCATION),
        h(header::CONTENT_TYPE),
    );
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    Reply {
        status,
        etag,
        location,
        content_type,
        body,
    }
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn write(method: &str, uri: &str, if_match: Option<&str>, body: &Value) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, FHIR_JSON);
    if let Some(v) = if_match {
        b = b.header(header::IF_MATCH, v);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

fn report_json() -> Value {
    let mut r = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        CodeableConcept::from_text("Sleep study"),
    );
    r.subject = Some(Reference::literal("Patient/p1"));
    serde_json::to_value(r).unwrap()
}

/// POST a preliminary report; returns its server-assigned id.
async fn create_report(s: &SharedStore) -> String {
    let r = send(
        app(s),
        write("POST", "/DiagnosticReport", None, &report_json()),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body["id"].as_str().unwrap().to_string()
}

fn assert_outcome(r: &Reply, status: StatusCode, code: &str) {
    assert_eq!(r.status, status, "{}", r.body);
    assert_eq!(r.body["resourceType"], "OperationOutcome");
    assert_eq!(r.body["issue"][0]["severity"], "error");
    assert_eq!(r.body["issue"][0]["code"], code, "{}", r.body);
    assert!(
        r.body["issue"][0]["details"]["text"]
            .as_str()
            .is_some_and(|t| !t.is_empty()),
        "Thai text for the user: {}",
        r.body
    );
}

fn assert_verified(s: &SharedStore) {
    assert!(
        s.lock().unwrap().verify().unwrap().is_ok(),
        "audit chain verifies"
    );
}

fn audit_len(s: &SharedStore) -> usize {
    s.lock().unwrap().audit_chain().unwrap().len()
}

#[tokio::test]
async fn create_ignores_client_id_and_returns_201_location_and_etag() {
    let s = store();
    let mut body = report_json();
    body["id"] = json!("client-chosen");
    let r = send(app(&s), write("POST", "/DiagnosticReport", None, &body)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let id = r.body["id"].as_str().unwrap();
    assert_ne!(id, "client-chosen", "server assigns the id on create");
    assert_eq!(r.body["meta"]["versionId"], "1");
    assert_eq!(r.etag.as_deref(), Some("W/\"1\""));
    assert_eq!(
        r.location.as_deref(),
        Some(format!("/fhir/DiagnosticReport/{id}/_history/1").as_str())
    );
    assert_eq!(r.content_type.as_deref(), Some(FHIR_JSON));
    assert_verified(&s);
}

#[tokio::test]
async fn read_returns_current_version_with_etag_and_unknown_is_404() {
    let s = store();
    let id = create_report(&s).await;
    let r = send(app(&s), get(&format!("/DiagnosticReport/{id}"))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.etag.as_deref(), Some("W/\"1\""));
    assert_eq!(r.body["status"], "preliminary");

    let missing = send(app(&s), get("/DiagnosticReport/nope")).await;
    assert_outcome(&missing, StatusCode::NOT_FOUND, "not-found");
    let bad_type = send(app(&s), get("/Spaceship/1")).await;
    assert_outcome(&bad_type, StatusCode::NOT_FOUND, "not-supported");
    assert_verified(&s);
}

#[tokio::test]
async fn update_with_if_match_bumps_version_and_vread_returns_old_version_verbatim() {
    let s = store();
    let id = create_report(&s).await;
    let v1 = send(app(&s), get(&format!("/DiagnosticReport/{id}"))).await;
    let mut body = v1.body.clone();
    body["conclusion"] = json!("ข้อสรุปฉบับแก้");
    let r = send(
        app(&s),
        write(
            "PUT",
            &format!("/DiagnosticReport/{id}"),
            Some("W/\"1\""),
            &body,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.etag.as_deref(), Some("W/\"2\""));
    assert_eq!(r.body["conclusion"], "ข้อสรุปฉบับแก้");

    let old = send(app(&s), get(&format!("/DiagnosticReport/{id}/_history/1"))).await;
    assert_eq!(old.status, StatusCode::OK);
    assert_eq!(old.etag.as_deref(), Some("W/\"1\""));
    assert_eq!(old.body, v1.body, "vread returns version 1 exactly");

    let none = send(app(&s), get(&format!("/DiagnosticReport/{id}/_history/9"))).await;
    assert_outcome(&none, StatusCode::NOT_FOUND, "not-found");
    let junk = send(app(&s), get(&format!("/DiagnosticReport/{id}/_history/x"))).await;
    assert_outcome(&junk, StatusCode::BAD_REQUEST, "invalid");
    assert_verified(&s);
}

#[tokio::test]
async fn stale_if_match_is_412_and_writes_nothing() {
    let s = store();
    let id = create_report(&s).await;
    let mut body = report_json();
    body["id"] = json!(id);
    let uri = format!("/DiagnosticReport/{id}");
    let first = send(app(&s), write("PUT", &uri, Some("W/\"1\""), &body)).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let before = audit_len(&s);
    let stale = send(app(&s), write("PUT", &uri, Some("W/\"1\""), &body)).await;
    assert_outcome(&stale, StatusCode::PRECONDITION_FAILED, "conflict");
    assert_eq!(audit_len(&s), before, "nothing written on conflict");
    assert_verified(&s);
}

#[tokio::test]
async fn update_preconditions_missing_if_match_id_mismatch_and_unknown_id() {
    let s = store();
    let id = create_report(&s).await;
    let mut body = report_json();
    body["id"] = json!(id);
    let uri = format!("/DiagnosticReport/{id}");

    let no_match = send(app(&s), write("PUT", &uri, None, &body)).await;
    assert_outcome(&no_match, StatusCode::PRECONDITION_REQUIRED, "required");
    let bad_match = send(app(&s), write("PUT", &uri, Some("v1"), &body)).await;
    assert_outcome(&bad_match, StatusCode::BAD_REQUEST, "invalid");

    let mismatch = send(
        app(&s),
        write("PUT", "/DiagnosticReport/other", Some("W/\"1\""), &body),
    )
    .await;
    assert_outcome(&mismatch, StatusCode::BAD_REQUEST, "invalid");

    let mut ghost = report_json();
    ghost["id"] = json!("ghost");
    let create_by_put = send(
        app(&s),
        write("PUT", "/DiagnosticReport/ghost", Some("W/\"1\""), &ghost),
    )
    .await;
    assert_outcome(
        &create_by_put,
        StatusCode::METHOD_NOT_ALLOWED,
        "not-supported",
    );
    assert_eq!(audit_len(&s), 1, "only the original create was written");
    assert_verified(&s);
}

#[tokio::test]
async fn if_match_may_be_optional_by_config() {
    let s = store();
    let id = create_report(&s).await;
    let mut body = report_json();
    body["id"] = json!(id);
    let lenient = router(
        s.clone(),
        RestConfig {
            require_if_match: false,
            ..RestConfig::default()
        },
    )
    .layer(Extension(Agent(DR.into())));
    let r = send(
        lenient,
        write("PUT", &format!("/DiagnosticReport/{id}"), None, &body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.etag.as_deref(), Some("W/\"2\""));
    assert_verified(&s);
}

#[tokio::test]
async fn history_is_a_bundle_newest_first_with_request_and_response() {
    let s = store();
    let id = create_report(&s).await;
    let mut body = report_json();
    body["id"] = json!(id);
    send(
        app(&s),
        write(
            "PUT",
            &format!("/DiagnosticReport/{id}"),
            Some("W/\"1\""),
            &body,
        ),
    )
    .await;
    let r = send(app(&s), get(&format!("/DiagnosticReport/{id}/_history"))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["resourceType"], "Bundle");
    assert_eq!(r.body["type"], "history");
    assert_eq!(r.body["total"], 2);
    let e = &r.body["entry"];
    assert_eq!(e[0]["resource"]["meta"]["versionId"], "2", "newest first");
    assert_eq!(e[0]["request"]["method"], "PUT");
    assert_eq!(e[0]["response"]["etag"], "W/\"2\"");
    assert_eq!(e[1]["request"]["method"], "POST");
    assert_eq!(e[1]["response"]["status"], "201");
    assert_eq!(
        e[1]["fullUrl"],
        format!("/fhir/DiagnosticReport/{id}").as_str()
    );

    let missing = send(app(&s), get("/DiagnosticReport/nope/_history")).await;
    assert_outcome(&missing, StatusCode::NOT_FOUND, "not-found");
    assert_verified(&s);
}

#[tokio::test]
async fn search_returns_searchset_and_rejects_unsupported_parameters() {
    let s = store();
    for owner in ["PractitionerRole/dr1", "PractitionerRole/dr2"] {
        let t = Task::requested(
            Reference::literal("DiagnosticReport/r"),
            Reference::literal("Patient/p1"),
        )
        .owned_by(Reference::literal(owner));
        let r = send(
            app(&s),
            write("POST", "/Task", None, &serde_json::to_value(t).unwrap()),
        )
        .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    }
    let r = send(
        app(&s),
        get("/Task?owner=PractitionerRole/dr1&status=requested"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["type"], "searchset");
    assert_eq!(r.body["total"], 1);
    assert_eq!(r.body["entry"][0]["search"]["mode"], "match");
    assert_eq!(
        r.body["entry"][0]["resource"]["owner"]["reference"],
        "PractitionerRole/dr1"
    );
    let all = send(app(&s), get("/Task")).await;
    assert_eq!(all.body["total"], 2);

    let unknown = send(app(&s), get("/Task?_sort=date")).await;
    assert_outcome(&unknown, StatusCode::BAD_REQUEST, "not-supported");
    let or = send(app(&s), get("/Task?status=requested,completed")).await;
    assert_outcome(&or, StatusCode::BAD_REQUEST, "not-supported");
    assert_verified(&s);
}

#[tokio::test]
async fn search_by_nott_scope_tag() {
    let s = store();
    let mut research = report_json();
    research["meta"] = json!({"tag": [{"system": SCOPE, "code": "research"}]});
    for body in [research, report_json()] {
        let r = send(app(&s), write("POST", "/DiagnosticReport", None, &body)).await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    }
    let q = format!("/DiagnosticReport?_tag={SCOPE}%7Cresearch");
    let r = send(app(&s), get(&q)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["total"], 1);
    assert_eq!(
        r.body["entry"][0]["resource"]["meta"]["tag"][0]["code"],
        "research"
    );
    assert_verified(&s);
}

#[tokio::test]
async fn malformed_bodies_are_400_and_wrong_media_type_is_415() {
    let s = store();
    let syntax = Request::post("/DiagnosticReport")
        .header(header::CONTENT_TYPE, FHIR_JSON)
        .body(Body::from("{not json"))
        .unwrap();
    assert_outcome(
        &send(app(&s), syntax).await,
        StatusCode::BAD_REQUEST,
        "structure",
    );

    let mut wrong_type = report_json();
    wrong_type["resourceType"] = json!("Patient");
    let r = send(
        app(&s),
        write("POST", "/DiagnosticReport", None, &wrong_type),
    )
    .await;
    assert_outcome(&r, StatusCode::BAD_REQUEST, "structure");

    let mut unknown_field = report_json();
    unknown_field["notAField"] = json!(1);
    let r = send(
        app(&s),
        write("POST", "/DiagnosticReport", None, &unknown_field),
    )
    .await;
    assert_outcome(&r, StatusCode::BAD_REQUEST, "structure");

    let text = Request::post("/DiagnosticReport")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(report_json().to_string()))
        .unwrap();
    assert_outcome(
        &send(app(&s), text).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "not-supported",
    );
    assert_eq!(audit_len(&s), 0);
    assert_verified(&s);
}

#[tokio::test]
async fn without_an_agent_every_request_is_401() {
    let s = store();
    let bare = || router(s.clone(), RestConfig::default());
    assert_outcome(
        &send(bare(), get("/DiagnosticReport/x")).await,
        StatusCode::UNAUTHORIZED,
        "login",
    );
    assert_outcome(
        &send(bare(), get("/Task?status=requested")).await,
        StatusCode::UNAUTHORIZED,
        "login",
    );
    let post = write("POST", "/DiagnosticReport", None, &report_json());
    assert_outcome(&send(bare(), post).await, StatusCode::UNAUTHORIZED, "login");
    let blank = bare().layer(Extension(Agent("  ".into())));
    assert_outcome(
        &send(blank, get("/DiagnosticReport/x")).await,
        StatusCode::UNAUTHORIZED,
        "login",
    );
    assert_eq!(audit_len(&s), 0);
    assert_verified(&s);
}

#[tokio::test]
async fn provenance_and_audit_event_are_read_only_here() {
    let s = store();
    for rtype in ["Provenance", "AuditEvent"] {
        let r = send(
            app(&s),
            write(
                "POST",
                &format!("/{rtype}"),
                None,
                &json!({ "resourceType": rtype }),
            ),
        )
        .await;
        assert_outcome(&r, StatusCode::METHOD_NOT_ALLOWED, "not-supported");
        let r = send(
            app(&s),
            write(
                "PUT",
                &format!("/{rtype}/x"),
                Some("W/\"1\""),
                &json!({ "resourceType": rtype, "id": "x" }),
            ),
        )
        .await;
        assert_outcome(&r, StatusCode::METHOD_NOT_ALLOWED, "not-supported");
    }
    assert_eq!(audit_len(&s), 0);
    assert_verified(&s);
}

#[tokio::test]
async fn signed_report_states_are_only_reachable_through_the_sign_flow() {
    let s = store();
    for status in ["final", "amended", "corrected", "appended"] {
        let mut body = report_json();
        body["status"] = json!(status);
        let r = send(app(&s), write("POST", "/DiagnosticReport", None, &body)).await;
        assert_outcome(&r, StatusCode::UNPROCESSABLE_ENTITY, "business-rule");
    }

    let id = create_report(&s).await;
    let uri = format!("/DiagnosticReport/{id}");
    let mut sign = report_json();
    sign["id"] = json!(id);
    sign["status"] = json!("final");
    let r = send(app(&s), write("PUT", &uri, Some("W/\"1\""), &sign)).await;
    assert_outcome(&r, StatusCode::UNPROCESSABLE_ENTITY, "business-rule");

    // A report signed by the sign flow (simulated straight on the store) cannot be
    // overwritten here, not even back to preliminary.
    {
        let mut st = s.lock().unwrap();
        let mut signed = st.read::<DiagnosticReport>(&id).unwrap().unwrap();
        signed.status = DiagnosticReportStatus::Final;
        st.update(signed, Some(1), DR).unwrap();
    }
    let mut back = report_json();
    back["id"] = json!(id);
    let r = send(app(&s), write("PUT", &uri, Some("W/\"2\""), &back)).await;
    assert_outcome(&r, StatusCode::UNPROCESSABLE_ENTITY, "business-rule");
    assert_eq!(audit_len(&s), 2, "create + simulated sign only");
    assert_verified(&s);
}
