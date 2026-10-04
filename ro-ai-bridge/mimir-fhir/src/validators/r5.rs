//! FHIR R5 conformance of a resource before it is written. The store calls
//! [`validate`] on every create and update, and refuses the write when it finds an issue.
//!
//! What it checks, all from the FHIR R5 definitions (`r5_elements.rs` is generated
//! from them; the invariant keys are R5's own):
//! - every element with `min ≥ 1`, at any depth, is present (the types enforce most
//!   1..1 elements; this adds 1..* lists and elements inside backbones/datatypes);
//! - a literal reference (`Type/id`, `Type/id/_history/n`, or an absolute URL ending
//!   so) names a type the element allows (`Reference(Patient | Group)` …);
//! - the error-level invariants that need no `resolve()`: org-1, org-3, org-4,
//!   obs-3, obs-6, obs-7, obs-8, pat-1, dev-1, inv-1, tsk-1, and the datatype
//!   invariants ref-1, ref-2, att-1, qty-3, per-1, cpt-2, ext-1.
//!
//! Not checked here: invariants that resolve references (prov-1/2/3, obs-9, dgr-1),
//! value-set bindings beyond the typed status codes, and narrative XHTML — the HL7
//! validator covers those in the conformance tests.

use serde_json::{Map, Value};

use super::r5_elements::{COMPLEX_TYPES, ELEMENTS};

/// One element definition (generated).
#[derive(Debug)]
pub(crate) struct Element {
    pub path: &'static str,
    pub types: &'static [&'static str],
    pub min: u32,
    #[allow(dead_code)] // the typed model already refuses a list where one value goes
    pub many: bool,
    pub targets: &'static [&'static str],
    pub content_ref: Option<&'static str>,
}

/// One way a resource is not valid R5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Where, as a FHIRPath-like path with list indexes (`Provenance.agent[0].who`).
    pub path: String,
    /// The R5 invariant key (`org-1`), or `required` / `reference-target`.
    pub rule: String,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]: {}", self.path, self.rule, self.message)
    }
}

/// The FHIR version the rules come from.
#[must_use]
pub fn fhir_version() -> &'static str {
    super::r5_elements::FHIR_VERSION
}

/// Check `resource` (JSON of a `resource_type`) against R5; empty when valid.
#[must_use]
pub fn validate(resource_type: &str, resource: &Value) -> Vec<Issue> {
    let mut v = Validator {
        root: resource,
        issues: Vec::new(),
    };
    match resource.as_object() {
        Some(obj) => {
            v.object(obj, resource_type, resource_type);
            v.resource_rules(resource_type, obj);
        }
        None => v.issue(
            resource_type,
            "structure",
            "a resource is a JSON object".into(),
        ),
    }
    v.issues
}

fn element(path: &str) -> Option<&'static Element> {
    ELEMENTS
        .binary_search_by(|e| e.path.cmp(path))
        .ok()
        .map(|i| &ELEMENTS[i])
}

fn children(path: &str) -> Vec<&'static Element> {
    let prefix = format!("{path}.");
    let start = ELEMENTS.partition_point(|e| e.path < prefix.as_str());
    ELEMENTS[start..]
        .iter()
        .take_while(|e| e.path.starts_with(&prefix))
        .filter(|e| !e.path[prefix.len()..].contains('.'))
        .collect()
}

fn name_of(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

fn capitalised(t: &str) -> String {
    let mut c = t.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// The element `key` names under `ctx`, and the type its value has.
fn find(ctx: &str, key: &str) -> Option<(&'static Element, &'static str)> {
    if let Some(el) = element(&format!("{ctx}.{key}")) {
        return Some((el, el.types.first().copied().unwrap_or("")));
    }
    for el in children(ctx) {
        if let Some(base) = name_of(el.path).strip_suffix("[x]") {
            if let Some(rest) = key.strip_prefix(base) {
                if let Some(t) = el.types.iter().find(|t| capitalised(t) == rest) {
                    return Some((el, t));
                }
            }
        }
    }
    None
}

fn present(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `obj[key]`, or null when absent (a `Map` index would panic).
fn field<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a Value {
    obj.get(key).unwrap_or(&Value::Null)
}

fn has(obj: &Map<String, Value>, key: &str) -> bool {
    obj.get(key).is_some_and(present)
}

/// Any `value[x]` key (`valueQuantity` …) present.
fn has_choice(obj: &Map<String, Value>, base: &str) -> bool {
    obj.iter().any(|(k, v)| {
        k.strip_prefix(base)
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
            && present(v)
    })
}

/// The resource type a literal reference names, if it is one we can read.
fn referenced_type(reference: &str) -> Option<&str> {
    if reference.starts_with('#') || reference.starts_with("urn:") {
        return None;
    }
    let mut parts: Vec<&str> = reference.split('/').collect();
    if parts.len() >= 4 && parts[parts.len() - 2] == "_history" {
        parts.truncate(parts.len() - 2);
    }
    let t = *parts.get(parts.len().checked_sub(2)?)?;
    let is_type = t.starts_with(|c: char| c.is_ascii_uppercase())
        && t.chars().all(|c| c.is_ascii_alphanumeric());
    is_type.then_some(t)
}

/// `a <= b` for two FHIR dateTime/instant values, when both can be compared exactly:
/// full date-times (any offset), or partial dates of the same precision.
fn not_after(a: &str, b: &str) -> Option<bool> {
    let full = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    match (full(a), full(b)) {
        (Some(x), Some(y)) => Some(x <= y),
        (None, None) if a.len() == b.len() && a.len() <= 10 => Some(a <= b),
        _ => None,
    }
}

struct Validator<'a> {
    root: &'a Value,
    issues: Vec<Issue>,
}

impl Validator<'_> {
    fn issue(&mut self, path: &str, rule: &str, message: String) {
        self.issues.push(Issue {
            path: path.to_owned(),
            rule: rule.to_owned(),
            message,
        });
    }

    /// Walk the object found at `at`, defined by the elements under `ctx`.
    fn object(&mut self, obj: &Map<String, Value>, ctx: &str, at: &str) {
        for (key, value) in obj {
            if key == "resourceType" || key.starts_with('_') {
                continue;
            }
            let Some((el, ty)) = find(ctx, key) else {
                continue;
            };
            let items: Vec<(String, &Value)> = match value {
                Value::Array(a) => a
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (format!("{at}.{key}[{i}]"), v))
                    .collect(),
                v => vec![(format!("{at}.{key}"), v)],
            };
            for (here, item) in items {
                let Some(child) = item.as_object() else {
                    continue;
                };
                if let Some(target) = el.content_ref {
                    self.element_rules(target, child, &here);
                    self.object(child, target, &here);
                } else if COMPLEX_TYPES.contains(&ty) {
                    self.element_rules(el.path, child, &here);
                    self.datatype_rules(ty, el, child, &here);
                    self.object(child, ty, &here);
                } else if ty == "BackboneElement" || ty == "Element" {
                    self.element_rules(el.path, child, &here);
                    self.object(child, el.path, &here);
                }
            }
        }
        for el in children(ctx) {
            if el.min == 0 {
                continue;
            }
            let name = name_of(el.path);
            let there = match name.strip_suffix("[x]") {
                Some(base) => has_choice(obj, base),
                None => has(obj, name),
            };
            if !there {
                let shown = name.replace("[x]", "");
                self.issue(
                    &format!("{at}.{shown}"),
                    "required",
                    format!("{} is required ({}..)", el.path, el.min),
                );
            }
        }
    }

    fn datatype_rules(&mut self, ty: &str, el: &Element, obj: &Map<String, Value>, at: &str) {
        match ty {
            "Reference" => self.reference(el, obj, at),
            "CodeableReference" => {
                if let Some(r) = obj.get("reference").and_then(Value::as_object) {
                    self.target(el, r, &format!("{at}.reference"));
                }
            }
            "Attachment" if has(obj, "data") && !has(obj, "contentType") => self.issue(
                at,
                "att-1",
                "an Attachment with data has a contentType".into(),
            ),
            "Quantity" | "Age" | "Count" | "Distance" | "Duration" | "SimpleQuantity"
                if has(obj, "code") && !has(obj, "system") =>
            {
                self.issue(at, "qty-3", "a unit code comes with its system".into());
            }
            "Period" => {
                if let (Some(s), Some(e)) = (
                    obj.get("start").and_then(Value::as_str),
                    obj.get("end").and_then(Value::as_str),
                ) {
                    if not_after(s, e) == Some(false) {
                        self.issue(at, "per-1", format!("start {s} is after end {e}"));
                    }
                }
            }
            "ContactPoint" if has(obj, "value") && !has(obj, "system") => {
                self.issue(
                    at,
                    "cpt-2",
                    "a ContactPoint value comes with its system".into(),
                );
            }
            "Extension" if has(obj, "extension") == has_choice(obj, "value") => self.issue(
                at,
                "ext-1",
                "an Extension has either nested extensions or a value[x], not both or neither"
                    .into(),
            ),
            _ => {}
        }
    }

    fn reference(&mut self, el: &Element, obj: &Map<String, Value>, at: &str) {
        if !(has(obj, "reference")
            || has(obj, "identifier")
            || has(obj, "display")
            || has(obj, "extension"))
        {
            self.issue(
                at,
                "ref-2",
                "a Reference has a reference, an identifier or a display".into(),
            );
        }
        if let Some(r) = obj.get("reference").and_then(Value::as_str) {
            if let Some(local) = r.strip_prefix('#') {
                let contained = self.root["contained"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(|x| x["id"] == local));
                if !contained {
                    self.issue(
                        at,
                        "ref-1",
                        format!("{r} names a contained resource the resource does not have"),
                    );
                }
            }
        }
        self.target(el, obj, at);
    }

    /// The referenced type is one the element allows.
    fn target(&mut self, el: &Element, obj: &Map<String, Value>, at: &str) {
        if el.targets.is_empty() || el.targets.contains(&"Resource") {
            return;
        }
        let Some(r) = obj.get("reference").and_then(Value::as_str) else {
            return;
        };
        if let Some(t) = referenced_type(r) {
            if !el.targets.contains(&t) {
                self.issue(
                    at,
                    "reference-target",
                    format!(
                        "{} may reference {}, not {t}",
                        el.path,
                        el.targets.join(" | ")
                    ),
                );
            }
        }
    }

    /// Invariants on a backbone or datatype element, by its definition path.
    fn element_rules(&mut self, path: &str, obj: &Map<String, Value>, at: &str) {
        match path {
            "Patient.contact"
                if !(has(obj, "name")
                    || has(obj, "telecom")
                    || has(obj, "address")
                    || has(obj, "organization")) =>
            {
                self.issue(
                    at,
                    "pat-1",
                    "a contact has a name, telecom, address or organization".into(),
                );
            }
            "Observation.referenceRange"
                if !(has(obj, "low") || has(obj, "high") || has(obj, "text")) =>
            {
                self.issue(
                    at,
                    "obs-3",
                    "a reference range has a low, a high or a text".into(),
                );
            }
            "Organization.contact" => {
                let home = |v: &Value| v["use"] == "home";
                if obj
                    .get("telecom")
                    .and_then(Value::as_array)
                    .is_some_and(|t| t.iter().any(home))
                {
                    self.issue(
                        at,
                        "org-3",
                        "an organization's telecom is never 'home'".into(),
                    );
                }
                if obj.get("address").is_some_and(home) {
                    self.issue(
                        at,
                        "org-4",
                        "an organization's address is never 'home'".into(),
                    );
                }
            }
            _ => {}
        }
    }

    /// Invariants on the resource itself.
    fn resource_rules(&mut self, rt: &str, obj: &Map<String, Value>) {
        match rt {
            "Organization" if !has(obj, "identifier") && !has(obj, "name") => self.issue(
                rt,
                "org-1",
                "an Organization has a name or an identifier".into(),
            ),
            "Observation" => {
                let value = has_choice(obj, "value");
                if value && has(obj, "dataAbsentReason") {
                    self.issue(
                        rt,
                        "obs-6",
                        "dataAbsentReason only when there is no value[x]".into(),
                    );
                }
                if has(obj, "bodySite") && has(obj, "bodyStructure") {
                    self.issue(
                        rt,
                        "obs-8",
                        "bodyStructure only when there is no bodySite".into(),
                    );
                }
                if value {
                    let codes = |cc: &Value| -> Vec<(Value, Value)> {
                        cc["coding"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|c| (c["system"].clone(), c["code"].clone()))
                            .collect()
                    };
                    let own = codes(field(obj, "code"));
                    let repeated = field(obj, "component")
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|c| codes(&c["code"]).iter().any(|x| own.contains(x)));
                    if repeated {
                        self.issue(
                            rt,
                            "obs-7",
                            "no value[x] when a component has the Observation's own code".into(),
                        );
                    }
                }
            }
            "Device" => {
                let shown = field(obj, "name")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|n| n["display"] == true)
                    .count();
                if shown > 1 {
                    self.issue(
                        rt,
                        "dev-1",
                        "at most one Device.name is the display name".into(),
                    );
                }
            }
            "Task" => {
                if let (Some(m), Some(a)) = (
                    obj.get("lastModified").and_then(Value::as_str),
                    obj.get("authoredOn").and_then(Value::as_str),
                ) {
                    if not_after(a, m) == Some(false) {
                        self.issue(
                            rt,
                            "inv-1",
                            format!("lastModified {m} is before authoredOn {a}"),
                        );
                    }
                }
                if has(obj, "restriction") {
                    let fulfil = field(obj, "code")["coding"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|c| {
                            c["code"] == "fulfill"
                                && c["system"] == "http://hl7.org/fhir/CodeSystem/task-code"
                        });
                    if !fulfil || !has(obj, "focus") {
                        self.issue(
                            rt,
                            "tsk-1",
                            "restriction only on a fulfill Task with a focus".into(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
