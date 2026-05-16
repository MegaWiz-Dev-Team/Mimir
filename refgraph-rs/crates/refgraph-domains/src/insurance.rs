//! Insurance domain configuration

/// Insurance domain configuration
pub struct InsuranceDomain;

impl InsuranceDomain {
    /// Insurance entity types
    pub const ENTITY_TYPES: &'static [&'static str] =
        &["product", "coverage", "exclusion", "condition", "organization"];

    /// Insurance relationship types
    pub const RELATIONSHIP_TYPES: &'static [&'static str] = &[
        "HAS_COVERAGE",
        "HAS_EXCLUSION",
        "REQUIRES_CONDITION",
        "EXCLUDES_CONDITION",
        "OFFERED_BY",
    ];
}
