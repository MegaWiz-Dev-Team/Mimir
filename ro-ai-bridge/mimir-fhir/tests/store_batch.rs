//! `Store::batch`: several writes in one transaction — all or none — with the audit
//! chain still verifying afterwards.
#![cfg(feature = "store-sqlite")]

use mimir_fhir::datatypes::{CodeableConcept, Reference};
use mimir_fhir::resources::{
    DiagnosticReport, DiagnosticReportStatus, Observation, ObservationStatus, Provenance,
};
use mimir_fhir::store::{Store, StoreError};

const ENGINE: &str = "Device/nott-engine";

fn observation() -> Observation {
    let mut o = Observation::new(ObservationStatus::Final, CodeableConcept::from_text("AHI"));
    o.subject = Some(Reference::literal("Patient/p1"));
    o
}

fn report(id: &str) -> DiagnosticReport {
    let mut r = DiagnosticReport::new(
        DiagnosticReportStatus::Preliminary,
        CodeableConcept::from_text("Sleep study"),
    );
    r.id = Some(mimir_fhir::datatypes::Id::new(id).unwrap());
    r.subject = Some(Reference::literal("Patient/p1"));
    r
}

fn rows(s: &Store) -> i64 {
    s.connection()
        .query_row(
            "SELECT (SELECT count(*) FROM resource) + (SELECT count(*) FROM resource_history) \
             + (SELECT count(*) FROM audit_chain)",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn a_batch_writes_every_resource_with_one_agent_and_the_chain_verifies() {
    let mut s = Store::in_memory().unwrap();
    let (obs, rep) = s
        .batch(ENGINE, |b| {
            let o = b.create(observation())?;
            let mut r = report("study-1");
            r.result.push(Reference::literal(format!(
                "Observation/{}",
                o.id.as_ref().unwrap()
            )));
            let r = b.create(r)?;
            Ok((o, r))
        })
        .unwrap();
    assert_eq!(
        rep.id.as_ref().unwrap().to_string(),
        "study-1",
        "caller's id kept"
    );
    assert!(s
        .read::<Observation>(obs.id.unwrap().as_ref())
        .unwrap()
        .is_some());
    assert!(s.read::<DiagnosticReport>("study-1").unwrap().is_some());
    let chain = s.audit_chain().unwrap();
    assert_eq!(chain.len(), 2);
    assert!(chain.iter().all(|e| e.agent == ENGINE));
    assert_eq!(s.verify().unwrap(), Ok(2));
}

#[test]
fn an_error_returned_partway_rolls_back_the_whole_batch() {
    let mut s = Store::in_memory().unwrap();
    s.create(report("existing"), ENGINE).unwrap();
    let before = rows(&s);
    let err = s
        .batch(ENGINE, |b| {
            b.create(observation())?;
            b.create(report("existing"))?; // already exists → error after one write
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::AlreadyExists(..)));
    assert_eq!(rows(&s), before, "the first write is gone too");
    assert_eq!(s.verify().unwrap(), Ok(1));
}

#[test]
fn a_database_failure_partway_rolls_back_the_whole_batch() {
    let mut s = Store::in_memory().unwrap();
    s.connection()
        .execute_batch(
            "CREATE TRIGGER no_provenance BEFORE INSERT ON resource WHEN NEW.type = 'Provenance' \
             BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
        )
        .unwrap();
    let res = s.batch(ENGINE, |b| {
        let r = b.create(report("study-2"))?;
        b.create(Provenance::of(
            Reference::literal("DiagnosticReport/study-2/_history/1"),
            Reference::literal(ENGINE),
        ))?;
        Ok(r)
    });
    assert!(matches!(res, Err(StoreError::Sqlite(_))));
    assert_eq!(rows(&s), 0);
    assert_eq!(s.verify().unwrap(), Ok(0));
}

#[test]
fn reads_inside_a_batch_see_its_own_writes_and_updates_bump_versions() {
    let mut s = Store::in_memory().unwrap();
    s.batch(ENGINE, |b| {
        b.create(report("study-3"))?;
        let mut r = b
            .read::<DiagnosticReport>("study-3")?
            .expect("own write visible");
        r.conclusion = Some("edited in the same batch".into());
        let v2 = b.update(r, Some(1))?;
        assert_eq!(v2.meta.as_ref().unwrap().version_id.as_deref(), Some("2"));
        Ok(())
    })
    .unwrap();
    assert!(s
        .vread::<DiagnosticReport>("study-3", 1)
        .unwrap()
        .unwrap()
        .conclusion
        .is_none());
    assert_eq!(s.verify().unwrap(), Ok(2));
}

#[test]
fn a_version_conflict_inside_a_batch_rolls_it_back() {
    let mut s = Store::in_memory().unwrap();
    let v1 = s.create(report("study-4"), ENGINE).unwrap();
    s.update(v1.clone(), Some(1), ENGINE).unwrap();
    let before = rows(&s);
    let err = s
        .batch("Practitioner/dr1", |b| {
            b.create(observation())?;
            b.update(v1, Some(1)) // stale
        })
        .unwrap_err();
    assert!(matches!(
        err,
        StoreError::VersionConflict {
            expected: 1,
            current: 2,
            ..
        }
    ));
    assert_eq!(rows(&s), before);
    assert_eq!(s.verify().unwrap(), Ok(2));
}

#[test]
fn history_sha256_is_the_hash_recorded_in_the_audit_chain() {
    let mut s = Store::in_memory().unwrap();
    let in_batch = s
        .batch(ENGINE, |b| {
            b.create(report("study-5"))?;
            b.history_sha256("DiagnosticReport", "study-5", 1)
        })
        .unwrap()
        .unwrap();
    let chain = s.audit_chain().unwrap();
    assert_eq!(in_batch, chain[0].sha256);
    assert_eq!(
        s.history_sha256("DiagnosticReport", "study-5", 1)
            .unwrap()
            .as_deref(),
        Some(chain[0].sha256.as_str())
    );
    assert_eq!(
        s.history_sha256("DiagnosticReport", "study-5", 2).unwrap(),
        None
    );
}

#[test]
fn a_batch_needs_an_agent() {
    let mut s = Store::in_memory().unwrap();
    let err = s.batch("  ", |b| b.create(report("study-6"))).unwrap_err();
    assert!(matches!(err, StoreError::MissingAgent));
    assert_eq!(rows(&s), 0);
}
