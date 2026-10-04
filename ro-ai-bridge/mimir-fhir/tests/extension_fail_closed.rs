//! An Extension whose value[x] type is not modelled is refused, never silently dropped
//! (fail closed): reading it would otherwise lose the value and store the extension
//! without it. `id` (R5 Element.id) is kept.

use mimir_fhir::datatypes::Extension;
use mimir_fhir::resources::{ExternalOrganization, Organization, Patient};
use serde_json::json;

#[test]
fn an_unmodelled_value_type_is_refused() {
    for (key, value) in [
        ("valueUri", json!("https://example.org/x")),
        (
            "valueCoding",
            json!({ "system": "https://example.org", "code": "a" }),
        ),
        ("valueDate", json!("2026-10-04")),
        ("valuePeriod", json!({ "start": "2026-10-04" })),
    ] {
        let mut e = json!({ "url": "https://example.org/ext" });
        e[key] = value;
        let err = serde_json::from_value::<Extension>(e).unwrap_err();
        assert!(err.to_string().contains(key), "{key}: {err}");
    }
}

#[test]
fn a_resource_carrying_one_is_refused_too() {
    let p = json!({ "resourceType": "Patient",
        "extension": [{ "url": "https://example.org/ext", "valueUri": "https://example.org/x" }] });
    assert!(serde_json::from_value::<Patient>(p).is_err());
    let o = json!({ "resourceType": "Organization", "name": "x",
        "extension": [{ "url": "https://example.org/ext", "valueCoding": { "code": "a" } }] });
    assert!(serde_json::from_value::<Organization>(o.clone()).is_err());
    assert!(
        serde_json::from_value::<ExternalOrganization>(o).is_err(),
        "even on lenient ingest"
    );
}

#[test]
fn every_modelled_value_type_and_id_round_trip() {
    for (key, value) in [
        ("valueString", json!("a")),
        ("valueMarkdown", json!("**a**")),
        ("valueCode", json!("a")),
        ("valueBoolean", json!(true)),
        ("valueDateTime", json!("2026-10-04T10:00:00+07:00")),
        ("valueDecimal", json!(1.5)),
        ("valueInteger", json!(3)),
        ("valueQuantity", json!({ "value": 1, "unit": "/h" })),
        ("valueCodeableConcept", json!({ "text": "a" })),
        ("valueReference", json!({ "reference": "Patient/p" })),
    ] {
        let mut e = json!({ "id": "e1", "url": "https://example.org/ext" });
        e[key] = value;
        let back: Extension =
            serde_json::from_value(e.clone()).unwrap_or_else(|err| panic!("{key}: {err}"));
        assert_eq!(serde_json::to_value(&back).unwrap(), e, "{key}");
    }
}
