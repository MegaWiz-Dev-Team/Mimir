//! Medical Domain Connector — ro-ai-domain-medical
//!
//! This crate provides medical-specific functionality for Project Mimir:
//! - Medical disclaimer enforcement
//! - NCBI/MedlinePlus data source connectors (future)
//! - ICD-10/UMLS tagging support (future)
//! - DICOM file handling utilities (future)
//!
//! The core domain routing logic lives in `mimir-core-ai::services::domain`.
//! This crate implements the domain-specific services that are invoked
//! when a tenant's domain is set to "medical".

pub mod pubmed;
pub mod curation;
pub mod graphrag;
pub mod search;
pub mod bigquery;
pub mod normalizer;
pub mod safety_pruner;

/// Medical Domain Service — entry point for medical-specific operations.
pub struct MedicalDomainService;

impl MedicalDomainService {
    pub fn new() -> Self {
        Self
    }

    /// Returns the mandatory medical disclaimer text.
    /// This must be appended to all AI responses for medical-domain tenants.
    pub fn disclaimer(&self) -> &str {
        "⚕️ MEDICAL DISCLAIMER: This information is for educational purposes only \
         and should not be considered as medical advice. Always consult a qualified \
         healthcare professional for medical decisions."
    }

    /// Placeholder: Validate a DICOM file header (future implementation).
    pub fn validate_dicom_header(&self, _data: &[u8]) -> bool {
        // TODO: Implement DICOM magic number check (DICM at offset 128)
        true
    }
}

impl Default for MedicalDomainService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_medical_disclaimer_not_empty() {
        let svc = MedicalDomainService::new();
        assert!(!svc.disclaimer().is_empty());
        assert!(svc.disclaimer().contains("DISCLAIMER"));
    }

    #[test]
    fn test_medical_service_default() {
        let svc = MedicalDomainService::default();
        assert!(!svc.disclaimer().is_empty());
    }
}
