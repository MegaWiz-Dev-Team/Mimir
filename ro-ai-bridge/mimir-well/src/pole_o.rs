//! POLE+O ontology — applied to `asgard_insurance` tenant only.
//!
//! ADR-011 §D4. POLE+O = Person / Object / Location / Event / Organization.
//! Operational-intel ontology that fits insurance underwriting cleanly
//! (insureds, policies, claim events, hospitals, insurers).
//!
//! Implemented as sub-labels on `:Artifact`:
//!   (:Artifact:Person {tenant_id: 'asgard_insurance', ...})
//!
//! **Scope rule:** apply ONLY when tenant_id == "asgard_insurance".
//! Forcing POLE+O on `asgard_medical` would degrade PrimeKG biomedical
//! relationships.

/// POLE+O entity types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoleO {
    /// Insured, beneficiary, doctor, agent.
    Person,
    /// Policy, rider, claim doc, medical certificate.
    Object,
    /// Branch, hospital, incident site.
    Location,
    /// Application, UW decision, claim event.
    Event,
    /// Insurer, reinsurer, hospital, employer.
    Organization,
}

impl PoleO {
    /// Neo4j sub-label string.
    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Object => "Object",
            Self::Location => "Location",
            Self::Event => "Event",
            Self::Organization => "Organization",
        }
    }

    /// Scope guard — POLE+O labels are applied only for asgard_insurance.
    /// Returns true if the given tenant is eligible.
    pub fn applies_to_tenant(tenant_id: &str) -> bool {
        tenant_id == "asgard_insurance"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_only_to_insurance() {
        assert!(PoleO::applies_to_tenant("asgard_insurance"));
        assert!(!PoleO::applies_to_tenant("asgard_medical"));
        assert!(!PoleO::applies_to_tenant("asgard_platform"));
        assert!(!PoleO::applies_to_tenant("asgard_wellness"));
    }

    #[test]
    fn label_strings_stable() {
        assert_eq!(PoleO::Person.label(), "Person");
        assert_eq!(PoleO::Object.label(), "Object");
        assert_eq!(PoleO::Location.label(), "Location");
        assert_eq!(PoleO::Event.label(), "Event");
        assert_eq!(PoleO::Organization.label(), "Organization");
    }
}
