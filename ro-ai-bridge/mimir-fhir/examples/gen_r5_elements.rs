//! Writes `src/validators/r5_elements.rs`: every element of the resources mimir-fhir
//! stores and of the complex datatypes they use, with its types, cardinality, allowed
//! reference targets and content reference, taken from the FHIR R5 definitions.
//!
//! ```text
//! curl -LO https://hl7.org/fhir/R5/definitions.json.zip && unzip definitions.json.zip -d defs
//! cargo run --example gen_r5_elements -- defs src/validators/r5_elements.rs <sha256 of the zip>
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value;

/// The resources mimir-fhir has a type for.
const RESOURCES: [&str; 15] = [
    "AuditEvent",
    "Binary",
    "Consent",
    "Device",
    "DiagnosticReport",
    "DocumentReference",
    "Encounter",
    "Observation",
    "Organization",
    "Patient",
    "Practitioner",
    "PractitionerRole",
    "Provenance",
    "ServiceRequest",
    "Task",
];

struct Row {
    types: Vec<String>,
    min: u64,
    many: bool,
    targets: Vec<String>,
    content_ref: Option<String>,
}

fn load(path: &str) -> BTreeMap<String, Value> {
    let bundle: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect(path)).expect(path);
    bundle["entry"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| &e["resource"])
        .filter(|r| r["resourceType"] == "StructureDefinition")
        .map(|r| (r["id"].as_str().unwrap().to_owned(), r.clone()))
        .collect()
}

fn last(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (defs, out, sha) = (&args[1], &args[2], args.get(3).cloned().unwrap_or_default());
    let resources = load(&format!("{defs}/profiles-resources.json"));
    let types = load(&format!("{defs}/profiles-types.json"));
    let version = std::fs::read_to_string(format!("{defs}/version.info")).unwrap_or_default();
    let field = |k: &str| {
        version
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .unwrap_or("?")
            .to_owned()
    };
    let complex: BTreeSet<String> = types
        .values()
        .filter(|sd| sd["kind"] == "complex-type" && sd["abstract"] != true)
        .map(|sd| sd["id"].as_str().unwrap().to_owned())
        .collect();
    let (rows, done) = collect(&resources, &types, &complex);
    let used: Vec<String> = done
        .iter()
        .filter(|n| complex.contains(*n))
        .cloned()
        .collect();
    let source = format!(
        "FHIR {} build {}, zip sha256\n//! {sha}",
        field("FhirVersion"),
        field("buildId")
    );
    std::fs::write(out, render(&rows, &used, &field("FhirVersion"), &source)).expect(out);
    eprintln!(
        "{} elements from {} definitions into {out}",
        rows.len(),
        done.len()
    );
}

/// Every element of the resources and, transitively, of the complex types they use.
fn collect(
    resources: &BTreeMap<String, Value>,
    types: &BTreeMap<String, Value>,
    complex: &BTreeSet<String>,
) -> (BTreeMap<String, Row>, BTreeSet<String>) {
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut wanted: Vec<String> = RESOURCES.iter().map(ToString::to_string).collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    while let Some(name) = wanted.pop() {
        if !done.insert(name.clone()) {
            continue;
        }
        let sd = resources
            .get(&name)
            .or_else(|| types.get(&name))
            .expect(&name);
        for el in sd["snapshot"]["element"].as_array().unwrap() {
            let path = el["path"].as_str().unwrap();
            if !path.contains('.') {
                continue;
            }
            let mut tys = Vec::new();
            let mut targets = Vec::new();
            for t in el["type"].as_array().into_iter().flatten() {
                let code = t["code"].as_str().unwrap_or_default();
                // FHIRPath system types (id, url of extensions) are primitives.
                let code = if code.contains('/') {
                    "string".to_owned()
                } else {
                    code.to_owned()
                };
                if complex.contains(&code) && !done.contains(&code) {
                    wanted.push(code.clone());
                }
                if code == "Reference" || code == "CodeableReference" {
                    targets.extend(
                        t["targetProfile"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(last),
                    );
                }
                tys.push(code);
            }
            let content_ref = el["contentReference"]
                .as_str()
                .map(|c| c.rsplit('#').next().unwrap_or(c).to_owned());
            rows.insert(
                path.to_owned(),
                Row {
                    types: tys,
                    min: el["min"].as_u64().unwrap_or(0),
                    many: el["max"].as_str() != Some("1") && el["max"].as_str() != Some("0"),
                    targets,
                    content_ref,
                },
            );
        }
    }
    (rows, done)
}

/// The Rust source of `r5_elements.rs`.
fn render(rows: &BTreeMap<String, Row>, complex: &[String], version: &str, source: &str) -> String {
    let quote = |v: &[String]| {
        v.iter()
            .map(|s| format!("{s:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut s = String::new();
    writeln!(
        s,
        "//! Generated by `cargo run --example gen_r5_elements` from the FHIR R5 definitions"
    )
    .unwrap();
    writeln!(s, "//! (<https://hl7.org/fhir/R5/definitions.json.zip>, {source}). Do not edit by hand: regenerate.").unwrap();
    writeln!(s).unwrap();
    writeln!(s, "use super::r5::Element;").unwrap();
    writeln!(s).unwrap();
    writeln!(s, "/// FHIR version these elements come from.").unwrap();
    writeln!(s, "pub(crate) const FHIR_VERSION: &str = {version:?};").unwrap();
    writeln!(s).unwrap();
    writeln!(
        s,
        "/// Complex datatypes (an element of one of these types is walked into)."
    )
    .unwrap();
    writeln!(
        s,
        "pub(crate) static COMPLEX_TYPES: &[&str] = &[{}];",
        quote(complex)
    )
    .unwrap();
    writeln!(s).unwrap();
    writeln!(s, "/// Every element, sorted by path.").unwrap();
    writeln!(s, "pub(crate) static ELEMENTS: &[Element] = &[").unwrap();
    for (path, r) in rows {
        writeln!(
            s,
            "    Element {{ path: {path:?}, types: &[{}], min: {}, many: {}, targets: &[{}], content_ref: {} }},",
            quote(&r.types),
            r.min,
            r.many,
            quote(&r.targets),
            r.content_ref.as_ref().map_or("None".to_owned(), |c| format!("Some({c:?})")),
        )
        .unwrap();
    }
    writeln!(s, "];").unwrap();
    s
}
