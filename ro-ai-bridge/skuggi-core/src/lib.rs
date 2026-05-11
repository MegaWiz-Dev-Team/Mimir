//! 🌑 Skuggi core — shared PII regex set + scoring primitives.
//!
//! Single source of truth for the Tier 1 PII detectors used by:
//!   - **Heimdall gateway** (`gateway/src/skuggi.rs`) for **redaction** of
//!     outbound LLM payloads (proxy.rs dispatch on `PiiMode`).
//!   - **Mimir** (`routes/admin_skuggi.rs`) for **leak detection** on LLM
//!     responses (score-batch endpoint).
//!   - **skuggi-leak-runner** (binary in `ro-ai-bridge/src/bin/`) which
//!     orchestrates corpus → agent → scorer end-to-end.
//!
//! Before this crate existed (Sprint 50b PR #6 / #273), each callsite
//! kept its own copy of the same regex — three sources of truth that
//! could drift independently. Consolidating here means a single
//! `cargo test --package skuggi-core` is the contract.
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
//! Order in [`patterns`] is intentional: anchored runs first so a
//! `ThaiID: <digits>` hit is audited as `thai_id_anchored` (high-
//! fidelity) rather than swallowed by the free-text `thai_national_id`
//! finder. This was caught by leak-contract tests in Heimdall #6.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ─── Tier 1a — free-text finders ─────────────────────────────────────────

static RE_THAI_NATIONAL_ID: Lazy<Regex> = Lazy::new(|| {
    // 13 digits, optionally separated by dashes/spaces. First digit
    // is 1-8 per Thai citizen-ID spec (excludes test ranges 0/9).
    Regex::new(r"\b[1-8][- ]?\d{4}[- ]?\d{5}[- ]?\d{2}[- ]?\d\b").unwrap()
});

static RE_THAI_PHONE: Lazy<Regex> = Lazy::new(|| {
    // Thai mobile/landline: 0X + 8 more digits, or +66 international prefix.
    Regex::new(r"(?:\+66[- ]?|0)\d{1,2}[- ]?\d{3,4}[- ]?\d{4}\b").unwrap()
});

static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    // RFC-5322 simplified — covers ~99% of clinical-doc emails.
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap()
});

// ─── Tier 1b — anchored form-field patterns ──────────────────────────────

static RE_PATIENT_NAME_ANCHOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Patient\s*Name\s*[:：]?\s*([^\n]+?)(?:\n|$)").unwrap()
});

static RE_DOCTOR_NAME_ANCHOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Doctor\s*Name\s*[:：]?\s*([^\n]+?)(?:\n|$)").unwrap()
});

static RE_HN_ANCHOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bHN\s*[:：]?\s*([0-9][\d\-/]*)").unwrap()
});

static RE_LICENSE_NO_ANCHOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)License\s*Number\s*[:：]?\s*((?:ว\.?\s*)?\d[\w.\-\s]*?)(?:\n|$)").unwrap()
});

static RE_THAI_ID_ANCHOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bThai\s*ID\s*[:：]?\s*(\d{13})").unwrap()
});

/// Replacement strategy per pattern. `Whole` swaps the entire match
/// for the placeholder (free-text finders). `Group1` keeps the form
/// label intact and replaces only capture group 1 (anchors — the LLM
/// still knows "this is the patient name field").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceMode {
    Whole,
    Group1,
}

/// Canonical pattern dispatch table.
///
/// **Order matters**: anchored patterns run FIRST so a labelled field
/// (`ThaiID: 1111111111110`) is audited as the anchored category (high-
/// fidelity form context) rather than swallowed by the free-text finder.
/// Free-text finders mop up unlabelled PII afterward.
pub fn patterns() -> [(&'static str, &'static str, &'static Regex, ReplaceMode); 8] {
    [
        // Tier 1b — anchored first
        ("patient_name",     "[REDACTED_PATIENT_NAME]", &RE_PATIENT_NAME_ANCHOR, ReplaceMode::Group1),
        ("doctor_name",      "[REDACTED_DOCTOR_NAME]",  &RE_DOCTOR_NAME_ANCHOR,  ReplaceMode::Group1),
        ("hn",               "[REDACTED_HN]",           &RE_HN_ANCHOR,           ReplaceMode::Group1),
        ("license_no",       "[REDACTED_LICENSE_NO]",   &RE_LICENSE_NO_ANCHOR,   ReplaceMode::Group1),
        ("thai_id_anchored", "[REDACTED_THAI_ID]",      &RE_THAI_ID_ANCHOR,      ReplaceMode::Group1),
        // Tier 1a — mop up
        ("thai_national_id", "[REDACTED_THAI_ID]",      &RE_THAI_NATIONAL_ID,    ReplaceMode::Whole),
        ("thai_phone",       "[REDACTED_PHONE]",        &RE_THAI_PHONE,          ReplaceMode::Whole),
        ("email",            "[REDACTED_EMAIL]",        &RE_EMAIL,               ReplaceMode::Whole),
    ]
}

// ─── Detection ───────────────────────────────────────────────────────────

/// Per-call result from [`redact_text`].
#[derive(Debug, Serialize, Default, Clone)]
pub struct RedactionResult {
    /// Text with every Tier 1 PII match replaced by the category placeholder.
    pub redacted_text: String,
    /// Non-empty when at least one match was found. Order: same as [`patterns`].
    pub detections: Vec<Detection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Detection {
    pub category: &'static str,
    /// Number of matches replaced for this category.
    pub count: usize,
}

/// Apply all Tier 1 patterns to `text` and return redacted text +
/// detection audit. **Always succeeds**.
///
/// Used by Heimdall for redaction; Mimir's leak-detection uses
/// [`scan_categories`] instead (which doesn't need the redacted text).
pub fn redact_text(text: &str) -> RedactionResult {
    let mut current = text.to_string();
    let mut detections = Vec::new();

    for (category, placeholder, regex, mode) in patterns() {
        let count = regex.find_iter(&current).count();
        if count == 0 {
            continue;
        }
        match mode {
            ReplaceMode::Whole => {
                current = regex.replace_all(&current, placeholder).into_owned();
            }
            ReplaceMode::Group1 => {
                current = regex.replace_all(&current, |caps: &regex::Captures| {
                    let whole = &caps[0];
                    match caps.get(1) {
                        Some(g1) => {
                            let start = g1.start() - caps.get(0).unwrap().start();
                            let end = g1.end() - caps.get(0).unwrap().start();
                            format!("{}{}{}", &whole[..start], placeholder, &whole[end..])
                        }
                        None => whole.to_string(),
                    }
                }).into_owned();
            }
        }
        detections.push(Detection { category, count });
    }

    RedactionResult { redacted_text: current, detections }
}

/// Returns the list of PII categories present in `text`. Lighter than
/// [`redact_text`] — used by leak-detection scorers that only need the
/// category set, not the redacted output.
pub fn scan_categories(text: &str) -> Vec<&'static str> {
    let mut hits: Vec<&'static str> = Vec::new();
    for (category, _placeholder, regex, _mode) in patterns() {
        if regex.is_match(text) {
            hits.push(category);
        }
    }
    hits
}

// ─── OpenAI-style chat-body redaction ────────────────────────────────────

/// Walk an OpenAI-style chat-completions JSON body and Tier-1-redact every
/// user-visible text field. Modifies `body` in place. Returns aggregate
/// detections so the caller can log/audit.
///
/// Handles two `messages[*].content` shapes:
///   - `"content": "string"` — redacted directly
///   - `"content": [{"type":"text","text":"…"}, {"type":"image_url",…}]`
///     — redacts only the `text` fields; `image_url` is left untouched
///       (image PII redaction is Sprint 50b Phase 2 — OpenCV YuNet).
pub fn redact_chat_body(body: &mut serde_json::Value) -> Vec<Detection> {
    let mut totals: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();

    let Some(messages) = body.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        return Vec::new();
    };

    for msg in messages.iter_mut() {
        let Some(content) = msg.get_mut("content") else { continue };
        match content {
            serde_json::Value::String(s) => {
                let r = redact_text(s);
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
                    }) else { continue };
                    let r = redact_text(text);
                    for d in r.detections {
                        *totals.entry(d.category).or_insert(0) += d.count;
                    }
                    *text = r.redacted_text;
                }
            }
            _ => {}
        }
    }

    let mut out: Vec<Detection> = totals.into_iter()
        .map(|(category, count)| Detection { category, count })
        .collect();
    out.sort_by_key(|d| d.category);
    out
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
}
