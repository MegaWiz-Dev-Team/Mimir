//! TDD tests for the resources added for the Nótt Local reading/sign-off
//! workflow: wire names (R5), required fields, value-set codes, strict-out /
//! lenient-in, and the `FhirResource` trait.

use mimir_fhir::datatypes::{
    Attachment, Code, CodeableConcept, CodeableReference, Coding, Id, Instant, Reference,
    Signature, Uri,
};
use mimir_fhir::resources::{
    AuditEvent, AuditEventAction, Binary, Consent, ConsentStatus, ConversionError, Device,
    DeviceName, DeviceNameType, DeviceVersion, DiagnosticReport, DiagnosticReportStatus,
    DocumentReference, DocumentReferenceRelatesTo, DocumentReferenceStatus, ExternalPractitioner,
    ExternalServiceRequest, FhirResource, Observation, ObservationStatus, Organization,
    Practitioner, PractitionerRole, Provenance, ProvenanceEntity, ProvenanceEntityRole,
    RequestIntent, RequestStatus, ServiceRequest, Task, TaskStatus,
};
use serde_json::json;

fn loinc(code: &str, display: &str) -> CodeableConcept {
    CodeableConcept::from_coding(
        Coding::new(
            Uri::new("http://loinc.org").unwrap(),
            Code::new(code).unwrap(),
        )
        .with_display(display),
    )
}

#[test]
fn service_request_requires_subject_and_uses_codeable_reference_code() {
    let sr = ServiceRequest::order(Reference::literal("Patient/p1"))
        .with_code(loinc("28633-6", "Sleep study"));
    let v = serde_json::to_value(&sr).unwrap();
    assert_eq!(v["resourceType"], "ServiceRequest");
    assert_eq!(v["status"], "active");
    assert_eq!(v["intent"], "order");
    assert_eq!(v["subject"]["reference"], "Patient/p1");
    // R5: code is CodeableReference → nested under "concept"
    assert_eq!(v["code"]["concept"]["coding"][0]["code"], "28633-6");
}

#[test]
fn external_service_request_missing_subject_is_rejected() {
    let ext: ExternalServiceRequest =
        serde_json::from_value(json!({"status":"active","intent":"order","unknownField":1}))
            .unwrap();
    assert_eq!(
        ServiceRequest::try_from(ext).unwrap_err(),
        ConversionError::MissingRequiredField {
            resource: "ServiceRequest",
            field: "subject"
        }
    );
}

#[test]
fn request_codes_are_kebab_case() {
    assert_eq!(
        serde_json::to_value(RequestStatus::EnteredInError).unwrap(),
        "entered-in-error"
    );
    assert_eq!(
        serde_json::to_value(RequestIntent::OriginalOrder).unwrap(),
        "original-order"
    );
    assert_eq!(
        serde_json::to_value(TaskStatus::InProgress).unwrap(),
        "in-progress"
    );
}

#[test]
fn task_for_field_serializes_as_for_and_owner_is_a_reference() {
    let t = Task::requested(
        Reference::literal("DiagnosticReport/r1"),
        Reference::literal("Patient/p1"),
    )
    .owned_by(Reference::literal("PractitionerRole/dr1"));
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["for"]["reference"], "Patient/p1");
    assert_eq!(v["owner"]["reference"], "PractitionerRole/dr1");
    assert_eq!(v["status"], "requested");
    assert!(v.get("for_").is_none());
}

#[test]
fn observation_rejects_two_values_and_unknown_fields() {
    let mut o = Observation::new(ObservationStatus::Preliminary, loinc("x", "AHI"));
    o.value_string = Some("a".into());
    o.value_boolean = Some(true);
    assert!(o.validate().is_err());
    let bad = json!({"resourceType":"Observation","status":"final","code":{"text":"x"},"nope":1});
    assert!(serde_json::from_value::<Observation>(bad).is_err());
}

#[test]
fn diagnostic_report_lifecycle_codes_and_presented_form() {
    let mut r = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        loinc("28633-6", "Sleep study"),
    );
    r.presented_form.push(
        Attachment::by_url(
            Code::new("application/pdf").unwrap(),
            "Binary/pdf1",
            123_456,
        )
        .with_title("report"),
    );
    r.status = DiagnosticReportStatus::Final;
    r.results_interpreter
        .push(Reference::literal("PractitionerRole/dr1"));
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["status"], "final");
    assert_eq!(
        v["resultsInterpreter"][0]["reference"],
        "PractitionerRole/dr1"
    );
    // integer64 is a JSON string in R5
    assert_eq!(v["presentedForm"][0]["size"], "123456");
    let back: DiagnosticReport = serde_json::from_value(v).unwrap();
    assert_eq!(back.presented_form[0].size, Some(123_456));
}

#[test]
fn document_reference_replaces_previous_version() {
    let mut d = DocumentReference::current(Attachment::by_url(
        Code::new("application/json").unwrap(),
        "Binary/events-v2",
        2048,
    ));
    d.relates_to.push(DocumentReferenceRelatesTo {
        code: CodeableConcept::from_text("replaces"),
        target: Reference::literal("DocumentReference/events-v1"),
    });
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["status"], "current");
    assert_eq!(
        v["relatesTo"][0]["target"]["reference"],
        "DocumentReference/events-v1"
    );
    assert_eq!(
        serde_json::to_value(DocumentReferenceStatus::EnteredInError).unwrap(),
        "entered-in-error"
    );
    // content is 1..* and required
    assert!(serde_json::from_value::<DocumentReference>(
        json!({"resourceType":"DocumentReference","status":"current"})
    )
    .is_err());
}

#[test]
fn binary_requires_content_type_and_has_no_narrative() {
    assert!(serde_json::from_value::<Binary>(json!({"resourceType":"Binary"})).is_err());
    assert!(serde_json::from_value::<Binary>(
        json!({"resourceType":"Binary","contentType":"application/pdf","text":{}})
    )
    .is_err());
    let b = Binary::external(Code::new("application/octet-stream").unwrap());
    assert_eq!(
        serde_json::to_value(&b).unwrap()["contentType"],
        "application/octet-stream"
    );
}

#[test]
fn device_carries_software_versions_with_r5_names() {
    let d = Device {
        manufacturer: Some("Mega Wiz Co., Ltd.".into()),
        name: vec![DeviceName {
            value: "Asgard Nótt".into(),
            type_: DeviceNameType::RegisteredName,
        }],
        version: vec![DeviceVersion {
            type_: Some(CodeableConcept::from_text("engine")),
            value: "0.16.0".into(),
        }],
        ..Device::default()
    };
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["name"][0]["type"], "registered-name");
    assert_eq!(v["version"][0]["value"], "0.16.0");
}

#[test]
fn provenance_records_a_verification_signature_on_a_versioned_target() {
    let who = Reference::literal("PractitionerRole/dr1");
    let mut p = Provenance::of(
        Reference::literal("DiagnosticReport/r1/_history/3"),
        who.clone(),
    );
    p.entity.push(ProvenanceEntity {
        role: ProvenanceEntityRole::Revision,
        what: Reference::literal("DocumentReference/events-v2"),
    });
    p.signature.push(Signature::verification(
        who,
        Instant::new("2026-10-04T10:00:00.000Z").unwrap(),
    ));
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(
        v["target"][0]["reference"],
        "DiagnosticReport/r1/_history/3"
    );
    assert_eq!(
        v["signature"][0]["type"][0]["code"],
        "1.2.840.10065.1.12.1.5"
    );
    assert_eq!(v["entity"][0]["role"], "revision");
    // target and agent are required
    assert!(
        serde_json::from_value::<Provenance>(json!({"resourceType":"Provenance","agent":[]}))
            .is_err()
    );
}

#[test]
fn audit_event_requires_recorded_agent_source() {
    let v = json!({
        "resourceType":"AuditEvent","code":{"text":"read"},"action":"R",
        "recorded":"2026-10-04T10:00:00Z",
        "agent":[{"who":{"reference":"PractitionerRole/dr1"}}],
        "source":{"observer":{"reference":"Device/nott"}}
    });
    let a: AuditEvent = serde_json::from_value(v).unwrap();
    assert_eq!(a.action, Some(AuditEventAction::R));
    assert!(serde_json::from_value::<AuditEvent>(
        json!({"resourceType":"AuditEvent","code":{"text":"x"}})
    )
    .is_err());
}

#[test]
fn consent_status_and_decision_codes() {
    let c: Consent = serde_json::from_value(json!({
        "resourceType":"Consent","status":"active","decision":"permit",
        "provision":[{"purpose":[{"system":"http://terminology.hl7.org/CodeSystem/v3-ActReason","code":"TREAT"}]}]
    })).unwrap();
    assert_eq!(c.status, ConsentStatus::Active);
    assert_eq!(
        serde_json::to_value(ConsentStatus::NotDone).unwrap(),
        "not-done"
    );
}

#[test]
fn practitioner_role_and_external_practitioner() {
    let ext: ExternalPractitioner =
        serde_json::from_value(json!({"active":true,"unmodelled":"x"})).unwrap();
    let p: Practitioner = ext.into();
    assert_eq!(p.active, Some(true));
    let role = PractitionerRole {
        practitioner: Some(Reference::literal("Practitioner/1")),
        organization: Some(Reference::literal("Organization/h1")),
        code: vec![CodeableConcept::from_text("physician")],
        ..PractitionerRole::default()
    };
    assert_eq!(
        serde_json::to_value(&role).unwrap()["resourceType"],
        "PractitionerRole"
    );
    assert_eq!(
        Organization::named("โรงพยาบาลตัวอย่าง").name.as_deref(),
        Some("โรงพยาบาลตัวอย่าง")
    );
}

#[test]
fn fhir_resource_trait_reports_type_and_ids() {
    assert_eq!(<Task as FhirResource>::RESOURCE_TYPE, "Task");
    assert_eq!(
        <DiagnosticReport as FhirResource>::RESOURCE_TYPE,
        "DiagnosticReport"
    );
    let mut b = Binary::external(Code::new("application/pdf").unwrap());
    b.set_id(Id::new("abc").unwrap());
    assert_eq!(b.id().unwrap().as_ref(), "abc");
    let _ = CodeableReference::reference(Reference::literal("Condition/1"));
}
