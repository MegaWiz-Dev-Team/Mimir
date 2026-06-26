//! TDD tests for FHIR R5 `HumanName` datatype.

use mimir_fhir::datatypes::{HumanName, NameUse};

// --- Construction ---

#[test]
fn human_name_thai_helper_marks_official() {
    let name = HumanName::thai("บุญส่ง", "กิติชัย");
    assert_eq!(name.use_, Some(NameUse::Official));
    assert_eq!(name.family.as_deref(), Some("บุญส่ง"));
    assert_eq!(name.given, vec!["กิติชัย".to_string()]);
    assert_eq!(name.language.as_ref().unwrap().as_str(), "th");
}

#[test]
fn human_name_english_helper_marks_usual() {
    let name = HumanName::english("Boonsong", "Kittichai");
    assert_eq!(name.use_, Some(NameUse::Usual));
    assert_eq!(name.family.as_deref(), Some("Boonsong"));
    assert_eq!(name.given, vec!["Kittichai".to_string()]);
    assert_eq!(name.language.as_ref().unwrap().as_str(), "en");
}

#[test]
fn human_name_with_prefix_pushes_prefix() {
    let name = HumanName::thai("บุญส่ง", "กิติชัย")
        .with_prefix("นาย")
        .with_prefix("นพ.");
    assert_eq!(name.prefix, vec!["นาย".to_string(), "นพ.".to_string()]);
}

#[test]
fn human_name_with_given_appends_middle_names() {
    let name = HumanName::english("Smith", "John")
        .with_given("Robert")
        .with_given("William");
    assert_eq!(
        name.given,
        vec![
            "John".to_string(),
            "Robert".to_string(),
            "William".to_string(),
        ]
    );
}

#[test]
fn human_name_with_use_overrides_default() {
    let maiden = HumanName::english("Brown", "Mary").with_use(NameUse::Maiden);
    assert_eq!(maiden.use_, Some(NameUse::Maiden));
}

// --- Bilingual pair pattern (the canonical Thai patient name shape) ---

#[test]
fn bilingual_thai_latin_pair_round_trips() {
    let thai = HumanName::thai("บุญส่ง", "กิติชัย");
    let latin = HumanName::english("Boonsong", "Kittichai");
    let names = vec![thai.clone(), latin.clone()];

    let json = serde_json::to_string(&names).unwrap();
    let restored: Vec<HumanName> = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0], thai);
    assert_eq!(restored[1], latin);
    // Caller can filter by language to render the right script
    assert!(restored
        .iter()
        .any(|n| n.language.as_ref().unwrap().as_str() == "th"));
    assert!(restored
        .iter()
        .any(|n| n.language.as_ref().unwrap().as_str() == "en"));
}

// --- NameUse enum serde ---

#[test]
fn name_use_serializes_lowercase() {
    let cases = [
        (NameUse::Usual, "usual"),
        (NameUse::Official, "official"),
        (NameUse::Temp, "temp"),
        (NameUse::Nickname, "nickname"),
        (NameUse::Anonymous, "anonymous"),
        (NameUse::Old, "old"),
        (NameUse::Maiden, "maiden"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

// --- Serde — wire format ---

#[test]
fn human_name_use_renamed_to_use_on_wire() {
    let name = HumanName::thai("บุญส่ง", "กิติชัย");
    let json = serde_json::to_value(&name).unwrap();
    assert_eq!(json["use"], "official");
    assert!(json.get("use_").is_none());
}

#[test]
fn human_name_omits_empty_vecs() {
    let name = HumanName::thai("บุญส่ง", "กิติชัย");
    let json = serde_json::to_value(&name).unwrap();
    // No prefix / suffix used; arrays must be omitted, not "[]"
    assert!(json.get("prefix").is_none());
    assert!(json.get("suffix").is_none());
    // `given` has one entry — must be present
    assert!(json["given"].is_array());
}

#[test]
fn human_name_round_trips_with_prefix_and_suffix() {
    let mut name = HumanName::english("Smith", "John")
        .with_prefix("Dr.")
        .with_use(NameUse::Official);
    name.suffix.push("MD".to_string());

    let json = serde_json::to_string(&name).unwrap();
    let restored: HumanName = serde_json::from_str(&json).unwrap();
    assert_eq!(name, restored);
}

#[test]
fn human_name_text_field_for_unparsed_full_name() {
    // Use case: legacy systems give full name as one string, no parts breakdown
    let name = HumanName {
        text: Some("Somchai Boonsong".to_string()),
        ..HumanName::default()
    };
    let json = serde_json::to_value(&name).unwrap();
    assert_eq!(json["text"], "Somchai Boonsong");
    assert!(json.get("family").is_none());
    assert!(json.get("given").is_none());
}
