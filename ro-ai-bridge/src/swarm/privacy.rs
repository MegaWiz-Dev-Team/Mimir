use regex::Regex;
use std::sync::OnceLock;

static PHONE_REGEX: OnceLock<Regex> = OnceLock::new();
static NID_REGEX: OnceLock<Regex> = OnceLock::new();

/// Rig-Guard Medical Privacy Shield
/// Evaluates and masks Personally Identifiable Information (PII) to prevent
/// unauthorized data leakage to upstream LLM clouds.
pub struct MedicalPrivacyShield;

impl MedicalPrivacyShield {
    /// Scrubs an input string of high-risk PII constructs.
    pub fn scrub(input: &str) -> String {
        let phone_re = PHONE_REGEX.get_or_init(|| Regex::new(r"(?:\+66\s?|0)[689]\d{8}").unwrap());
        // Matches typical Thai 13-digit national IDs with optional dashes/spaces
        let nid_re = NID_REGEX.get_or_init(|| Regex::new(r"\b\d{1}[\s-]?\d{4}[\s-]?\d{5}[\s-]?\d{2}[\s-]?\d{1}\b").unwrap());

        let mut output = input.to_string();
        output = nid_re.replace_all(&output, "[REDACTED_NID]").to_string();
        output = phone_re.replace_all(&output, "[REDACTED_PHONE]").to_string();

        output
    }
}
