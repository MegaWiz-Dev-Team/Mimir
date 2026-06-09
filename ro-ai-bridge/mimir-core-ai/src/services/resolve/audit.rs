//! Provenance audit for entity merges.
//!
//! A merge is an irreversible-ish graph mutation, so each one emits a structured
//! audit record. Tyr (the Asgard SIEM, Wazuh-based) ingests structured logs, so
//! the record is emitted as a tracing event on the dedicated `tyr_audit` target
//! with stable field names. This is the local-first audit path; the same
//! [`MergeAudit`] struct is the seam to also push to an HTTP Tyr sink later
//! without changing call sites.

use serde::Serialize;

/// One auditable entity-merge event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeAudit {
    /// Stable action discriminator for SIEM rules.
    pub action: &'static str,
    pub tenant_id: String,
    pub entity_type: String,
    /// The canonical node that survived.
    pub survivor: String,
    /// The duplicate node that was tombstoned (its original surface name).
    pub duplicate: String,
    /// Who triggered the merge: a user id, "system", or "dream".
    pub merged_by: String,
    pub confidence: f64,
    /// Whether an ontology code (SNOMED/UMLS/ICD) matched — drives the medical gate.
    pub code_match: bool,
}

impl MergeAudit {
    pub fn new(
        tenant_id: &str,
        entity_type: &str,
        survivor: &str,
        duplicate: &str,
        merged_by: &str,
        confidence: f64,
        code_match: bool,
    ) -> Self {
        Self {
            action: "entity_merge",
            tenant_id: tenant_id.to_string(),
            entity_type: entity_type.to_string(),
            survivor: survivor.to_string(),
            duplicate: duplicate.to_string(),
            merged_by: merged_by.to_string(),
            confidence,
            code_match,
        }
    }
}

/// Emit a merge audit as a structured `tyr_audit` tracing event for SIEM ingest.
pub fn emit_merge_audit(a: &MergeAudit) {
    tracing::info!(
        target: "tyr_audit",
        action = a.action,
        tenant_id = %a.tenant_id,
        entity_type = %a.entity_type,
        survivor = %a.survivor,
        duplicate = %a.duplicate,
        merged_by = %a.merged_by,
        confidence = a.confidence,
        code_match = a.code_match,
        "entity merge"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_carries_stable_action_and_fields() {
        let a = MergeAudit::new("asgard_medical", "DRUG", "aspirin", "asprin", "reviewer", 0.97, true);
        assert_eq!(a.action, "entity_merge");
        assert_eq!(a.survivor, "aspirin");
        assert_eq!(a.duplicate, "asprin");
        assert!(a.code_match);
    }

    #[test]
    fn audit_serializes_with_expected_keys() {
        let a = MergeAudit::new("t", "DISEASE", "htn", "hypertension", "dream", 0.9, false);
        let j = serde_json::to_value(&a).unwrap();
        for k in ["action", "tenant_id", "entity_type", "survivor", "duplicate", "merged_by", "confidence", "code_match"] {
            assert!(j.get(k).is_some(), "audit JSON missing key {k}");
        }
        assert_eq!(j["action"], "entity_merge");
    }
}
