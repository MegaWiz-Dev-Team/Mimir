//! Legal domain configuration

/// Legal domain configuration
pub struct LegalDomain;

impl LegalDomain {
    /// Legal entity types
    pub const ENTITY_TYPES: &'static [&'static str] =
        &["clause", "party", "obligation", "right", "jurisdiction"];

    /// Legal relationship types
    pub const RELATIONSHIP_TYPES: &'static [&'static str] =
        &["DEFINES", "OBLIGATES", "GRANTS", "REQUIRES", "GOVERNED_BY"];
}
