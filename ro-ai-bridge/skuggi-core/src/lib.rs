//! 🌑 Skuggi core — Tier 1 PII detection **engine** + runtime-loadable rule set.
//!
//! Used by:
//!   - **Heimdall gateway** (`gateway/src/skuggi.rs`) for **redaction** of
//!     outbound LLM payloads (proxy.rs dispatch on `PiiMode`).
//!   - **Mimir** (`routes/admin_skuggi.rs`, `routes/a2a.rs`) for **leak
//!     detection** / dispatch redaction.
//!   - **skuggi-leak-runner / skuggi-bench** binaries in `ro-ai-bridge/src/bin/`.
//!
//! ## Engine vs rules split (v0.2 — open-core boundary)
//!
//! This crate is the **engine**: format-agnostic detection/redaction plumbing
//! ([`ReplaceMode`], [`RedactionResult`], [`RuleSet::redact_text`], etc.) plus a
//! generic, non-proprietary default rule set ([`RuleSet::builtin`]). It is and
//! stays **AGPL-public** so the Tier-B platform (Mimir, Heimdall) builds and runs
//! from public source.
//!
//! The **rules** — the tuned/proprietary detector set — are loaded at runtime from
//! a data file via [`RuleSet::load`] (`SKUGGI_RULES_PATH` env). When unset, the
//! engine falls back to [`RuleSet::builtin`], so behaviour is identical to v0.1.
//! No private *code* dependency is introduced; commercial/on-prem boxes drop in a
//! `skuggi-rules.toml` and set the env var. See
//! `Asgard/docs/technical/skuggi-extraction-migration.md` + ADR-023.
//!
//! ## Tier 1a — free-text finders
//! Format-based patterns that fire anywhere in text (Thai national ID,
//! Thai phone, email). Replacement is `Whole` (entire match → placeholder).
//!
//! ## Tier 1b — anchored form-field patterns
//! Label-anchored patterns for Thai medical certificate / discharge
//! summary / insurance form layouts (Patient Name:, Doctor Name:, HN:,
//! License Number:, ThaiID:). Replacement is `Group1` (label survives,
//! value replaced — so the LLM keeps structural context).
//!
//! Order in [`RuleSet::builtin`] is intentional: anchored runs first so a
//! `ThaiID: <digits>` hit is audited as `thai_id_anchored` (high-fidelity)
//! rather than swallowed by the free-text `thai_national_id` finder. This was
//! caught by leak-contract tests in Heimdall #6. Loaded rule files MUST preserve
//! the same ordering discipline (anchored before free-text mop-up).

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Replacement strategy per rule. `Whole` swaps the entire match for the
/// placeholder (free-text finders). `Group1` keeps the form label intact and
/// replaces only capture group 1 (anchors — the LLM still knows "this is the
/// patient name field").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceMode {
    Whole,
    Group1,
}

/// A single compiled Tier 1 PII detector.
///
/// `category` and `placeholder` are `&'static str` so the public API
/// ([`Detection::category`], [`RuleSet::scan_categories`]) can keep returning
/// `&'static str` — preserving the v0.1 contract that downstream callers
/// (e.g. Mimir's `pii_matches_in_response: Vec<&'static str>`) depend on. For
/// runtime-loaded rules these strings are intentionally leaked once at load
/// time (see [`RuleSet::from_toml_str`]).
pub struct Rule {
    pub category: &'static str,
    pub placeholder: &'static str,
    pub regex: Regex,
    pub mode: ReplaceMode,
}

/// An ordered set of Tier 1 detectors. **Order matters** — anchored patterns
/// must precede free-text finders (see crate docs).
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// The generic, non-proprietary baseline rule set. Public builds and dev/test
    /// runs use this so detection works out of the box. Identical to the v0.1
    /// hard-coded pattern table.
    pub fn builtin() -> Self {
        // (category, placeholder, pattern, mode) — anchored (Group1) first,
        // then free-text (Whole) mop-up.
        let specs: [(&'static str, &'static str, &str, ReplaceMode); 8] = [
            // Tier 1b — anchored
            (
                "patient_name",
                "[REDACTED_PATIENT_NAME]",
                r"(?i)Patient\s*Name\s*[:：]?\s*([^\n]+?)(?:\n|$)",
                ReplaceMode::Group1,
            ),
            (
                "doctor_name",
                "[REDACTED_DOCTOR_NAME]",
                r"(?i)Doctor\s*Name\s*[:：]?\s*([^\n]+?)(?:\n|$)",
                ReplaceMode::Group1,
            ),
            (
                "hn",
                "[REDACTED_HN]",
                r"(?i)\bHN\s*[:：]?\s*([0-9][\d\-/]*)",
                ReplaceMode::Group1,
            ),
            (
                "license_no",
                "[REDACTED_LICENSE_NO]",
                r"(?i)License\s*Number\s*[:：]?\s*((?:ว\.?\s*)?\d[\w.\-\s]*?)(?:\n|$)",
                ReplaceMode::Group1,
            ),
            (
                "thai_id_anchored",
                "[REDACTED_THAI_ID]",
                r"(?i)\bThai\s*ID\s*[:：]?\s*(\d{13})",
                ReplaceMode::Group1,
            ),
            // Tier 1a — free-text mop up
            (
                "thai_national_id",
                "[REDACTED_THAI_ID]",
                r"\b[1-8][- ]?\d{4}[- ]?\d{5}[- ]?\d{2}[- ]?\d\b",
                ReplaceMode::Whole,
            ),
            (
                "thai_phone",
                "[REDACTED_PHONE]",
                r"(?:\+66[- ]?|0)\d{1,2}[- ]?\d{3,4}[- ]?\d{4}\b",
                ReplaceMode::Whole,
            ),
            (
                "email",
                "[REDACTED_EMAIL]",
                r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}",
                ReplaceMode::Whole,
            ),
        ];

        let rules = specs
            .into_iter()
            .map(|(category, placeholder, pattern, mode)| Rule {
                category,
                placeholder,
                regex: Regex::new(pattern).expect("builtin skuggi regex must compile"),
                mode,
            })
            .collect();

        Self { rules }
    }

    /// Resolve the active rule set: load tuned rules from `SKUGGI_RULES_PATH`
    /// (commercial/on-prem) when set and readable, otherwise [`RuleSet::builtin`].
    /// **Never panics** — a missing/invalid file logs and falls back to builtin.
    pub fn load() -> Self {
        match std::env::var("SKUGGI_RULES_PATH") {
            Ok(path) if !path.trim().is_empty() => match Self::from_toml_path(&path) {
                Ok(rs) => rs,
                Err(e) => {
                    eprintln!(
                        "🌑 skuggi: failed to load rules from {path}: {e}; falling back to builtin"
                    );
                    Self::builtin()
                }
            },
            _ => Self::builtin(),
        }
    }

    /// Load a rule set from a TOML file. See [`RuleSet::from_toml_str`].
    pub fn from_toml_path(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let text = std::fs::read_to_string(path)?;
        Self::from_toml_str(&text)
    }

    /// Parse a rule set from a TOML document of the form:
    ///
    /// ```toml
    /// [[rule]]
    /// category = "patient_name"
    /// placeholder = "[REDACTED_PATIENT_NAME]"
    /// pattern = "(?i)Patient\\s*Name\\s*[:：]?\\s*([^\\n]+?)(?:\\n|$)"
    /// mode = "group1"   # or "whole"
    /// ```
    ///
    /// `category`/`placeholder` are leaked to `&'static str`. This is a bounded,
    /// intentional, **load-once-at-startup** leak: the rule set is held in a
    /// process-global for the program's lifetime anyway.
    pub fn from_toml_str(text: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file: RuleFile = toml::from_str(text)?;
        let mut rules = Vec::with_capacity(file.rule.len());
        for spec in file.rule {
            let regex = Regex::new(&spec.pattern)?;
            let category: &'static str = Box::leak(spec.category.into_boxed_str());
            let placeholder: &'static str = Box::leak(spec.placeholder.into_boxed_str());
            rules.push(Rule {
                category,
                placeholder,
                regex,
                mode: spec.mode.into(),
            });
        }
        Ok(Self { rules })
    }

    /// Number of detectors in this set.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// True when the set has no detectors.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Apply all rules to `text` and return redacted text + detection audit.
    /// **Always succeeds.**
    pub fn redact_text(&self, text: &str) -> RedactionResult {
        let mut current = text.to_string();
        let mut detections = Vec::new();

        for rule in &self.rules {
            let count = rule.regex.find_iter(&current).count();
            if count == 0 {
                continue;
            }
            match rule.mode {
                ReplaceMode::Whole => {
                    current = rule.regex.replace_all(&current, rule.placeholder).into_owned();
                }
                ReplaceMode::Group1 => {
                    let placeholder = rule.placeholder;
                    current = rule
                        .regex
                        .replace_all(&current, |caps: &regex::Captures| {
                            let whole = &caps[0];
                            match caps.get(1) {
                                Some(g1) => {
                                    let start = g1.start() - caps.get(0).unwrap().start();
                                    let end = g1.end() - caps.get(0).unwrap().start();
                                    format!("{}{}{}", &whole[..start], placeholder, &whole[end..])
                                }
                                None => whole.to_string(),
                            }
                        })
                        .into_owned();
                }
            }
            detections.push(Detection {
                category: rule.category,
                count,
            });
        }

        RedactionResult {
            redacted_text: current,
            detections,
        }
    }

    /// Returns the list of PII categories present in `text`. Lighter than
    /// [`RuleSet::redact_text`] — used by leak-detection scorers.
    pub fn scan_categories(&self, text: &str) -> Vec<&'static str> {
        let mut hits: Vec<&'static str> = Vec::new();
        for rule in &self.rules {
            if rule.regex.is_match(text) {
                hits.push(rule.category);
            }
        }
        hits
    }

    /// Walk an OpenAI-style chat-completions JSON body and Tier-1-redact every
    /// user-visible text field, in place. Returns aggregate detections.
    ///
    /// Handles two `messages[*].content` shapes:
    ///   - `"content": "string"` — redacted directly
    ///   - `"content": [{"type":"text","text":"…"}, {"type":"image_url",…}]`
    ///     — redacts only the `text` fields; `image_url` is left untouched.
    pub fn redact_chat_body(&self, body: &mut serde_json::Value) -> Vec<Detection> {
        let mut totals: std::collections::HashMap<&'static str, usize> =
            std::collections::HashMap::new();

        let Some(messages) = body.get_mut("messages").and_then(|v| v.as_array_mut()) else {
            return Vec::new();
        };

        for msg in messages.iter_mut() {
            let Some(content) = msg.get_mut("content") else {
                continue;
            };
            match content {
                serde_json::Value::String(s) => {
                    let r = self.redact_text(s);
                    for d in r.detections {
                        *totals.entry(d.category).or_insert(0) += d.count;
                    }
                    *s = r.redacted_text;
                }
                serde_json::Value::Array(parts) => {
                    for part in parts.iter_mut() {
                        let Some(text) = part.get_mut("text").and_then(|v| match v {
                            serde_json::Value::String(s) => Some(s),
                            _ => None,
                        }) else {
                            continue;
                        };
                        let r = self.redact_text(text);
                        for d in r.detections {
                            *totals.entry(d.category).or_insert(0) += d.count;
                        }
                        *text = r.redacted_text;
                    }
                }
                _ => {}
            }
        }

        let mut out: Vec<Detection> = totals
            .into_iter()
            .map(|(category, count)| Detection { category, count })
            .collect();
        out.sort_by_key(|d| d.category);
        out
    }
}

// ─── TOML rule-file schema (private) ──────────────────────────────────────

#[derive(Deserialize)]
struct RuleFile {
    #[serde(default)]
    rule: Vec<RuleSpec>,
}

#[derive(Deserialize)]
struct RuleSpec {
    category: String,
    placeholder: String,
    pattern: String,
    #[serde(default)]
    mode: RuleMode,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum RuleMode {
    #[default]
    Whole,
    Group1,
}

impl From<RuleMode> for ReplaceMode {
    fn from(m: RuleMode) -> Self {
        match m {
            RuleMode::Whole => ReplaceMode::Whole,
            RuleMode::Group1 => ReplaceMode::Group1,
        }
    }
}

// ─── Detection result types ───────────────────────────────────────────────

/// Per-call result from [`redact_text`].
#[derive(Debug, Serialize, Default, Clone)]
pub struct RedactionResult {
    /// Text with every Tier 1 PII match replaced by the category placeholder.
    pub redacted_text: String,
    /// Non-empty when at least one match was found.
    pub detections: Vec<Detection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Detection {
    pub category: &'static str,
    /// Number of matches replaced for this category.
    pub count: usize,
}

// ─── Process-global default rule set + free-function compatibility API ─────
//
// The free functions preserve the exact v0.1 signatures so Heimdall + Mimir
// callsites compile unchanged. They delegate to a process-global `RuleSet`
// resolved once via `RuleSet::load()` (builtin unless `SKUGGI_RULES_PATH` set).

static DEFAULT_RULES: Lazy<RuleSet> = Lazy::new(RuleSet::load);

/// Force-initialise the process-global rule set and return its detector count.
/// Optional — the set initialises lazily on first use — but callers (e.g.
/// Heimdall startup) can call this to load eagerly and log the active count /
/// surface a bad `SKUGGI_RULES_PATH` early.
pub fn init_default_rules() -> usize {
    DEFAULT_RULES.len()
}

/// Apply the active rule set's Tier 1 redaction to `text`. See
/// [`RuleSet::redact_text`].
pub fn redact_text(text: &str) -> RedactionResult {
    DEFAULT_RULES.redact_text(text)
}

/// Categories present in `text` under the active rule set. See
/// [`RuleSet::scan_categories`].
pub fn scan_categories(text: &str) -> Vec<&'static str> {
    DEFAULT_RULES.scan_categories(text)
}

/// Redact an OpenAI-style chat body in place under the active rule set. See
/// [`RuleSet::redact_chat_body`].
pub fn redact_chat_body(body: &mut serde_json::Value) -> Vec<Detection> {
    DEFAULT_RULES.redact_chat_body(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_thai_national_id() {
        let r = redact_text("ผู้ป่วย ID 1-9001-00000-01-1 มาตรวจ");
        assert!(r.redacted_text.contains("[REDACTED_THAI_ID]"));
        assert!(r.detections.iter().any(|d| d.category == "thai_national_id"));
    }

    #[test]
    fn redacts_phone_intl() {
        let r = redact_text("call +66 81 555 0001 anytime");
        assert!(r.redacted_text.contains("[REDACTED_PHONE]"));
    }

    #[test]
    fn redacts_email() {
        let r = redact_text("send to pii-test@example.com today");
        assert!(r.redacted_text.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn anchored_patient_name_preserves_label() {
        let r = redact_text("Patient Name: SYNTHETIC_PATIENT\nDiagnosis: ไข้");
        assert!(r.redacted_text.contains("Patient Name: [REDACTED_PATIENT_NAME]"));
        assert!(r.redacted_text.contains("Diagnosis: ไข้"));
    }

    #[test]
    fn anchored_runs_before_free_text() {
        // Confirms the algorithmic order: ThaiID label captured as
        // `thai_id_anchored`, not swallowed by free-text finder.
        let r = redact_text("ThaiID: 1111111111110\n");
        let cats: Vec<&str> = r.detections.iter().map(|d| d.category).collect();
        assert!(cats.contains(&"thai_id_anchored"));
        assert!(!cats.contains(&"thai_national_id"));
    }

    #[test]
    fn scan_categories_matches_redact_text() {
        // Both should agree on what's present (modulo replacement).
        let text = "Patient Name: A\nphone 081-555-0001 email a@b.co";
        let cats = scan_categories(text);
        assert!(cats.contains(&"patient_name"));
        assert!(cats.contains(&"thai_phone"));
        assert!(cats.contains(&"email"));
    }

    #[test]
    fn scan_categories_empty_on_clean_text() {
        assert!(scan_categories("Patient is stable. No complications.").is_empty());
    }

    #[test]
    fn chat_body_redacts_string_content() {
        let mut body: serde_json::Value = serde_json::json!({
            "messages": [{"role": "user", "content": "MRN 1-9001-00000-01-1 came to ER"}]
        });
        let dets = redact_chat_body(&mut body);
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("[REDACTED_THAI_ID]"));
        assert_eq!(dets.len(), 1);
    }

    #[test]
    fn chat_body_array_content_redacts_text_parts_only() {
        let mut body: serde_json::Value = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "phone 081-555-0001"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,…"}}
                ]
            }]
        });
        let _dets = redact_chat_body(&mut body);
        let text = body["messages"][0]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("[REDACTED_PHONE]"));
        // image_url untouched
        assert!(body["messages"][0]["content"][1]["image_url"]["url"]
            .as_str().unwrap().contains("base64"));
    }

    // ─── v0.2 RuleSet API ──────────────────────────────────────────────────

    #[test]
    fn builtin_has_eight_rules_in_anchored_first_order() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.len(), 8);
        assert!(!rs.is_empty());
        // First five are the anchored Group1 detectors, last three free-text.
        assert_eq!(rs.rules[0].category, "patient_name");
        assert_eq!(rs.rules[0].mode, ReplaceMode::Group1);
        assert_eq!(rs.rules[5].category, "thai_national_id");
        assert_eq!(rs.rules[5].mode, ReplaceMode::Whole);
        assert_eq!(rs.rules[7].category, "email");
    }

    #[test]
    fn builtin_method_matches_free_function() {
        // The free functions delegate to load() which (env unset in tests)
        // resolves to builtin(), so results must agree.
        let rs = RuleSet::builtin();
        let text = "Patient Name: A\nphone 081-555-0001";
        assert_eq!(
            rs.redact_text(text).redacted_text,
            redact_text(text).redacted_text
        );
    }

    #[test]
    fn from_toml_str_loads_custom_rules_and_leaks_static() {
        let toml = r#"
[[rule]]
category = "custom_token"
placeholder = "[REDACTED_TOKEN]"
pattern = "TOK-\\d+"
mode = "whole"

[[rule]]
category = "case_id"
placeholder = "[REDACTED_CASE]"
pattern = "(?i)Case\\s*ID\\s*[:：]?\\s*(\\w+)"
mode = "group1"
"#;
        let rs = RuleSet::from_toml_str(toml).expect("valid toml");
        assert_eq!(rs.len(), 2);

        let r = rs.redact_text("ref TOK-12345 and Case ID: ABC99");
        assert!(r.redacted_text.contains("[REDACTED_TOKEN]"));
        assert!(r.redacted_text.contains("Case ID: [REDACTED_CASE]"));
        assert!(!r.redacted_text.contains("TOK-12345"));
        assert!(!r.redacted_text.contains("ABC99"));

        // category survives as &'static str (the leak)
        let cats = rs.scan_categories("TOK-1");
        assert_eq!(cats, vec!["custom_token"]);
    }

    #[test]
    fn from_toml_str_defaults_mode_to_whole() {
        let toml = r#"
[[rule]]
category = "x"
placeholder = "[X]"
pattern = "secret"
"#;
        let rs = RuleSet::from_toml_str(toml).unwrap();
        assert_eq!(rs.rules[0].mode, ReplaceMode::Whole);
    }

    #[test]
    fn from_toml_str_rejects_bad_regex() {
        let toml = r#"
[[rule]]
category = "bad"
placeholder = "[X]"
pattern = "("
mode = "whole"
"#;
        assert!(RuleSet::from_toml_str(toml).is_err());
    }

    #[test]
    fn load_falls_back_to_builtin_when_env_unset() {
        // SKUGGI_RULES_PATH is not set in the test environment.
        assert_eq!(RuleSet::load().len(), RuleSet::builtin().len());
    }

    #[test]
    fn from_toml_path_missing_file_is_err() {
        assert!(RuleSet::from_toml_path("/nonexistent/skuggi-rules.toml").is_err());
    }

    #[test]
    fn init_default_rules_returns_builtin_count() {
        assert_eq!(init_default_rules(), 8);
    }
}
