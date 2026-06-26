//! TDD tests for FHIR R5 `DateTime` primitive.
//!
//! FHIR R5 dateTime allows 4 precisions: YYYY, YYYY-MM, YYYY-MM-DD, full RFC3339.

use mimir_fhir::datatypes::{DateTime, DateTimeError};

// --- Happy path — all 4 precisions ---

#[test]
fn datetime_accepts_year_only() {
    assert!(DateTime::new("2026").is_ok());
    assert!(DateTime::new("1973").is_ok());
}

#[test]
fn datetime_accepts_year_month() {
    assert!(DateTime::new("2026-05").is_ok());
    assert!(DateTime::new("1905-08").is_ok());
}

#[test]
fn datetime_accepts_full_date() {
    assert!(DateTime::new("2026-05-24").is_ok());
    assert!(DateTime::new("2024-02-29").is_ok()); // leap year valid
}

#[test]
fn datetime_accepts_full_datetime_with_tz() {
    assert!(DateTime::new("2026-05-24T13:00:00+07:00").is_ok());
    assert!(DateTime::new("2026-05-24T06:00:00Z").is_ok());
    assert!(DateTime::new("2017-01-01T00:00:00.000Z").is_ok());
}

// --- Rejects ---

#[test]
fn datetime_rejects_invalid_month() {
    assert!(matches!(
        DateTime::new("2026-13"),
        Err(DateTimeError::InvalidFormat(_))
    ));
    assert!(matches!(
        DateTime::new("2026-00"),
        Err(DateTimeError::InvalidFormat(_))
    ));
}

#[test]
fn datetime_rejects_non_leap_feb_29() {
    // 2025 is not a leap year — Feb 29 must fail
    assert!(matches!(
        DateTime::new("2025-02-29"),
        Err(DateTimeError::InvalidFormat(_))
    ));
}

#[test]
fn datetime_rejects_malformed() {
    assert!(DateTime::new("26-05-24").is_err()); // 2-digit year
    assert!(DateTime::new("2026/05/24").is_err()); // slash separator
    assert!(DateTime::new("2026-5-24").is_err()); // single-digit month
    assert!(DateTime::new("2026-05-24T").is_err()); // dangling T
    assert!(DateTime::new("").is_err());
    assert!(DateTime::new("not a date").is_err());
}

#[test]
fn datetime_rejects_missing_timezone_on_full_datetime() {
    // FHIR R5 requires timezone offset on full datetimes
    assert!(DateTime::new("2026-05-24T13:00:00").is_err());
}

// --- Serde round-trip ---

#[test]
fn datetime_serializes_as_string() {
    let dt = DateTime::new("2026-05-24T13:00:00+07:00").unwrap();
    let json = serde_json::to_string(&dt).unwrap();
    assert_eq!(json, "\"2026-05-24T13:00:00+07:00\"");
}

#[test]
fn datetime_round_trips_all_precisions() {
    for sample in ["2026", "2026-05", "2026-05-24", "2026-05-24T13:00:00+07:00"] {
        let original = DateTime::new(sample).unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let restored: DateTime = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored, "round trip failed for {sample}");
    }
}

#[test]
fn datetime_deserialize_rejects_invalid_grammar() {
    let result: Result<DateTime, _> = serde_json::from_str("\"not a date\"");
    assert!(result.is_err());
}
