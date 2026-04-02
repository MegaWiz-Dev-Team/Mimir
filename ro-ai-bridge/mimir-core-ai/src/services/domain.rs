//! Domain Connector Architecture — Issue #76
//!
//! Provides domain-specific routing for tenant types: `game`, `medical`, `general`.
//! Uses a Shared Core + Domain-specific Connector pattern with feature flags.

use anyhow::{Result, bail};

// ============================================================
// Domain Constants
// ============================================================

pub const DOMAIN_GAME: &str = "game";
pub const DOMAIN_MEDICAL: &str = "medical";
pub const DOMAIN_GENERAL: &str = "general";

/// All known feature keys for domain-based feature flags.
pub const ALL_FEATURES: &[&str] = &[
    "dicom",
    "medical_sources",
    "ai_vision_ocr",
    "medical_disclaimer",
    "rathena_connector",
];

// ============================================================
// DomainConnector Trait
// ============================================================

/// Trait representing a domain-specific connector.
/// Each domain (game, medical, general) implements this to provide
/// domain-specific behavior such as allowed file extensions,
/// system prompts, and feature flags.
pub trait DomainConnector: Send + Sync {
    /// Returns the domain name (e.g., "game", "medical", "general").
    fn domain_name(&self) -> &str;

    /// Returns the domain-specific system prompt prefix.
    fn system_prompt(&self) -> &str;

    /// Returns allowed file extensions for this domain.
    fn allowed_extensions(&self) -> &[&str];

    /// Returns a list of enabled feature keys for this domain.
    fn enabled_features(&self) -> &[&str];
}

// ============================================================
// GameConnector
// ============================================================

pub struct GameConnector;

impl DomainConnector for GameConnector {
    fn domain_name(&self) -> &str {
        DOMAIN_GAME
    }

    fn system_prompt(&self) -> &str {
        "You are a game data assistant specializing in MMORPG server data and rAthena databases."
    }

    fn allowed_extensions(&self) -> &[&str] {
        &[
            "pdf", "csv", "xlsx", "xls", "txt", "docx", "doc", "json", "md", "html", "htm", "xml",
            "yaml", "yml", "png", "jpg", "jpeg",
        ]
    }

    fn enabled_features(&self) -> &[&str] {
        &["rathena_connector"]
    }
}

// ============================================================
// MedicalConnector
// ============================================================

pub struct MedicalConnector;

impl DomainConnector for MedicalConnector {
    fn domain_name(&self) -> &str {
        DOMAIN_MEDICAL
    }

    fn system_prompt(&self) -> &str {
        "You are a medical data assistant. All responses must include appropriate medical disclaimers. Do not provide direct medical advice."
    }

    fn allowed_extensions(&self) -> &[&str] {
        &[
            "pdf", "csv", "xlsx", "xls", "txt", "docx", "doc", "json", "md", "html", "htm", "xml",
            "yaml", "yml", "png", "jpg", "jpeg", "dicom", "dcm",
        ]
    }

    fn enabled_features(&self) -> &[&str] {
        &[
            "dicom",
            "medical_sources",
            "ai_vision_ocr",
            "medical_disclaimer",
        ]
    }
}

// ============================================================
// DefaultConnector (general)
// ============================================================

pub struct DefaultConnector;

impl DomainConnector for DefaultConnector {
    fn domain_name(&self) -> &str {
        DOMAIN_GENERAL
    }

    fn system_prompt(&self) -> &str {
        "You are a general-purpose knowledge assistant."
    }

    fn allowed_extensions(&self) -> &[&str] {
        &[
            "pdf", "csv", "xlsx", "xls", "txt", "docx", "doc", "json", "md", "html", "htm", "xml",
            "yaml", "yml", "png", "jpg", "jpeg",
        ]
    }

    fn enabled_features(&self) -> &[&str] {
        &["ai_vision_ocr"]
    }
}

// ============================================================
// Factory & Feature Flag Functions
// ============================================================

/// Returns the appropriate DomainConnector for the given domain string.
///
/// Defaults to `DefaultConnector` for unknown domain values.
pub fn get_domain_connector(domain: &str) -> Box<dyn DomainConnector> {
    match domain {
        DOMAIN_GAME => Box::new(GameConnector),
        DOMAIN_MEDICAL => Box::new(MedicalConnector),
        _ => Box::new(DefaultConnector),
    }
}

/// Check if a specific feature is enabled for a given domain.
///
/// Returns `true` if the domain's connector lists the feature as enabled.
pub fn is_feature_enabled(domain: &str, feature: &str) -> bool {
    let connector = get_domain_connector(domain);
    connector.enabled_features().contains(&feature)
}

/// Get all features with their enabled/disabled status for a domain.
///
/// Returns a Vec of (feature_key, is_enabled) tuples.
pub fn get_all_features(domain: &str) -> Vec<(&'static str, bool)> {
    let connector = get_domain_connector(domain);
    let enabled = connector.enabled_features();
    ALL_FEATURES
        .iter()
        .map(|f| (*f, enabled.contains(f)))
        .collect()
}

/// Validate that a filename has an extension allowed for the given domain.
///
/// # Errors
/// Returns `Err` with "Unsupported file type" if the extension is not
/// allowed for the specified domain.
pub fn validate_extension_for_domain(filename: &str, domain: &str) -> Result<()> {
    let connector = get_domain_connector(domain);
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    if ext.is_empty() || !connector.allowed_extensions().contains(&ext.as_str()) {
        bail!("Unsupported file type: .{} (domain: {})", ext, domain);
    }
    Ok(())
}

// ============================================================
// Tests (TDD — written first per Issue #76 spec)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-005a: get_domain_connector(domain="game") → GameConnector instance
    // ========================================
    #[test]
    fn ut_005a_get_domain_connector_game() {
        let connector = get_domain_connector("game");
        assert_eq!(connector.domain_name(), "game");
        assert!(
            connector.enabled_features().contains(&"rathena_connector"),
            "Game connector should have rathena_connector enabled"
        );
    }

    // ========================================
    // UT-005b: get_domain_connector(domain="medical") → MedicalConnector instance
    // ========================================
    #[test]
    fn ut_005b_get_domain_connector_medical() {
        let connector = get_domain_connector("medical");
        assert_eq!(connector.domain_name(), "medical");
        assert!(
            connector.system_prompt().contains("medical"),
            "Medical connector should have medical-related system prompt"
        );
    }

    // ========================================
    // UT-005c: get_domain_connector(domain="general") → DefaultConnector instance
    // ========================================
    #[test]
    fn ut_005c_get_domain_connector_general() {
        let connector = get_domain_connector("general");
        assert_eq!(connector.domain_name(), "general");
    }

    // ========================================
    // UT-005d: is_feature_enabled(domain="game", feature="dicom") → false
    // ========================================
    #[test]
    fn ut_005d_game_dicom_disabled() {
        assert!(
            !is_feature_enabled("game", "dicom"),
            "Game domain should NOT have DICOM enabled"
        );
    }

    // ========================================
    // UT-005e: is_feature_enabled(domain="medical", feature="dicom") → true
    // ========================================
    #[test]
    fn ut_005e_medical_dicom_enabled() {
        assert!(
            is_feature_enabled("medical", "dicom"),
            "Medical domain SHOULD have DICOM enabled"
        );
    }

    // ========================================
    // UT-001c: validate_extension("scan.dcm") + domain=medical → Ok(())
    // ========================================
    #[test]
    fn ut_001c_dcm_allowed_for_medical() {
        let result = validate_extension_for_domain("scan.dcm", "medical");
        assert!(
            result.is_ok(),
            "DICOM files should be allowed for medical domain"
        );
    }

    // ========================================
    // UT-001d: validate_extension("scan.dcm") + domain=game → Err(UnsupportedType)
    // ========================================
    #[test]
    fn ut_001d_dcm_rejected_for_game() {
        let result = validate_extension_for_domain("scan.dcm", "game");
        assert!(
            result.is_err(),
            "DICOM files should NOT be allowed for game domain"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unsupported file type"),
            "Error should mention unsupported type, got: {}",
            err_msg
        );
    }

    // ========================================
    // Additional: get_all_features returns correct map
    // ========================================
    #[test]
    fn test_get_all_features_medical() {
        let features = get_all_features("medical");
        let dicom = features.iter().find(|(k, _)| *k == "dicom");
        assert_eq!(dicom, Some(&("dicom", true)));

        let rathena = features.iter().find(|(k, _)| *k == "rathena_connector");
        assert_eq!(rathena, Some(&("rathena_connector", false)));
    }

    #[test]
    fn test_get_all_features_game() {
        let features = get_all_features("game");
        let rathena = features.iter().find(|(k, _)| *k == "rathena_connector");
        assert_eq!(rathena, Some(&("rathena_connector", true)));

        let dicom = features.iter().find(|(k, _)| *k == "dicom");
        assert_eq!(dicom, Some(&("dicom", false)));
    }

    // ========================================
    // Additional: Unknown domain falls back to DefaultConnector
    // ========================================
    #[test]
    fn test_unknown_domain_defaults_to_general() {
        let connector = get_domain_connector("unknown_xyz");
        assert_eq!(connector.domain_name(), "general");
    }
}
