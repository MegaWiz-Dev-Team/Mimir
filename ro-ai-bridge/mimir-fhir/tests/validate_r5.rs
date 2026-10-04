//! FHIR R5 conformance before a write (`validators::r5`): required elements at any
//! depth, reference target types, and the error-level invariants that need no
//! `resolve()` — each with a failing and a passing case. The store refuses an invalid
//! resource and writes nothing, in a batch too; REST answers 422.

use mimir_fhir::validators::r5::{fhir_version, validate, Issue};
use serde_json::{json, Value};

fn issues(rt: &str, v: &Value) -> Vec<Issue> {
    validate(rt, v)
}

#[track_caller]
fn valid(rt: &str, v: &Value) {
    let found = issues(rt, v);
    assert!(found.is_empty(), "{rt} should be valid: {found:?}");
}

#[track_caller]
fn refused(rt: &str, v: &Value, path: &str, rule: &str) {
    let found = issues(rt, v);
    assert!(
        found.iter().any(|i| i.path == path && i.rule == rule),
        "expected {rule} at {path}, got {found:?}"
    );
}

fn observation() -> Value {
    json!({ "resourceType": "Observation", "status": "final", "code": { "text": "AHI" } })
}

#[test]
fn the_rules_come_from_fhir_5_0_0() {
    assert_eq!(fhir_version(), "5.0.0");
}

#[test]
fn a_reference_names_a_type_the_element_allows() {
    let mut r = json!({ "resourceType": "DiagnosticReport", "status": "final",
        "code": { "text": "Sleep study" }, "subject": { "reference": "Patient/p1" },
        "resultsInterpreter": [{ "reference": "PractitionerRole/r1", "display": "นพ. ก" }] });
    valid("DiagnosticReport", &r);
    for performer in [
        "Device/engine",
        "Device/engine/_history/2",
        "https://nott.local/fhir/Device/engine",
    ] {
        r["performer"] = json!([{ "reference": performer }]);
        refused(
            "DiagnosticReport",
            &r,
            "DiagnosticReport.performer[0]",
            "reference-target",
        );
    }
    // A urn cannot be read for a type; Provenance.target takes any resource.
    r["performer"] = json!([{ "reference": "urn:uuid:7b1c7a52-3a4f-4b7e-9a39-0d3c4b1a2e11" }]);
    valid("DiagnosticReport", &r);
    let p = json!({ "resourceType": "Provenance",
        "target": [{ "reference": "Binary/b1/_history/1" }],
        "agent": [{ "who": { "reference": "Device/engine" } }],
        "signature": [{ "type": [{ "code": "1.2.840.10065.1.12.1.5" }], "when": "2026-10-04T00:00:00Z",
                         "who": { "reference": "Encounter/e1" } }] });
    refused(
        "Provenance",
        &p,
        "Provenance.signature[0].who",
        "reference-target",
    );
}

#[test]
fn required_elements_are_present_at_every_depth() {
    let p = json!({ "resourceType": "Provenance", "target": [],
        "agent": [{ "role": [{ "text": "author" }] }] });
    refused("Provenance", &p, "Provenance.target", "required");
    refused("Provenance", &p, "Provenance.agent[0].who", "required");
    let d = json!({ "resourceType": "DocumentReference", "status": "current", "content": [] });
    refused(
        "DocumentReference",
        &d,
        "DocumentReference.content",
        "required",
    );
    let a = json!({ "resourceType": "AuditEvent", "code": { "text": "x" },
        "recorded": "2026-10-04T00:00:00Z", "agent": [], "source": { "observer": { "reference": "Device/d" } } });
    refused("AuditEvent", &a, "AuditEvent.agent", "required");
    let mut o = observation();
    o["extension"] = json!([{ "valueString": "no url" }]);
    refused(
        "Observation",
        &o,
        "Observation.extension[0].url",
        "required",
    );
    let ok = json!({ "resourceType": "AuditEvent", "code": { "text": "x" },
        "recorded": "2026-10-04T00:00:00Z", "agent": [{ "who": { "reference": "Device/d" } }],
        "source": { "observer": { "reference": "Device/d" } } });
    valid("AuditEvent", &ok);
}

#[test]
fn organization_invariants() {
    refused(
        "Organization",
        &json!({ "resourceType": "Organization" }),
        "Organization",
        "org-1",
    );
    valid(
        "Organization",
        &json!({ "resourceType": "Organization",
            "identifier": [{ "system": "https://megawiz.co.th/fhir/NamingSystem/nott-site", "value": "site" }] }),
    );
    let home = json!({ "resourceType": "Organization", "name": "x",
        "contact": [{ "telecom": [{ "system": "phone", "value": "1", "use": "home" }],
                      "address": { "text": "a", "use": "home" } }] });
    refused("Organization", &home, "Organization.contact[0]", "org-3");
    refused("Organization", &home, "Organization.contact[0]", "org-4");
    valid(
        "Organization",
        &json!({ "resourceType": "Organization", "name": "x",
            "contact": [{ "telecom": [{ "system": "phone", "value": "1", "use": "work" }], "address": { "text": "a" } }] }),
    );
}

#[test]
fn observation_invariants() {
    let mut o = observation();
    o["valueQuantity"] =
        json!({ "value": 1, "unit": "/h", "system": "http://unitsofmeasure.org", "code": "/h" });
    valid("Observation", &o);
    let mut absent = o.clone();
    absent["dataAbsentReason"] = json!({ "text": "no flow" });
    refused("Observation", &absent, "Observation", "obs-6");
    let mut own = o.clone();
    own["code"] = json!({ "coding": [{ "system": "https://x", "code": "ahi" }] });
    own["component"] = json!([{ "code": { "coding": [{ "system": "https://x", "code": "ahi" }] }, "valueString": "x" }]);
    refused("Observation", &own, "Observation", "obs-7");
    let mut body = observation();
    body["bodySite"] = json!({ "text": "finger" });
    body["bodyStructure"] = json!({ "reference": "BodyStructure/b" });
    refused("Observation", &body, "Observation", "obs-8");
    let mut range = observation();
    range["referenceRange"] = json!([{ "type": { "text": "normal" } }]);
    refused(
        "Observation",
        &range,
        "Observation.referenceRange[0]",
        "obs-3",
    );
    // component.referenceRange is the same element (contentReference).
    let mut inner = observation();
    inner["component"] = json!([{ "code": { "text": "odi" }, "referenceRange": [{}] }]);
    refused(
        "Observation",
        &inner,
        "Observation.component[0].referenceRange[0]",
        "obs-3",
    );
    range["referenceRange"] = json!([{ "text": "< 5 /h" }]);
    valid("Observation", &range);
}

#[test]
fn patient_device_and_task_invariants() {
    refused(
        "Patient",
        &json!({ "resourceType": "Patient", "contact": [{ "gender": "female" }] }),
        "Patient.contact[0]",
        "pat-1",
    );
    valid(
        "Patient",
        &json!({ "resourceType": "Patient", "contact": [{ "name": { "text": "ญาติ" } }] }),
    );
    let names = json!([{ "value": "A", "type": "registered-name", "display": true },
                       { "value": "B", "type": "user-friendly-name", "display": true }]);
    refused(
        "Device",
        &json!({ "resourceType": "Device", "name": names }),
        "Device",
        "dev-1",
    );
    let task = |extra: Value| {
        let mut t = json!({ "resourceType": "Task", "status": "requested", "intent": "order" });
        t.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        t
    };
    refused(
        "Task",
        &task(
            json!({ "authoredOn": "2026-10-04T10:00:00+07:00", "lastModified": "2026-10-04T02:00:00Z" }),
        ),
        "Task",
        "inv-1",
    );
    valid(
        "Task",
        &task(
            json!({ "authoredOn": "2026-10-04T10:00:00+07:00", "lastModified": "2026-10-04T03:00:00Z" }),
        ),
    );
    refused(
        "Task",
        &task(json!({ "restriction": { "repetitions": 1 } })),
        "Task",
        "tsk-1",
    );
}

#[test]
fn datatype_invariants() {
    let mut report = json!({ "resourceType": "DiagnosticReport", "status": "final",
        "code": { "text": "Sleep study" }, "subject": {} });
    refused(
        "DiagnosticReport",
        &report,
        "DiagnosticReport.subject",
        "ref-2",
    );
    report["subject"] = json!({ "reference": "#p" });
    refused(
        "DiagnosticReport",
        &report,
        "DiagnosticReport.subject",
        "ref-1",
    );
    let doc = json!({ "resourceType": "DocumentReference", "status": "current",
        "content": [{ "attachment": { "data": "JVBERi0=" } }] });
    refused(
        "DocumentReference",
        &doc,
        "DocumentReference.content[0].attachment",
        "att-1",
    );
    let mut quantity = observation();
    quantity["valueQuantity"] = json!({ "value": 1, "code": "/h" });
    refused(
        "Observation",
        &quantity,
        "Observation.valueQuantity",
        "qty-3",
    );
    let mut period = observation();
    period["effectivePeriod"] =
        json!({ "start": "2026-10-05T00:00:00Z", "end": "2026-10-04T00:00:00Z" });
    refused(
        "Observation",
        &period,
        "Observation.effectivePeriod",
        "per-1",
    );
    period["effectivePeriod"] = json!({ "start": "2026-10-04", "end": "2026-10-05" });
    valid("Observation", &period);
    refused(
        "Patient",
        &json!({ "resourceType": "Patient", "telecom": [{ "value": "02-000-0000" }] }),
        "Patient.telecom[0]",
        "cpt-2",
    );
    let mut ext = observation();
    ext["extension"] = json!([{ "url": "https://x", "valueString": "a",
        "extension": [{ "url": "https://y", "valueString": "b" }] }]);
    refused("Observation", &ext, "Observation.extension[0]", "ext-1");
    ext["extension"] = json!([{ "url": "https://x", "valueString": "a" }]);
    valid("Observation", &ext);
}

#[cfg(feature = "store-sqlite")]
#[test]
fn the_store_refuses_an_invalid_resource_and_writes_nothing() {
    use mimir_fhir::datatypes::{CodeableConcept, Reference};
    use mimir_fhir::resources::{DiagnosticReport, DiagnosticReportStatus, Organization};
    use mimir_fhir::store::{Store, StoreError};

    let mut store = Store::in_memory().unwrap();
    let err = store
        .create(Organization::default(), "Device/test")
        .unwrap_err();
    match &err {
        StoreError::Invalid { resource, issues } => {
            assert!(resource.starts_with("Organization/"), "{resource}");
            assert!(issues.iter().any(|i| i.rule == "org-1"), "{issues:?}");
        }
        other => panic!("{other:?}"),
    }
    assert!(err.to_string().contains("org-1"), "{err}");
    assert!(store.search::<Organization>(&[]).unwrap().is_empty());
    assert!(
        store.audit_chain().unwrap().is_empty(),
        "nothing was written"
    );

    // In a batch, the valid write before it goes too.
    let mut report = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        CodeableConcept::from_text("Sleep study"),
    );
    report.performer = vec![Reference::literal("Device/engine")];
    let r = store.batch("Device/test", |b| {
        b.create(Organization::named("ok"))?;
        b.create(report.clone())?;
        Ok(())
    });
    assert!(
        matches!(&r, Err(StoreError::Invalid { issues, .. }) if issues.iter().any(|i| i.rule == "reference-target")),
        "{r:?}"
    );
    assert!(store.search::<Organization>(&[]).unwrap().is_empty());
    assert!(store.verify().unwrap().is_ok());
    store
        .create(Organization::named("ok"), "Device/test")
        .unwrap();
}

#[cfg(feature = "rest-axum")]
#[tokio::test]
async fn rest_answers_422_with_the_rule() {
    use std::sync::{Arc, Mutex};

    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::Extension;
    use http_body_util::BodyExt;
    use mimir_fhir::rest::{router, Agent, RestConfig, FHIR_JSON};
    use mimir_fhir::store::Store;
    use tower::ServiceExt;

    let store = Arc::new(Mutex::new(Store::in_memory().unwrap()));
    let app =
        router(store, RestConfig::default()).layer(Extension(Agent("Practitioner/dr1".into())));
    let req = Request::post("/Organization")
        .header(header::CONTENT_TYPE, FHIR_JSON)
        .body(Body::from(
            json!({ "resourceType": "Organization" }).to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["issue"][0]["code"], "processing");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .unwrap()
            .contains("org-1"),
        "{body}"
    );
}
