//! Store tests for ADR-006 Amendment 1: current + history + audit written in one
//! transaction; vread/_history; If-Match; search; tamper detection; rollback.
#![cfg(feature = "store-sqlite")]

use mimir_fhir::datatypes::{Code, CodeableConcept, Coding, Identifier, Meta, Reference, Uri};
use mimir_fhir::resources::{DiagnosticReport, DiagnosticReportStatus, Patient, Task, TaskStatus};
use mimir_fhir::store::{Store, StoreError};

const DR: &str = "Practitioner/dr1";

fn report() -> DiagnosticReport {
    let mut r = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        CodeableConcept::from_text("Sleep study"),
    );
    r.subject = Some(Reference::literal("Patient/p1"));
    r
}

#[test]
fn create_assigns_id_version_and_timestamp() {
    let mut s = Store::in_memory().unwrap();
    let r = s.create(report(), "Device/nott").unwrap();
    let meta = r.meta.as_ref().unwrap();
    assert_eq!(meta.version_id.as_deref(), Some("1"));
    assert!(meta.last_updated.is_some());
    let id = r.id.as_ref().unwrap().to_string();
    assert_eq!(s.read::<DiagnosticReport>(&id).unwrap().unwrap(), r);
}

#[test]
fn update_bumps_version_and_vread_returns_the_signed_version_verbatim() {
    let mut s = Store::in_memory().unwrap();
    let v1 = s.create(report(), "Device/nott").unwrap();
    let id = v1.id.clone().unwrap().to_string();
    let mut signed = v1.clone();
    signed.status = DiagnosticReportStatus::Final;
    signed
        .results_interpreter
        .push(Reference::literal("PractitionerRole/dr1"));
    let v2 = s.update(signed, Some(1), DR).unwrap();
    assert_eq!(v2.meta.as_ref().unwrap().version_id.as_deref(), Some("2"));
    let mut amended = v2.clone();
    amended.status = DiagnosticReportStatus::Amended;
    s.update(amended, Some(2), DR).unwrap();

    let got = s.vread::<DiagnosticReport>(&id, 2).unwrap().unwrap();
    assert_eq!(got, v2, "the signed version comes back exactly");
    let hist = s.history::<DiagnosticReport>(&id).unwrap();
    assert_eq!(hist.len(), 3);
    assert_eq!(
        hist[0].status,
        DiagnosticReportStatus::Amended,
        "newest first"
    );
    assert_eq!(
        s.read::<DiagnosticReport>(&id).unwrap().unwrap().status,
        DiagnosticReportStatus::Amended
    );
}

#[test]
fn if_match_conflict_writes_nothing() {
    let mut s = Store::in_memory().unwrap();
    let v1 = s.create(report(), "Device/nott").unwrap();
    s.update(v1.clone(), Some(1), DR).unwrap();
    let err = s.update(v1, Some(1), DR).unwrap_err();
    assert!(matches!(
        err,
        StoreError::VersionConflict {
            expected: 1,
            current: 2,
            ..
        }
    ));
    assert_eq!(s.audit_chain().unwrap().len(), 2);
}

#[test]
fn create_with_existing_id_and_missing_agent_are_errors() {
    let mut s = Store::in_memory().unwrap();
    let v1 = s.create(report(), "Device/nott").unwrap();
    assert!(matches!(
        s.create(v1, "Device/nott").unwrap_err(),
        StoreError::AlreadyExists(..)
    ));
    assert!(matches!(
        s.create(report(), "  ").unwrap_err(),
        StoreError::MissingAgent
    ));
}

#[test]
fn failed_audit_insert_rolls_back_current_and_history() {
    let mut s = Store::in_memory().unwrap();
    s.connection()
        .execute_batch(
            "CREATE TRIGGER fail_audit BEFORE INSERT ON audit_chain BEGIN SELECT RAISE(ABORT, 'audit sink down'); END;",
        )
        .unwrap();
    assert!(s.create(report(), "Device/nott").is_err());
    let rows: i64 = s
        .connection()
        .query_row(
            "SELECT (SELECT count(*) FROM resource) + (SELECT count(*) FROM resource_history)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0, "no current or history row without its audit entry");
}

#[test]
fn history_and_audit_are_append_only() {
    let mut s = Store::in_memory().unwrap();
    s.create(report(), "Device/nott").unwrap();
    assert!(s
        .connection()
        .execute("UPDATE resource_history SET json = '{}'", [])
        .is_err());
    assert!(s
        .connection()
        .execute("DELETE FROM audit_chain", [])
        .is_err());
}

#[test]
fn verify_detects_a_tampered_chain() {
    let mut s = Store::in_memory().unwrap();
    let v1 = s.create(report(), "Device/nott").unwrap();
    s.update(v1, None, DR).unwrap();
    assert_eq!(s.verify().unwrap(), Ok(2));
    // bypass the trigger the way an attacker with file access would, then re-check
    s.connection().execute_batch("DROP TRIGGER history_no_update; UPDATE resource_history SET json = replace(json, 'preliminary', 'final') WHERE version_id = 1;").unwrap();
    assert_eq!(s.verify().unwrap(), Err(1));
}

#[test]
fn search_worklist_by_owner_and_status_and_patient_by_identifier() {
    let mut s = Store::in_memory().unwrap();
    for owner in [
        "PractitionerRole/dr1",
        "PractitionerRole/dr1",
        "PractitionerRole/dr2",
    ] {
        let t = Task::requested(
            Reference::literal("DiagnosticReport/r"),
            Reference::literal("Patient/p1"),
        )
        .owned_by(Reference::literal(owner));
        s.create(t, "Device/nott").unwrap();
    }
    let mine = s
        .search::<Task>(&[("owner", "PractitionerRole/dr1"), ("status", "requested")])
        .unwrap();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().all(|t| t.status == TaskStatus::Requested));

    let hn = Uri::new("https://fhir.moph.go.th/identifier/hn").unwrap();
    let mut p = Patient::default();
    p.identifier.push(Identifier::new(hn.clone(), "HN-0001"));
    s.create(p, "Practitioner/tech1").unwrap();
    assert_eq!(
        s.search::<Patient>(&[(
            "identifier",
            "https://fhir.moph.go.th/identifier/hn|HN-0001"
        )])
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        s.search::<Patient>(&[("identifier", "HN-0001")])
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        s.search::<Patient>(&[("identifier", "other|HN-0001")])
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn search_by_tag_matches_system_and_code() {
    const SCOPE: &str = "https://megawiz.co.th/fhir/CodeSystem/nott-scope";
    let mut s = Store::in_memory().unwrap();
    let mut research = report();
    research.meta = Some(Meta::default());
    research.meta.as_mut().unwrap().tag.push(Coding::new(
        Uri::new(SCOPE).unwrap(),
        Code::new("research").unwrap(),
    ));
    s.create(research, "Device/nott").unwrap();
    s.create(report(), "Device/nott").unwrap();

    let tagged = |q: &str| s.search::<DiagnosticReport>(&[("_tag", q)]).unwrap().len();
    assert_eq!(tagged(&format!("{SCOPE}|research")), 1);
    assert_eq!(tagged("research"), 1, "code alone matches any system");
    assert_eq!(tagged("other-system|research"), 0);
    assert_eq!(tagged(&format!("{SCOPE}|clinical")), 0);
}

#[test]
fn search_by_result_finds_the_reports_that_reference_an_observation() {
    let mut s = Store::in_memory().unwrap();
    let mut with = report();
    with.result.push(Reference::literal("Observation/o1"));
    with.result.push(Reference::literal("Observation/o2"));
    s.create(with, "Device/nott").unwrap();
    s.create(report(), "Device/nott").unwrap();
    let found = |o: &str| {
        s.search::<DiagnosticReport>(&[("result", o)])
            .unwrap()
            .len()
    };
    assert_eq!(found("Observation/o2"), 1);
    assert_eq!(found("Observation/o9"), 0);
}

#[test]
fn patient_search_follows_each_types_own_element() {
    use mimir_fhir::datatypes::Instant;
    use mimir_fhir::resources::{AuditEvent, AuditEventAgent, AuditEventSource, Provenance};
    let mut s = Store::in_memory().unwrap();
    let p1 = || Reference::literal("Patient/p1");
    // R5 AuditEvent and Provenance name the patient in `patient`, not `subject`.
    let mut event = AuditEvent::new(
        CodeableConcept::from_text("read"),
        Instant::new("2026-10-04T07:00:00.000Z").unwrap(),
        AuditEventAgent {
            type_: None,
            role: Vec::new(),
            who: Reference::literal("PractitionerRole/dr1"),
            requestor: Some(true),
        },
        AuditEventSource {
            observer: Reference::literal("Device/nott"),
        },
    );
    event.patient = Some(p1());
    s.create(event, "Device/nott").unwrap();
    let mut prov = Provenance::of(
        Reference::literal("DiagnosticReport/r/_history/1"),
        Reference::literal("Device/nott"),
    );
    prov.patient = Some(p1());
    s.create(prov, "Device/nott").unwrap();
    // Task names it in `for`.
    s.create(
        Task::requested(Reference::literal("DiagnosticReport/r"), p1()),
        "Device/nott",
    )
    .unwrap();
    // Most others in `subject`.
    s.create(report(), "Device/nott").unwrap();

    let q = [("patient", "Patient/p1")];
    let other = [("patient", "Patient/p2")];
    assert_eq!(
        s.search::<AuditEvent>(&q).unwrap().len(),
        1,
        "AuditEvent.patient"
    );
    assert_eq!(
        s.search::<Provenance>(&q).unwrap().len(),
        1,
        "Provenance.patient"
    );
    assert_eq!(s.search::<Task>(&q).unwrap().len(), 1, "Task.for");
    assert_eq!(
        s.search::<DiagnosticReport>(&q).unwrap().len(),
        1,
        "subject"
    );
    assert!(s.search::<AuditEvent>(&other).unwrap().is_empty());
    assert!(s.search::<Task>(&other).unwrap().is_empty());
}

#[test]
fn audit_events_are_found_by_the_entity_they_name_any_version() {
    use mimir_fhir::datatypes::Instant;
    use mimir_fhir::resources::{AuditEvent, AuditEventAgent, AuditEventEntity, AuditEventSource};
    let mut s = Store::in_memory().unwrap();
    let event = |what: &str| {
        let mut e = AuditEvent::new(
            CodeableConcept::from_text("read"),
            Instant::new("2026-10-04T07:00:00.000Z").unwrap(),
            AuditEventAgent {
                type_: None,
                role: Vec::new(),
                who: Reference::literal("PractitionerRole/dr1"),
                requestor: Some(true),
            },
            AuditEventSource {
                observer: Reference::literal("Device/nott"),
            },
        );
        e.entity.push(AuditEventEntity {
            what: Some(Reference::literal(what)),
            role: None,
        });
        e
    };
    for what in [
        "DiagnosticReport/r1",
        "DiagnosticReport/r1/_history/2",
        "DiagnosticReport/r10",
        "DiagnosticReport/r2",
    ] {
        s.create(event(what), "Device/nott").unwrap();
    }
    let found = |q: &str| s.search::<AuditEvent>(&[("entity", q)]).unwrap().len();
    assert_eq!(
        found("DiagnosticReport/r1"),
        2,
        "the resource and its versions, not r10"
    );
    assert_eq!(
        found("DiagnosticReport/r1/_history/2"),
        1,
        "a version, exactly"
    );
    assert_eq!(found("DiagnosticReport/r9"), 0);
    assert!(matches!(
        s.search::<Task>(&[("entity", "DiagnosticReport/r1")])
            .unwrap_err(),
        StoreError::UnsupportedSearchParam(_)
    ));
}

#[test]
fn unknown_search_parameter_is_an_error_not_ignored() {
    let s = Store::in_memory().unwrap();
    assert!(matches!(
        s.search::<Task>(&[("not-a-param", "x")]).unwrap_err(),
        StoreError::UnsupportedSearchParam(_)
    ));
}

#[test]
fn store_persists_across_reopen() {
    let dir = std::env::temp_dir().join(format!("mimir-fhir-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fhir.sqlite");
    let id = {
        let mut s = Store::open(&path).unwrap();
        s.create(report(), "Device/nott")
            .unwrap()
            .id
            .unwrap()
            .to_string()
    };
    let s = Store::open(&path).unwrap();
    assert!(s.read::<DiagnosticReport>(&id).unwrap().is_some());
    assert_eq!(s.verify().unwrap(), Ok(1));
    let _ = Code::new("x");
    std::fs::remove_dir_all(&dir).ok();
}
