//! TDD tests for the FHIR R5 `Date` primitive (partial precision, no time).

use mimir_fhir::datatypes::{Date, DateError};

#[test]
fn date_accepts_full_date() {
    let d = Date::new("2026-05-24").unwrap();
    assert_eq!(d.as_str(), "2026-05-24");
}

#[test]
fn date_accepts_year_only() {
    assert!(Date::new("1958").is_ok());
}

#[test]
fn date_accepts_year_month() {
    assert!(Date::new("2026-05").is_ok());
}

#[test]
fn date_rejects_datetime_with_time() {
    // A `dateTime` instant is NOT a valid `date`.
    let err = Date::new("2026-05-24T10:00:00Z").unwrap_err();
    assert!(matches!(err, DateError::InvalidFormat(_)));
}

#[test]
fn date_rejects_impossible_month() {
    assert!(Date::new("2026-13").is_err());
}

#[test]
fn date_rejects_impossible_day() {
    assert!(Date::new("2026-02-30").is_err());
}

#[test]
fn date_rejects_garbage_and_empty() {
    assert!(Date::new("not-a-date").is_err());
    assert!(Date::new("").is_err());
}

#[test]
fn date_serializes_transparently() {
    let d = Date::new("2026-05-24").unwrap();
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(json, "\"2026-05-24\"");
}

#[test]
fn date_round_trips_partial_precision() {
    let d = Date::new("1958").unwrap();
    let json = serde_json::to_string(&d).unwrap();
    let back: Date = serde_json::from_str(&json).unwrap();
    assert_eq!(d, back);
}

#[test]
fn date_deserialize_rejects_invalid() {
    let r: Result<Date, _> = serde_json::from_str("\"2026-13-99\"");
    assert!(r.is_err());
}
