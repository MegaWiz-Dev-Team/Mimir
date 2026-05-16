//! Finance domain configuration

/// Finance domain configuration
pub struct FinanceDomain;

impl FinanceDomain {
    /// Finance entity types
    pub const ENTITY_TYPES: &'static [&'static str] =
        &["account_type", "fee", "rate", "product", "regulation"];

    /// Finance relationship types
    pub const RELATIONSHIP_TYPES: &'static [&'static str] =
        &["HAS_FEE", "HAS_RATE", "SUBJECT_TO", "REQUIRES", "OFFERS"];
}
