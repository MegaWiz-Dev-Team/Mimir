//! Sprint 1 Day 9 — Thai bilingual i18n end-to-end integration test.
//!
//! Verifies the full ADR-006 D5 i18n pattern by constructing a synthetic
//! Thai patient-shaped payload using mimir-fhir datatypes and round-tripping
//! it through JSON. Asserts:
//!
//! 1. Bilingual `HumanName` (Thai + Latin) preserves both entries with
//!    correct `language` tags.
//! 2. Thai-script content inside `Markdown` / `Annotation` / `Address` survives
//!    UTF-8 serialisation cleanly.
//! 3. Thai address 4-level mapping (line / district / state / postalCode +
//!    sub-district extension) round-trips.
//! 4. `Identifier` with Thai citizen ID system preserves the 13-digit value.
//! 5. `ContactPoint` with Thai +66 phone number serialises cleanly.
//!
//! This is NOT a resource-level test — Sprint 2+ adds Patient and the
//! other 20 resources. Day 9 only validates the datatype layer holds
//! together under i18n stress.

use std::str::FromStr;

use mimir_fhir::datatypes::{
    Address, Annotation, Code, CodeableConcept, Coding, ContactPoint, ContactPointSystem,
    ContactPointUse, DateTime, Decimal, HumanName, Identifier, Markdown, NameUse, Quantity,
    Reference, Uri,
};

/// Minimal struct holding the datatype-layer fields a Patient-shaped
/// resource would carry. Will be replaced by the real `Patient` resource
/// in Sprint 2. Defined locally here to avoid premature commitment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct PatientLikePayload {
    identifier: Vec<Identifier>,
    name: Vec<HumanName>,
    telecom: Vec<ContactPoint>,
    address: Vec<Address>,
    notes: Vec<Annotation>,
    vital_signs: Vec<Quantity>,
}

fn build_bilingual_payload() -> PatientLikePayload {
    PatientLikePayload {
        identifier: vec![Identifier::new(
            Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
            "1101700123456",
        )
        .official()
        .with_type(CodeableConcept::from_coding(
            Coding::new(
                Uri::new("http://terminology.hl7.org/CodeSystem/v2-0203").unwrap(),
                Code::new("NI").unwrap(),
            )
            .with_display("National identifier"),
        ))],
        name: vec![
            HumanName::thai("บุญส่ง", "กิติชัย")
                .with_prefix("นาย")
                .with_use(NameUse::Official),
            HumanName::english("Boonsong", "Kittichai")
                .with_prefix("Mr.")
                .with_use(NameUse::Usual),
        ],
        telecom: vec![
            ContactPoint::phone("+66-2-123-4567")
                .with_use(ContactPointUse::Home)
                .with_rank(1),
            ContactPoint::phone("+66-81-234-5678").with_use(ContactPointUse::Mobile),
        ],
        address: vec![Address::thai(
            "99/1 ถ.สาทร",
            "สีลม",          // ตำบล/แขวง → extension
            "บางรัก",        // เขต/อำเภอ → district
            "กรุงเทพมหานคร", // จังหวัด → state
            "10500",
        )],
        notes: vec![
            Annotation::by_string(
                "นพ.สมชาย",
                Markdown::new("**สรุป:** ผู้ป่วยอาการคงที่ ไม่มีไข้").unwrap(),
            )
            .with_time(DateTime::new("2026-05-25T08:30:00+07:00").unwrap()),
            Annotation::by_reference(
                Reference::literal("Practitioner/dr-somchai").with_display("นพ.สมชาย ใจดี"),
                Markdown::new("Reviewed and signed").unwrap(),
            ),
        ],
        vital_signs: vec![
            Quantity::ucum(
                Decimal::from_str("155").unwrap(),
                "mmHg",
                Code::new("mm[Hg]").unwrap(),
            ),
            Quantity::ucum(
                Decimal::from_str("36.5").unwrap(),
                "°C",
                Code::new("Cel").unwrap(),
            ),
        ],
    }
}

#[test]
fn bilingual_payload_round_trips_through_json() {
    let original = build_bilingual_payload();
    let json = serde_json::to_string(&original).unwrap();
    let restored: PatientLikePayload = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn thai_citizen_id_preserved_intact() {
    let payload = build_bilingual_payload();
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(
        json["identifier"][0]["system"],
        "https://fhir.moph.go.th/identifier/citizen-id"
    );
    assert_eq!(json["identifier"][0]["value"], "1101700123456");
    assert_eq!(json["identifier"][0]["use"], "official");
}

#[test]
fn thai_and_latin_names_both_present_with_language_tags() {
    let payload = build_bilingual_payload();
    let names: &Vec<HumanName> = &payload.name;
    assert_eq!(names.len(), 2);

    let thai = names
        .iter()
        .find(|n| n.language.as_ref().is_some_and(|c| c.as_str() == "th"));
    let latin = names
        .iter()
        .find(|n| n.language.as_ref().is_some_and(|c| c.as_str() == "en"));
    assert!(thai.is_some(), "Thai name entry must be present");
    assert!(latin.is_some(), "Latin name entry must be present");

    assert_eq!(thai.unwrap().family.as_deref(), Some("บุญส่ง"));
    assert_eq!(latin.unwrap().family.as_deref(), Some("Boonsong"));
}

#[test]
fn thai_script_in_markdown_annotation_survives_utf8() {
    let payload = build_bilingual_payload();
    let json = serde_json::to_string(&payload).unwrap();

    // Thai script must appear in the serialised JSON (not escaped to
    // \u sequences — serde_json default emits UTF-8)
    assert!(json.contains("**สรุป:**"));
    assert!(json.contains("ผู้ป่วยอาการคงที่"));
    assert!(json.contains("นพ.สมชาย"));
}

#[test]
fn thai_address_sub_district_extension_round_trips() {
    let payload = build_bilingual_payload();
    let address = &payload.address[0];

    assert_eq!(address.sub_district(), Some("สีลม"));
    assert_eq!(address.district.as_deref(), Some("บางรัก"));
    assert_eq!(address.state.as_deref(), Some("กรุงเทพมหานคร"));
    assert_eq!(address.postal_code.as_deref(), Some("10500"));
    assert_eq!(address.country.as_ref().unwrap().as_str(), "TH");

    // Round-trip via JSON and re-verify sub-district survives
    let json = serde_json::to_string(address).unwrap();
    let restored: Address = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.sub_district(), Some("สีลม"));
}

#[test]
fn thai_phone_with_plus_66_country_code_serialises() {
    let payload = build_bilingual_payload();
    let json = serde_json::to_value(&payload).unwrap();
    let phones: Vec<&str> = payload
        .telecom
        .iter()
        .filter(|c| c.system == Some(ContactPointSystem::Phone))
        .filter_map(|c| c.value.as_deref())
        .collect();
    assert_eq!(phones.len(), 2);
    assert!(phones.iter().any(|p| p.starts_with("+66")));
    // Check JSON emission keeps the + and -
    assert_eq!(json["telecom"][0]["value"], "+66-2-123-4567");
}

#[test]
fn annotation_authored_by_thai_practitioner_reference_with_thai_display() {
    let payload = build_bilingual_payload();
    // The second note is by reference with a Thai display name
    let note = &payload.notes[1];
    let author = note.author_reference.as_ref().unwrap();
    assert_eq!(author.reference.as_deref(), Some("Practitioner/dr-somchai"));
    assert_eq!(author.display.as_deref(), Some("นพ.สมชาย ใจดี"));
}

#[test]
fn vital_sign_with_unicode_celsius_unit_serialises() {
    // Body temp Quantity uses °C in the unit field — Unicode degree sign
    let payload = build_bilingual_payload();
    let body_temp = &payload.vital_signs[1];
    assert_eq!(body_temp.unit.as_deref(), Some("°C"));

    let json = serde_json::to_string(body_temp).unwrap();
    assert!(json.contains("°C"));

    let restored: Quantity = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, *body_temp);
}
