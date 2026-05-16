//! Domain-specific type definitions and configurations for RefGraph
//!
//! This crate provides domain configurations for insurance, medical, legal, and finance domains.
//! Each domain defines entity types, relationship types, and consolidation rules specific to that domain.

pub mod insurance;
pub mod medical;
pub mod legal;
pub mod finance;

pub use insurance::InsuranceDomain;
pub use medical::MedicalDomain;
pub use legal::LegalDomain;
pub use finance::FinanceDomain;
