//! TDD tests for `Organization.contact` (R5 `ExtendedContactDetail`) and
//! `Organization.extension`: a site's letterhead (address, phone) and its own
//! settings as profile extensions, with R5 wire names. R5 moved address and
//! telecom into `contact`, so the R4 top-level fields stay unknown.

use mimir_fhir::datatypes::{Address, Code, ContactPoint, ExtendedContactDetail, Extension, Uri};
use mimir_fhir::resources::{ExternalOrganization, Organization};
use serde_json::json;

const LANG: &str = "https://megawiz.co.th/fhir/StructureDefinition/nott-site-report-language";

fn site() -> Organization {
    Organization {
        contact: vec![ExtendedContactDetail {
            address: Some(Address {
                text: Some("123 ถนนพระราม 6\nกรุงเทพฯ 10400".into()),
                ..Address::default()
            }),
            telecom: vec![ContactPoint::phone("02-000-0000")],
            ..ExtendedContactDetail::default()
        }],
        extension: vec![Extension::code(
            Uri::new(LANG).unwrap(),
            Code::new("th").unwrap(),
        )],
        ..Organization::named("โรงพยาบาลตัวอย่าง")
    }
}

#[test]
fn organization_contact_and_extensions_use_r5_wire_names() {
    let v = serde_json::to_value(site()).unwrap();
    assert_eq!(
        v,
        json!({
            "resourceType": "Organization",
            "active": true,
            "name": "โรงพยาบาลตัวอย่าง",
            "contact": [{
                "address": { "text": "123 ถนนพระราม 6\nกรุงเทพฯ 10400" },
                "telecom": [{ "system": "phone", "value": "02-000-0000" }]
            }],
            "extension": [{ "url": LANG, "valueCode": "th" }]
        })
    );
    let back: Organization = serde_json::from_value(v).unwrap();
    assert_eq!(back, site());
}

#[test]
fn every_extended_contact_detail_field_round_trips() {
    let v = json!({
        "resourceType": "Organization",
        "name": "x",
        "contact": [{
            "purpose": { "text": "billing" },
            "name": [{ "text": "ฝ่ายบัญชี" }],
            "telecom": [{ "system": "email", "value": "a@example.org" }],
            "address": { "text": "a" },
            "organization": { "reference": "Organization/parent" },
            "period": { "start": "2026-10-04" }
        }]
    });
    let o: Organization = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(&o).unwrap(), v);
}

#[test]
fn an_empty_contact_and_extension_list_is_not_written() {
    let v = serde_json::to_value(Organization::named("x")).unwrap();
    assert!(
        v.get("contact").is_none() && v.get("extension").is_none(),
        "{v}"
    );
}

#[test]
fn r4_top_level_address_and_telecom_stay_unknown() {
    for field in ["address", "telecom"] {
        let mut v = json!({ "resourceType": "Organization", "name": "x" });
        v[field] = json!([{ "text": "a" }]);
        let err = serde_json::from_value::<Organization>(v).unwrap_err();
        assert!(err.to_string().contains(field), "{field}: {err}");
    }
}

#[test]
fn lenient_ingest_keeps_contact_and_extensions() {
    let e: ExternalOrganization =
        serde_json::from_value(serde_json::to_value(site()).unwrap()).unwrap();
    assert_eq!(Organization::from(e), site());
}
