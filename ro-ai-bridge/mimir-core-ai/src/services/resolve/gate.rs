//! Pure medical-safety gate for the deduplication decision.
//!
//! A false merge of two distinct medical entities (two different "Paris", two
//! different patients, two unrelated drugs with similar names) silently corrupts
//! the graph and is expensive to undo. So for medical entity types we refuse to
//! auto-merge on similarity alone: an auto-merge is only allowed when an ontology
//! code (SNOMED / UMLS / ICD / RxNorm) matches. Otherwise it is downgraded to
//! human review.
//!
//! This is the single chokepoint between banding and any write. It is pure so it
//! can be unit-tested and so no code path can write a merge that bypasses it.

use super::scoring::Band;

/// Entity types treated as clinically sensitive (case-insensitive prefix/equality).
const MEDICAL_TYPES: &[&str] = &[
    "drug",
    "medication",
    "disease",
    "disorder",
    "condition",
    "symptom",
    "finding",
    "procedure",
    "gene",
    "protein",
    "anatomy",
    "pathogen",
    "patient",
];

/// Is this entity type clinically sensitive?
pub fn is_medical_type(entity_type: &str) -> bool {
    let t = entity_type.trim().to_lowercase();
    MEDICAL_TYPES.iter().any(|m| t == *m || t.starts_with(m))
}

/// Apply the medical-safety gate to a raw band.
///
/// - Medical type + `AutoMerge` + no ontology code match → downgrade to `Review`.
/// - Medical type + `AutoMerge` + `code_match` → keep `AutoMerge`.
/// - Non-medical types and the `Review` / `New` bands pass through unchanged.
pub fn medical_gate(band: Band, code_match: bool, entity_type: &str) -> Band {
    if band == Band::AutoMerge && is_medical_type(entity_type) && !code_match {
        Band::Review
    } else {
        band
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_medical_types_case_insensitively() {
        assert!(is_medical_type("DRUG"));
        assert!(is_medical_type("Disease"));
        assert!(is_medical_type("medication")); // plural-ish prefix
        assert!(is_medical_type("disorder_mental"));
        assert!(!is_medical_type("organization"));
        assert!(!is_medical_type("location"));
    }

    #[test]
    fn medical_automerge_without_code_is_downgraded() {
        assert_eq!(medical_gate(Band::AutoMerge, false, "DRUG"), Band::Review);
    }

    #[test]
    fn medical_automerge_with_code_is_allowed() {
        assert_eq!(medical_gate(Band::AutoMerge, true, "DRUG"), Band::AutoMerge);
    }

    #[test]
    fn nonmedical_automerge_passes_through() {
        assert_eq!(medical_gate(Band::AutoMerge, false, "ORGANIZATION"), Band::AutoMerge);
    }

    #[test]
    fn review_and_new_unaffected() {
        assert_eq!(medical_gate(Band::Review, false, "DRUG"), Band::Review);
        assert_eq!(medical_gate(Band::New, false, "DRUG"), Band::New);
    }
}