//! Upload service: file validation, S3 key building, and SHA-256 hashing.
//!
//! This module provides pure utility functions for the file upload pipeline.
//! Domain-aware extension validation will be refactored to use domain.rs
//! once Issue #76 is merged.

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};

/// Maximum file size in bytes (50 MB)
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 52,428,800 bytes

/// Allowed file extensions (domain-agnostic baseline).
/// When Issue #76 (Domain Connector) is implemented, this will be replaced
/// with domain-specific whitelists via `get_domain_connector()`.
const ALLOWED_EXTENSIONS: &[&str] = &[
    "pdf", "csv", "xlsx", "xls", "txt", "docx", "doc", "pptx", "ppt", "json", "md", "html", "htm",
    "xml", "yaml", "yml", "png", "jpg", "jpeg", "dicom", "dcm",
];

/// Validate that a filename has an allowed extension.
///
/// # Errors
/// Returns `Err` with "Unsupported file type" if the extension is not in the whitelist.
pub fn validate_extension(filename: &str) -> Result<()> {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    if ext.is_empty() || !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        bail!("Unsupported file type: .{}", ext);
    }
    Ok(())
}

/// Validate that the file size does not exceed the maximum limit (50 MB).
///
/// # Errors
/// Returns `Err` with "Payload too large" if size exceeds `MAX_FILE_SIZE`.
pub fn validate_file_size(size_bytes: u64) -> Result<()> {
    if size_bytes > MAX_FILE_SIZE {
        bail!(
            "Payload too large: {} bytes exceeds maximum of {} bytes",
            size_bytes,
            MAX_FILE_SIZE
        );
    }
    Ok(())
}

/// Build the S3 object key for a file upload.
///
/// Format: `{tenant_id}/{source_id}/{folder_path}/{filename}`
/// When `folder_path` is empty, the folder segment is omitted.
pub fn build_s3_key(tenant_id: &str, source_id: &str, folder_path: &str, filename: &str) -> String {
    if folder_path.is_empty() {
        format!("{}/{}/{}", tenant_id, source_id, filename)
    } else {
        format!("{}/{}/{}/{}", tenant_id, source_id, folder_path, filename)
    }
}

/// Compute the SHA-256 hash of file data, returned as a lowercase hex string.
pub fn compute_file_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Auto-detect source_type from file extension.
///
/// Returns one of: "document", "tabular", "structured", "image".
/// Unknown extensions default to "document".
///
/// This replaces the manual source_type selection by users, reducing
/// cognitive load and preventing type mismatch errors.
pub fn detect_source_type(filename: &str) -> &str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    match ext.as_str() {
        "pdf" | "docx" | "doc" | "pptx" | "ppt" | "txt" | "md" | "html" | "htm" => "document",
        "csv" | "xlsx" | "xls" => "tabular",
        "json" | "yaml" | "yml" | "xml" => "structured",
        "png" | "jpg" | "jpeg" | "dicom" | "dcm" => "image",
        _ => "document",
    }
}

/// Check if source_type needs a storage_mode prompt (CSV/XLSX only).
///
/// Returns `true` for tabular files that can be stored as either
/// Markdown tables or SQL dynamic tables.
pub fn needs_storage_mode_prompt(filename: &str) -> bool {
    detect_source_type(filename) == "tabular"
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-001a: validate_extension("report.pdf") → Ok(())
    // ========================================
    #[test]
    fn test_validate_extension_pdf_ok() {
        let result = validate_extension("report.pdf");
        assert!(result.is_ok(), "PDF should be allowed");
    }

    // ========================================
    // UT-001b: validate_extension("virus.exe") → Err(UnsupportedType)
    // ========================================
    #[test]
    fn test_validate_extension_exe_rejected() {
        let result = validate_extension("virus.exe");
        assert!(result.is_err(), ".exe should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unsupported file type"),
            "Error should mention unsupported type, got: {}",
            err_msg
        );
    }

    // ========================================
    // UT-001e: validate_file_size(10MB) → Ok(())
    // ========================================
    #[test]
    fn test_validate_file_size_10mb_ok() {
        let size = 10 * 1024 * 1024; // 10 MB
        let result = validate_file_size(size);
        assert!(result.is_ok(), "10MB should be within limit");
    }

    // ========================================
    // UT-001f: validate_file_size(60MB) → Err(PayloadTooLarge)
    // ========================================
    #[test]
    fn test_validate_file_size_60mb_rejected() {
        let size = 60 * 1024 * 1024; // 60 MB
        let result = validate_file_size(size);
        assert!(result.is_err(), "60MB should exceed limit");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Payload too large"),
            "Error should mention payload too large, got: {}",
            err_msg
        );
    }

    // ========================================
    // UT-002a: build_s3_key with folder path
    // ========================================
    #[test]
    fn test_build_s3_key_with_folder() {
        let key = build_s3_key("1", "5", "finance", "q1.pdf");
        assert_eq!(key, "1/5/finance/q1.pdf");
    }

    // ========================================
    // UT-002b: build_s3_key without folder path
    // ========================================
    #[test]
    fn test_build_s3_key_without_folder() {
        let key = build_s3_key("1", "5", "", "report.csv");
        assert_eq!(key, "1/5/report.csv");
    }

    // ========================================
    // UT-002c: compute_file_hash — same file twice → same SHA-256
    // ========================================
    #[test]
    fn test_compute_file_hash_same_data() {
        let data = b"Hello, this is a test file content for hashing.";
        let hash1 = compute_file_hash(data);
        let hash2 = compute_file_hash(data);
        assert_eq!(hash1, hash2, "Same data should produce identical hashes");
        assert_eq!(hash1.len(), 64, "SHA-256 hex should be 64 characters");
    }

    // ========================================
    // UT-002d: compute_file_hash — different files → different SHA-256
    // ========================================
    #[test]
    fn test_compute_file_hash_different_data() {
        let data1 = b"File content version A";
        let data2 = b"File content version B";
        let hash1 = compute_file_hash(data1);
        let hash2 = compute_file_hash(data2);
        assert_ne!(
            hash1, hash2,
            "Different data should produce different hashes"
        );
    }

    // ========================================
    // UT-087a: detect_source_type("report.pdf") → "document"
    // ========================================
    #[test]
    fn test_detect_pdf_as_document() {
        assert_eq!(detect_source_type("report.pdf"), "document");
        assert_eq!(detect_source_type("notes.docx"), "document");
        assert_eq!(detect_source_type("readme.md"), "document");
        assert_eq!(detect_source_type("page.html"), "document");
        assert_eq!(detect_source_type("file.txt"), "document");
    }

    // ========================================
    // UT-087b: detect_source_type("data.csv") → "tabular"
    // ========================================
    #[test]
    fn test_detect_csv_xlsx_as_tabular() {
        assert_eq!(detect_source_type("data.csv"), "tabular");
        assert_eq!(detect_source_type("sheet.xlsx"), "tabular");
        assert_eq!(detect_source_type("legacy.xls"), "tabular");
    }

    // ========================================
    // UT-087c: detect_source_type("config.json") → "structured"
    // ========================================
    #[test]
    fn test_detect_json_yaml_as_structured() {
        assert_eq!(detect_source_type("config.json"), "structured");
        assert_eq!(detect_source_type("settings.yaml"), "structured");
        assert_eq!(detect_source_type("data.xml"), "structured");
    }

    // ========================================
    // UT-087d: detect_source_type("photo.png") → "image"
    // ========================================
    #[test]
    fn test_detect_image_types() {
        assert_eq!(detect_source_type("photo.png"), "image");
        assert_eq!(detect_source_type("scan.jpg"), "image");
        assert_eq!(detect_source_type("xray.dicom"), "image");
    }

    // ========================================
    // UT-087e: detect_source_type("mystery.exe") → "document" (default)
    // ========================================
    #[test]
    fn test_detect_unknown_defaults_document() {
        assert_eq!(detect_source_type("mystery.exe"), "document");
        assert_eq!(detect_source_type("noext"), "document");
    }

    // ========================================
    // UT-087f: detect_source_type is case-insensitive
    // ========================================
    #[test]
    fn test_detect_case_insensitive() {
        assert_eq!(detect_source_type("FILE.PDF"), "document");
        assert_eq!(detect_source_type("DATA.CSV"), "tabular");
        assert_eq!(detect_source_type("IMAGE.JPEG"), "image");
    }

    // ========================================
    // UT-087g: needs_storage_mode_prompt → true for CSV/XLSX
    // ========================================
    #[test]
    fn test_needs_storage_mode_prompt_tabular() {
        assert!(needs_storage_mode_prompt("data.csv"));
        assert!(needs_storage_mode_prompt("sheet.xlsx"));
    }

    // ========================================
    // UT-087h: needs_storage_mode_prompt → false for PDF/JSON
    // ========================================
    #[test]
    fn test_needs_storage_mode_prompt_non_tabular() {
        assert!(!needs_storage_mode_prompt("report.pdf"));
        assert!(!needs_storage_mode_prompt("config.json"));
        assert!(!needs_storage_mode_prompt("photo.png"));
    }
}
