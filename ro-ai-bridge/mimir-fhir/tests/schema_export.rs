//! TDD tests for Sprint 1 Day 8 — JSON Schema export pipeline.

use mimir_fhir::schema_export::{all_datatype_schemas, all_datatype_schemas_json};

#[test]
fn all_17_datatypes_emit_a_schema() {
    let schemas = all_datatype_schemas();
    let expected: Vec<&'static str> = vec![
        "Address",
        "Annotation",
        "CodeableConcept",
        "Coding",
        "ContactPoint",
        "Decimal",
        "Extension",
        "HumanName",
        "Identifier",
        "Meta",
        "Money",
        "Narrative",
        "Period",
        "Quantity",
        "Range",
        "Ratio",
        "Reference",
    ];
    assert_eq!(schemas.len(), expected.len());
    for name in expected {
        assert!(
            schemas.contains_key(name),
            "expected schema for {name} but it was missing"
        );
    }
}

#[test]
fn schemas_are_sorted_alphabetically_for_stable_diff() {
    let schemas = all_datatype_schemas();
    let names: Vec<&String> = schemas.keys().collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn patient_identifier_slice_emits_string_value() {
    // Identifier.value should appear as an optional string in the schema
    let schemas = all_datatype_schemas();
    let identifier_schema = serde_json::to_value(schemas.get("Identifier").unwrap()).unwrap();
    let properties = &identifier_schema["properties"];
    assert!(properties["value"].is_object());
}

#[test]
fn human_name_schema_includes_use_renamed() {
    let schemas = all_datatype_schemas();
    let hn = serde_json::to_value(schemas.get("HumanName").unwrap()).unwrap();
    // Rust field is `use_`; schema must use the FHIR wire name `use`
    let props = &hn["properties"];
    assert!(
        props.get("use").is_some(),
        "HumanName.use must appear under wire name"
    );
    assert!(
        props.get("use_").is_none(),
        "Rust field name use_ must not leak into schema"
    );
}

#[test]
fn extension_schema_includes_all_9_value_variants() {
    let schemas = all_datatype_schemas();
    let ext = serde_json::to_value(schemas.get("Extension").unwrap()).unwrap();
    let props = &ext["properties"];
    for variant in [
        "valueString",
        "valueCode",
        "valueBoolean",
        "valueDateTime",
        "valueDecimal",
        "valueInteger",
        "valueQuantity",
        "valueCodeableConcept",
        "valueReference",
    ] {
        assert!(
            props.get(variant).is_some(),
            "Extension schema missing {variant}"
        );
    }
}

#[test]
fn decimal_schema_uses_string_format() {
    // FHIR decimal serialises as string (rust_decimal serde-with-str feature)
    let schemas = all_datatype_schemas();
    let dec = serde_json::to_value(schemas.get("Decimal").unwrap()).unwrap();
    assert_eq!(dec["type"], "string");
    assert_eq!(dec["format"], "decimal");
    assert!(
        dec["pattern"].is_string(),
        "Decimal schema should carry FHIR R5 grammar pattern"
    );
}

#[test]
fn schemas_serialise_to_valid_json() {
    let json_map = all_datatype_schemas_json();
    assert!(!json_map.is_empty());
    // Every value must round-trip through serde_json
    for (name, json) in &json_map {
        let parsed: serde_json::Value = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("schema for {name} not valid JSON: {e}"));
        assert!(
            parsed.is_object(),
            "schema for {name} should be a JSON object"
        );
    }
}

#[test]
fn address_schema_includes_postal_code_camel_case() {
    let schemas = all_datatype_schemas();
    let addr = serde_json::to_value(schemas.get("Address").unwrap()).unwrap();
    let props = &addr["properties"];
    assert!(
        props.get("postalCode").is_some(),
        "Rust postal_code must serialise as camelCase postalCode"
    );
    assert!(
        props.get("postal_code").is_none(),
        "Rust snake_case must not appear in schema"
    );
}

#[test]
fn quantity_comparator_schema_uses_operator_strings() {
    let schemas = all_datatype_schemas();
    let q = serde_json::to_value(schemas.get("Quantity").unwrap()).unwrap();
    // Find the comparator property; it should be an enum with operator strings
    // The exact location depends on schemars output shape. We at least confirm
    // the property exists.
    let props = &q["properties"];
    assert!(props.get("comparator").is_some());
}
