//! FHIR JSON `decimal` is a JSON **number** whose written precision matters
//! (`12.30` ≠ `12.3` as text). Out: the exact decimal as a number. In: a number —
//! from text or from a `serde_json::Value`, exponent form included — and, leniently,
//! the old quoted-string form.

use std::str::FromStr;

use mimir_fhir::datatypes::{Decimal, Quantity};
use mimir_fhir::resources::Observation;
use serde_json::{json, Value};

fn quantity(text: &str) -> Quantity {
    Quantity {
        value: Some(Decimal::from_str(text).unwrap()),
        ..Quantity::default()
    }
}

#[test]
fn serialises_as_a_json_number_with_its_exact_digits() {
    let text = serde_json::to_string(&quantity("12.30")).unwrap();
    assert_eq!(text, r#"{"value":12.30}"#);
    let v = serde_json::to_value(quantity("-0.005")).unwrap();
    assert!(v["value"].is_number(), "{v}");
    assert_eq!(v["value"].as_f64(), Some(-0.005));
}

#[test]
fn reads_numbers_from_text_and_from_values() {
    let from_text: Quantity = serde_json::from_str(r#"{"value":12.30}"#).unwrap();
    assert_eq!(
        from_text.value.unwrap().to_string(),
        "12.30",
        "text keeps precision"
    );
    let from_value: Quantity = serde_json::from_value(json!({ "value": 12.3 })).unwrap();
    assert_eq!(
        from_value.value.unwrap(),
        Decimal::from_str("12.3").unwrap()
    );
    let int: Quantity = serde_json::from_value(json!({ "value": 155 })).unwrap();
    assert_eq!(int.value.unwrap(), Decimal::from_str("155").unwrap());
    let exp: Quantity = serde_json::from_str(r#"{"value":1.5e2}"#).unwrap();
    assert_eq!(exp.value.unwrap(), Decimal::from_str("150").unwrap());
}

#[test]
fn accepts_the_old_string_form_leniently_but_writes_a_number() {
    let q: Quantity = serde_json::from_str(r#"{"value":"70.0"}"#).unwrap();
    assert_eq!(serde_json::to_string(&q).unwrap(), r#"{"value":70.0}"#);
}

#[test]
fn rejects_what_is_not_a_decimal() {
    for bad in [
        json!({ "value": true }),
        json!({ "value": "12,3" }),
        json!({ "value": [1] }),
    ] {
        assert!(
            serde_json::from_value::<Quantity>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn an_engine_observation_round_trips_with_numeric_values() {
    let engine = json!({
        "resourceType": "Observation",
        "id": "s1-ahi",
        "status": "final",
        "code": { "text": "Apnea-Hypopnea Index" },
        "subject": { "reference": "Patient/s1" },
        "valueQuantity": { "value": 12.3, "unit": "/h", "system": "http://unitsofmeasure.org" }
    });
    let o: Observation = serde_json::from_value(engine).unwrap();
    let back: Value = serde_json::to_value(&o).unwrap();
    assert_eq!(back["valueQuantity"]["value"], json!(12.3));
}
