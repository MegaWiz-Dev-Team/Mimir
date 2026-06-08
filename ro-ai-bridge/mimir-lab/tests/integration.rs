//! mimir-lab MVP integration tests (TDD).

use mimir_lab::{engine::Engine, ingest, ingest::SourceFormat, pii, pii::PiiStatus, LabError};

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn read_only_query_returns_rows() {
    let e = Engine::in_memory().unwrap();
    let r = e
        .query_readonly("SELECT 1 AS a, 'hi' AS b", 100)
        .unwrap();
    assert_eq!(r.columns.len(), 2);
    assert_eq!(r.columns[0].name, "a");
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0][0].as_deref(), Some("1"));
    assert_eq!(r.rows[0][1].as_deref(), Some("hi"));
    assert!(!r.truncated);
}

#[test]
fn read_only_guard_rejects_mutations() {
    let e = Engine::in_memory().unwrap();
    for bad in ["INSERT INTO t VALUES (1)", "DROP TABLE t", "UPDATE t SET x=1", "  delete from t"] {
        let err = e.query_readonly(bad, 10).unwrap_err();
        assert!(matches!(err, LabError::NotReadOnly(_)), "should reject: {bad}");
    }
}

#[test]
fn row_cap_truncates() {
    let e = Engine::in_memory().unwrap();
    let r = e
        .query_readonly("SELECT * FROM range(10) AS t(n)", 3)
        .unwrap();
    assert_eq!(r.rows.len(), 3);
    assert!(r.truncated);
}

#[test]
fn ingest_csv_infers_schema_and_counts_rows() {
    let e = Engine::in_memory().unwrap();
    let res = ingest::ingest_csv(&e, &fixture("people.csv"), "people").unwrap();
    assert_eq!(res.row_count, 4);
    let names = res.schema.column_names();
    assert_eq!(names, vec!["id", "name", "age", "city"]);
    // numeric columns inferred as an integer type, text as VARCHAR
    let age = res.schema.columns.iter().find(|c| c.name == "age").unwrap();
    assert!(age.sql_type.to_uppercase().contains("INT"), "age type was {}", age.sql_type);
    let city = res.schema.columns.iter().find(|c| c.name == "city").unwrap();
    assert!(city.sql_type.to_uppercase().contains("VARCHAR"), "city type was {}", city.sql_type);
}

#[test]
fn ingest_rejects_bad_table_name() {
    let e = Engine::in_memory().unwrap();
    let err = ingest::ingest_csv(&e, &fixture("people.csv"), "drop table; --").unwrap_err();
    assert!(matches!(err, LabError::BadIdent(_)));
}

#[test]
fn source_format_from_path() {
    assert_eq!(SourceFormat::from_path("/a/b.csv").unwrap(), SourceFormat::Csv);
    assert_eq!(SourceFormat::from_path("x.parquet").unwrap(), SourceFormat::Parquet);
    assert_eq!(SourceFormat::from_path("x.JSON").unwrap(), SourceFormat::Json);
    assert!(SourceFormat::from_path("x.geojson").is_err());
}

#[test]
fn ingest_json_infers_schema() {
    let e = Engine::in_memory().unwrap();
    let res = ingest::ingest_file(&e, &fixture("people.json"), "people_j").unwrap();
    assert_eq!(res.row_count, 3);
    assert!(res.schema.column_names().contains(&"city"));
}

#[test]
fn parquet_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let pq = format!("{}/people.parquet", dir.path().display());
    let e = Engine::in_memory().unwrap();
    // ingest CSV → export Parquet → ingest Parquet back
    ingest::ingest_csv(&e, &fixture("people.csv"), "src").unwrap();
    ingest::export_parquet(&e, "src", &pq).unwrap();
    let res = ingest::ingest_file(&e, &pq, "from_pq").unwrap();
    assert_eq!(res.row_count, 4);
    assert_eq!(res.schema.column_names(), vec!["id", "name", "age", "city"]);
}

#[test]
fn pii_gate_clean_on_benign_text() {
    let status = pii::scan_samples(&[
        "Bangkok".to_string(),
        "annual revenue grew 12%".to_string(),
    ]);
    assert_eq!(status, PiiStatus::Clean);
}

#[test]
fn pii_gate_flags_pii() {
    // Thai national ID + email should trip the Skuggi Tier-1 finders.
    let status = pii::scan_samples(&[
        "ผู้ป่วย 1234567890123".to_string(),
        "contact me at somchai@example.com".to_string(),
    ]);
    match status {
        PiiStatus::Flagged { categories } => assert!(!categories.is_empty()),
        other => panic!("expected Flagged, got {other:?}"),
    }
}

#[test]
fn pii_gate_over_ingested_column() {
    let e = Engine::in_memory().unwrap();
    ingest::ingest_csv(&e, &fixture("people.csv"), "people").unwrap();
    // 'city' column is benign place names → Clean
    let status = pii::gate_table_column(&e, "people", "city", 100).unwrap();
    assert_eq!(status, PiiStatus::Clean);
}
