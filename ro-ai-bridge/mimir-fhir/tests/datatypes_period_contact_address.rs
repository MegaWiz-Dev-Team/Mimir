//! TDD tests for Day 5 datatypes — `Period`, `ContactPoint`, `Extension`, `Address`.

use mimir_fhir::datatypes::{
    Address, AddressType, AddressUse, Code, ContactPoint, ContactPointSystem, ContactPointUse,
    DateTime, Extension, Identifier, Period, Uri, TH_SUB_DISTRICT_EXTENSION_URL,
};

// =============================================================================
// Period
// =============================================================================

#[test]
fn period_starting_leaves_end_open() {
    let p = Period::starting(DateTime::new("2024-01-01").unwrap());
    assert_eq!(p.start.as_ref().unwrap().as_str(), "2024-01-01");
    assert!(p.end.is_none());
}

#[test]
fn period_between_sets_both_bounds() {
    let p = Period::between(
        DateTime::new("2024-01-01").unwrap(),
        DateTime::new("2025-12-31").unwrap(),
    );
    assert!(p.start.is_some());
    assert!(p.end.is_some());
}

#[test]
fn period_omits_none_fields_on_wire() {
    let p = Period::starting(DateTime::new("2024-01-01").unwrap());
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["start"], "2024-01-01");
    assert!(json.get("end").is_none(), "open-ended period must omit end");
}

#[test]
fn period_round_trips() {
    let original = Period::between(
        DateTime::new("2026-05-24").unwrap(),
        DateTime::new("2027-05-24T00:00:00Z").unwrap(),
    );
    let json = serde_json::to_string(&original).unwrap();
    let restored: Period = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn period_validates_inner_datetime_grammar() {
    let json = r#"{"start":"not-a-date"}"#;
    let result: Result<Period, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

// =============================================================================
// Period closes Identifier TODO
// =============================================================================

#[test]
fn identifier_period_field_round_trips() {
    let id = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1101700123456",
    )
    .official()
    .with_period(Period::between(
        DateTime::new("2020-01-01").unwrap(),
        DateTime::new("2030-12-31").unwrap(),
    ));

    let json = serde_json::to_string(&id).unwrap();
    let restored: Identifier = serde_json::from_str(&json).unwrap();
    assert_eq!(id, restored);
    assert!(restored.period.is_some());
}

// =============================================================================
// ContactPoint
// =============================================================================

#[test]
fn contact_point_phone_factory() {
    let cp = ContactPoint::phone("+66-2-123-4567");
    assert_eq!(cp.system, Some(ContactPointSystem::Phone));
    assert_eq!(cp.value.as_deref(), Some("+66-2-123-4567"));
    assert!(cp.use_.is_none());
}

#[test]
fn contact_point_email_factory() {
    let cp = ContactPoint::email("patient@example.com");
    assert_eq!(cp.system, Some(ContactPointSystem::Email));
    assert_eq!(cp.value.as_deref(), Some("patient@example.com"));
}

#[test]
fn contact_point_with_use_and_rank() {
    let cp = ContactPoint::phone("+66-81-234-5678")
        .with_use(ContactPointUse::Mobile)
        .with_rank(1);
    assert_eq!(cp.use_, Some(ContactPointUse::Mobile));
    assert_eq!(cp.rank, Some(1));
}

#[test]
fn contact_point_system_serializes_lowercase() {
    let cases = [
        (ContactPointSystem::Phone, "phone"),
        (ContactPointSystem::Fax, "fax"),
        (ContactPointSystem::Email, "email"),
        (ContactPointSystem::Pager, "pager"),
        (ContactPointSystem::Url, "url"),
        (ContactPointSystem::Sms, "sms"),
        (ContactPointSystem::Other, "other"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn contact_point_renames_use_on_wire() {
    let cp = ContactPoint::phone("123").with_use(ContactPointUse::Home);
    let json = serde_json::to_value(&cp).unwrap();
    assert_eq!(json["use"], "home");
    assert!(json.get("use_").is_none());
}

#[test]
fn contact_point_round_trips_with_period() {
    let original = ContactPoint::email("old@example.com")
        .with_use(ContactPointUse::Old)
        .with_rank(3);
    let json = serde_json::to_string(&original).unwrap();
    let restored: ContactPoint = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Extension (minimal — valueString only for Day 5)
// =============================================================================

#[test]
fn extension_string_factory() {
    let ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/foo").unwrap(),
        "bar",
    );
    assert_eq!(ext.value_string.as_deref(), Some("bar"));
}

#[test]
fn extension_serializes_as_value_string_camel_case() {
    let ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/foo").unwrap(),
        "bar",
    );
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueString"], "bar");
    assert!(
        json.get("value_string").is_none(),
        "Rust field uses snake_case; wire format must use camelCase"
    );
}

#[test]
fn extension_round_trips() {
    let original = Extension::string(Uri::new(TH_SUB_DISTRICT_EXTENSION_URL).unwrap(), "สีลม");
    let json = serde_json::to_string(&original).unwrap();
    let restored: Extension = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Address — Thai 4-level convention
// =============================================================================

#[test]
fn address_thai_helper_populates_all_4_levels() {
    let addr = Address::thai(
        "123 ถ.สีลม",
        "สีลม",          // ตำบล/แขวง
        "บางรัก",        // เขต/อำเภอ
        "กรุงเทพมหานคร", // จังหวัด
        "10500",
    );

    assert_eq!(addr.use_, Some(AddressUse::Home));
    assert_eq!(addr.line, vec!["123 ถ.สีลม".to_string()]);
    assert_eq!(addr.district.as_deref(), Some("บางรัก"));
    assert_eq!(addr.state.as_deref(), Some("กรุงเทพมหานคร"));
    assert_eq!(addr.postal_code.as_deref(), Some("10500"));
    assert_eq!(addr.country.as_ref().unwrap().as_str(), "TH");
    assert_eq!(addr.sub_district(), Some("สีลม"));
}

#[test]
fn address_sub_district_extracted_from_extension() {
    let addr = Address::thai("123", "ตำบลทดสอบ", "อำเภอทดสอบ", "ทดสอบ", "12345");
    assert_eq!(addr.sub_district(), Some("ตำบลทดสอบ"));
    // Should also appear in extension[]
    assert_eq!(addr.extension.len(), 1);
    assert_eq!(
        addr.extension[0].url.as_str(),
        TH_SUB_DISTRICT_EXTENSION_URL
    );
}

#[test]
fn address_round_trips_with_thai_extension() {
    let original = Address::thai("99/1 ถ.สาทร", "สีลม", "บางรัก", "กรุงเทพมหานคร", "10500");
    let json = serde_json::to_string(&original).unwrap();
    let restored: Address = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
    assert_eq!(restored.sub_district(), Some("สีลม"));
}

#[test]
fn address_postal_code_renames_on_wire() {
    let addr = Address::thai("X", "Y", "Z", "Q", "12345");
    let json = serde_json::to_value(&addr).unwrap();
    assert_eq!(json["postalCode"], "12345");
    assert!(json.get("postal_code").is_none());
}

#[test]
fn address_use_renames_on_wire() {
    let addr = Address {
        use_: Some(AddressUse::Work),
        ..Address::default()
    };
    let json = serde_json::to_value(&addr).unwrap();
    assert_eq!(json["use"], "work");
    assert!(json.get("use_").is_none());
}

#[test]
fn address_with_line_appends_more_lines() {
    let addr = Address::thai("Line 1", "X", "Y", "Z", "12345").with_line("Apt 4B");
    assert_eq!(addr.line.len(), 2);
    assert_eq!(addr.line[1], "Apt 4B");
}

#[test]
fn address_type_and_use_serialize_lowercase() {
    let cases_type = [
        (AddressType::Postal, "postal"),
        (AddressType::Physical, "physical"),
        (AddressType::Both, "both"),
    ];
    for (variant, expected) in cases_type {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }

    let cases_use = [
        (AddressUse::Home, "home"),
        (AddressUse::Work, "work"),
        (AddressUse::Temp, "temp"),
        (AddressUse::Old, "old"),
        (AddressUse::Billing, "billing"),
    ];
    for (variant, expected) in cases_use {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn address_empty_extension_omitted_on_wire() {
    let addr = Address {
        country: Some(Code::new("US").unwrap()),
        ..Address::default()
    };
    let json = serde_json::to_value(&addr).unwrap();
    assert!(json.get("extension").is_none());
}
