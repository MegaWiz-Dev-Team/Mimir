//! Upload service: file validation, S3 key building, and SHA-256 hashing.
//!
//! This module provides pure utility functions for the file upload pipeline.
//! Domain-aware extension validation will be refactored to use domain.rs
//! once Issue #76 is merged.

use anyhow::{Result, bail};
use sha2::{Sha256, Digest};

/// Maximum file size in bytes (50 MB)
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 52,428,800 bytes

/// Allowed file extensions (domain-agnostic baseline).
/// When Issue #76 (Domain Connector) is implemented, this will be replaced
/// with domain-specific whitelists via `get_domain_connector()`.
const ALLOWED_EXTENSIONS: &[&str] = &[
    "pdf", "csv", "xlsx", "xls", "txt", "docx", "doc",
    "json", "md", "html", "htm", "xml", "yaml", "yml",
    "png", "jpg", "jpeg", "dicom", "dcm",
];

/// Validate that a filename has an allowed extension.
///
/// # Errors
/// Returns `Err` with "Unsupported file type" if the extension is not in the whitelist.
pub fn validate_extension(filename: &str) -> Result<()> {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

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
        assert_ne!(hash1, hash2, "Different data should produce different hashes");
    }
}
