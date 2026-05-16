//! Medical domain configuration

/// Medical domain configuration
pub struct MedicalDomain;

impl MedicalDomain {
    /// Medical entity types
    pub const ENTITY_TYPES: &'static [&'static str] =
        &["symptom", "diagnosis", "drug", "procedure", "condition"];

    /// Medical relationship types
    pub const RELATIONSHIP_TYPES: &'static [&'static str] = &[
        "INDICATES",
        "TREATS",
        "CONTRAINDICATED_WITH",
        "CAUSES",
        "REQUIRES",
    ];
}
