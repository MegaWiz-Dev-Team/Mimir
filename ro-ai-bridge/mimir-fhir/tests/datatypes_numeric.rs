//! TDD tests for Sprint 1 Day 6 numeric datatypes —
//! `Decimal`, `Quantity`, `QuantityComparator`, `Money`, `Range`, `Ratio`.

use std::str::FromStr;

use mimir_fhir::datatypes::{Code, Decimal, Money, Quantity, QuantityComparator, Range, Ratio};

// =============================================================================
// Decimal (re-export of rust_decimal::Decimal)
// =============================================================================

#[test]
fn decimal_preserves_trailing_zero() {
    // FHIR R5: precision is significant — "1.50" must round-trip as "1.50" not "1.5"
    let d = Decimal::from_str("1.50").unwrap();
    assert_eq!(d.to_string(), "1.50");
}

#[test]
fn decimal_parses_negative() {
    let d = Decimal::from_str("-12.345").unwrap();
    assert!(d.is_sign_negative());
}

#[test]
fn decimal_handles_clinical_precision() {
    // Typical clinical decimal values
    assert!(Decimal::from_str("65.4").is_ok()); // body weight kg
    assert!(Decimal::from_str("36.5").is_ok()); // body temp C
    assert!(Decimal::from_str("155").is_ok()); // SBP mmHg
    assert!(Decimal::from_str("0.005").is_ok()); // adrenaline 1:200000
    assert!(Decimal::from_str("3.14159265358979323846").is_ok()); // 20-digit precision
}

// =============================================================================
// Quantity
// =============================================================================

#[test]
fn quantity_ucum_factory_sets_full_metadata() {
    let q = Quantity::ucum(
        Decimal::from_str("155").unwrap(),
        "mmHg",
        Code::new("mm[Hg]").unwrap(),
    );
    assert_eq!(q.value.unwrap().to_string(), "155");
    assert_eq!(q.unit.as_deref(), Some("mmHg"));
    assert_eq!(q.system.as_ref().unwrap().as_str(), Quantity::UCUM_SYSTEM);
    assert_eq!(q.code.as_ref().unwrap().as_str(), "mm[Hg]");
    assert!(q.comparator.is_none());
}

#[test]
fn quantity_with_comparator_for_lab_below_detection_limit() {
    // "< 0.1" — common pattern for lab values below assay sensitivity
    let q = Quantity::ucum(
        Decimal::from_str("0.1").unwrap(),
        "ng/mL",
        Code::new("ng/mL").unwrap(),
    )
    .with_comparator(QuantityComparator::LessThan);
    assert_eq!(q.comparator, Some(QuantityComparator::LessThan));
}

#[test]
fn quantity_comparator_serializes_as_fhir_operators() {
    let cases = [
        (QuantityComparator::LessThan, "<"),
        (QuantityComparator::LessOrEqual, "<="),
        (QuantityComparator::GreaterOrEqual, ">="),
        (QuantityComparator::GreaterThan, ">"),
        (QuantityComparator::Ad, "ad"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn quantity_omits_none_fields() {
    let q = Quantity {
        value: Some(Decimal::from_str("70").unwrap()),
        unit: Some("kg".into()),
        ..Quantity::default()
    };
    let json = serde_json::to_value(&q).unwrap();
    assert_eq!(json["unit"], "kg");
    assert!(json.get("comparator").is_none());
    assert!(json.get("system").is_none());
    assert!(json.get("code").is_none());
}

#[test]
fn quantity_round_trips_ucum_blood_pressure() {
    let original = Quantity::ucum(
        Decimal::from_str("155").unwrap(),
        "mmHg",
        Code::new("mm[Hg]").unwrap(),
    );
    let json = serde_json::to_string(&original).unwrap();
    let restored: Quantity = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn quantity_deserialize_validates_inner_uri() {
    let json = r#"{"value":"1","system":"has space"}"#;
    let result: Result<Quantity, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Uri validation must propagate");
}

// =============================================================================
// Money
// =============================================================================

#[test]
fn money_thb_factory() {
    let m = Money::thb(Decimal::from_str("500000").unwrap());
    assert_eq!(m.value.unwrap().to_string(), "500000");
    assert_eq!(m.currency.as_ref().unwrap().as_str(), "THB");
}

#[test]
fn money_arbitrary_currency() {
    let usd = Code::new("USD").unwrap();
    let m = Money::new(Decimal::from_str("14000.00").unwrap(), usd.clone());
    assert_eq!(m.currency, Some(usd));
}

#[test]
fn money_round_trips() {
    let original = Money::thb(Decimal::from_str("1234567.89").unwrap());
    let json = serde_json::to_string(&original).unwrap();
    let restored: Money = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn money_serializes_as_expected_shape() {
    let m = Money::thb(Decimal::from_str("100").unwrap());
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(json["currency"], "THB");
    // Money has no extra fields
    assert!(json.get("system").is_none());
    assert!(json.get("unit").is_none());
}

// =============================================================================
// Range
// =============================================================================

#[test]
fn range_lab_reference_range_normal_glucose() {
    // Fasting glucose normal range: 70-100 mg/dL
    let low = Quantity::ucum(
        Decimal::from_str("70").unwrap(),
        "mg/dL",
        Code::new("mg/dL").unwrap(),
    );
    let high = Quantity::ucum(
        Decimal::from_str("100").unwrap(),
        "mg/dL",
        Code::new("mg/dL").unwrap(),
    );
    let r = Range::between(low.clone(), high.clone());
    assert_eq!(r.low, Some(low));
    assert_eq!(r.high, Some(high));
}

#[test]
fn range_at_least_open_ended_above() {
    let low = Quantity::ucum(
        Decimal::from_str("0").unwrap(),
        "mg/dL",
        Code::new("mg/dL").unwrap(),
    );
    let r = Range::at_least(low);
    assert!(r.low.is_some());
    assert!(r.high.is_none());
}

#[test]
fn range_at_most_open_ended_below() {
    let high = Quantity::ucum(
        Decimal::from_str("100").unwrap(),
        "mg/dL",
        Code::new("mg/dL").unwrap(),
    );
    let r = Range::at_most(high);
    assert!(r.low.is_none());
    assert!(r.high.is_some());
}

#[test]
fn range_omits_none_bounds() {
    let r = Range::at_least(Quantity::ucum(
        Decimal::from_str("0").unwrap(),
        "ng/mL",
        Code::new("ng/mL").unwrap(),
    ));
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("low").is_some());
    assert!(json.get("high").is_none());
}

#[test]
fn range_round_trips_through_json() {
    let original = Range::between(
        Quantity::ucum(
            Decimal::from_str("3.5").unwrap(),
            "mmol/L",
            Code::new("mmol/L").unwrap(),
        ),
        Quantity::ucum(
            Decimal::from_str("5.1").unwrap(),
            "mmol/L",
            Code::new("mmol/L").unwrap(),
        ),
    );
    let json = serde_json::to_string(&original).unwrap();
    let restored: Range = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Ratio
// =============================================================================

#[test]
fn ratio_drug_concentration() {
    // Adrenaline 1 mg in 1 mL — common ACLS resuscitation dose-rate
    let numerator = Quantity::ucum(
        Decimal::from_str("1").unwrap(),
        "mg",
        Code::new("mg").unwrap(),
    );
    let denominator = Quantity::ucum(
        Decimal::from_str("1").unwrap(),
        "mL",
        Code::new("mL").unwrap(),
    );
    let r = Ratio::new(numerator, denominator);
    assert!(r.numerator.is_some());
    assert!(r.denominator.is_some());
}

#[test]
fn ratio_antibody_titer_pattern() {
    // Antibody titer 1:64 — modeled as Quantity values with no unit
    let one = Quantity {
        value: Some(Decimal::from_str("1").unwrap()),
        ..Quantity::default()
    };
    let sixty_four = Quantity {
        value: Some(Decimal::from_str("64").unwrap()),
        ..Quantity::default()
    };
    let titer = Ratio::new(one, sixty_four);

    let json = serde_json::to_value(&titer).unwrap();
    assert_eq!(json["numerator"]["value"], "1");
    assert_eq!(json["denominator"]["value"], "64");
}

#[test]
fn ratio_round_trips() {
    let original = Ratio::new(
        Quantity::ucum(
            Decimal::from_str("5").unwrap(),
            "mg",
            Code::new("mg").unwrap(),
        ),
        Quantity::ucum(
            Decimal::from_str("100").unwrap(),
            "mL",
            Code::new("mL").unwrap(),
        ),
    );
    let json = serde_json::to_string(&original).unwrap();
    let restored: Ratio = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
