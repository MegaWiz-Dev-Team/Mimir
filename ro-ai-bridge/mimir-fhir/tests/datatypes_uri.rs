//! TDD tests for FHIR R5 `Uri` and `Url` primitives.

use mimir_fhir::datatypes::{Uri, UriError, Url};

// --- Uri happy path ---

#[test]
fn uri_accepts_http_url() {
    assert!(Uri::new("http://hl7.org/fhir/R5/Patient").is_ok());
    assert!(Uri::new("https://fhir.moph.go.th/StructureDefinition/Patient").is_ok());
}

#[test]
fn uri_accepts_urn_oid() {
    assert!(Uri::new("urn:oid:2.16.840.1.113883.6.1").is_ok()); // LOINC OID
}

#[test]
fn uri_accepts_urn_uuid() {
    assert!(Uri::new("urn:uuid:01J5XQNK7VFW9RZGBQ2N3PMABC").is_ok());
}

#[test]
fn uri_accepts_relative_reference() {
    assert!(Uri::new("Patient/A12345").is_ok());
    assert!(Uri::new("#contained-1").is_ok());
}

#[test]
fn uri_accepts_thai_canonical_url() {
    // Thai FHIR Thailand IG profile URL — non-ASCII path components allowed in URI
    assert!(Uri::new("https://fhir.moph.go.th/StructureDefinition/TH-Patient").is_ok());
}

// --- Uri rejects ---

#[test]
fn uri_rejects_empty() {
    assert_eq!(Uri::new(""), Err(UriError::Empty));
}

#[test]
fn uri_rejects_space() {
    assert_eq!(
        Uri::new("http://example.com/with space"),
        Err(UriError::ContainsWhitespace(' '))
    );
}

#[test]
fn uri_rejects_tab() {
    assert_eq!(
        Uri::new("http://example.com\ttab"),
        Err(UriError::ContainsWhitespace('\t'))
    );
}

#[test]
fn uri_rejects_newline() {
    assert_eq!(
        Uri::new("http://example.com\nnew"),
        Err(UriError::ContainsWhitespace('\n'))
    );
}

// --- Url shares behavior with Uri ---

#[test]
fn url_accepts_http_url() {
    assert!(Url::new("http://fhir.example.com/r5").is_ok());
}

#[test]
fn url_rejects_empty() {
    assert_eq!(Url::new(""), Err(UriError::Empty));
}

#[test]
fn url_rejects_whitespace() {
    assert!(Url::new("http://example.com/has space").is_err());
}

// --- Serde round-trip ---

#[test]
fn uri_serializes_as_json_string() {
    let u = Uri::new("http://hl7.org/fhir").unwrap();
    assert_eq!(
        serde_json::to_string(&u).unwrap(),
        "\"http://hl7.org/fhir\""
    );
}

#[test]
fn uri_deserialize_rejects_whitespace() {
    let result: Result<Uri, _> = serde_json::from_str("\"http://has space.com\"");
    assert!(result.is_err());
}

#[test]
fn uri_round_trips_loinc_system() {
    // FHIR Identifier.system commonly references LOINC code system canonical URL
    let original = Uri::new("http://loinc.org").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Uri = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
