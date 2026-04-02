//! OCR service: Extract text from images and scanned documents using Gemini 2.5 Flash vision.
//!
//! Uses Gemini's OpenAI-compatible vision API to perform OCR on:
//! - Images (JPEG, PNG, GIF, WebP, BMP, TIFF)
//! - Scanned PDFs (when text extraction returns empty/minimal text)
//!
//! Pure service layer — no HTTP, no DB. Callers provide bytes + config.

use anyhow::{Result, bail};
use base64::Engine;
use tracing::info;

// ─── MIME Type Detection ───────────────────────────────────────────────────────

/// Detect MIME type from file extension.
pub fn detect_mime_type(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tiff" | "tif" => Some("image/tiff"),
        "pdf" => Some("application/pdf"),
        _ => None,
    }
}

/// Check if a file extension is an image type that supports OCR.
pub fn is_ocr_capable(filename: &str) -> bool {
    detect_mime_type(filename).is_some()
}

/// Check if file is a pure image (not PDF).
pub fn is_image_file(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif"
    )
}

// ─── Gemini Vision Request Builder ─────────────────────────────────────────────

/// Build a Gemini-compatible OpenAI vision API request body.
///
/// Uses the `chat/completions` endpoint with image_url content parts.
/// The image is inlined as base64 data URI.
pub fn build_vision_request(
    data: &[u8],
    mime_type: &str,
    model: &str,
    prompt: &str,
) -> serde_json::Value {
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    let data_uri = format!("data:{};base64,{}", mime_type, b64);

    serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are an OCR extraction assistant. Extract ALL text visible in the image accurately and completely. Preserve the original structure including headings, paragraphs, tables, and lists. Output as clean Markdown. Do not add commentary or explanation — only output the extracted text content."
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": data_uri
                        }
                    }
                ]
            }
        ],
        "max_tokens": 16000,
        "temperature": 0.1
    })
}

/// Call Gemini 2.5 Flash vision API to extract text from an image.
///
/// Returns `(extracted_text, tokens_used)`.
pub async fn extract_text_from_image(
    data: &[u8],
    filename: &str,
    api_key: &str,
    api_base: &str,
    model: &str,
) -> Result<(String, u32)> {
    let mime_type = detect_mime_type(filename)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file type for OCR: {}", filename))?;

    if data.is_empty() {
        bail!("Empty file data — nothing to OCR");
    }

    // Size guard: 20MB max for vision API
    if data.len() > 20 * 1024 * 1024 {
        bail!("File too large for OCR: {} bytes (max 20MB)", data.len());
    }

    info!(
        "OCR: Sending {} ({}, {} bytes) to Gemini vision (model={})",
        filename,
        mime_type,
        data.len(),
        model
    );

    let prompt = if mime_type == "application/pdf" {
        "Extract ALL text from this scanned PDF document. Preserve headings, paragraphs, tables, and lists. Output as clean Markdown."
    } else {
        "Extract ALL text visible in this image. Preserve the original layout and structure. Output as clean Markdown."
    };

    let body = build_vision_request(data, mime_type, model, prompt);
    let url = format!("{}chat/completions", api_base);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Gemini vision API request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        bail!("Gemini vision API returned {}: {}", status, error_body);
    }

    let resp_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse Gemini vision response: {}", e))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let total_tokens = resp_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32;

    info!(
        "OCR complete for {}: {} chars extracted, {} tokens used",
        filename,
        content.len(),
        total_tokens
    );

    Ok((content, total_tokens))
}

/// Check if a PDF appears to be scanned (image-only) by checking if text extraction
/// returned very little content relative to file size.
pub fn is_likely_scanned_pdf(text_content: &str, file_size: usize) -> bool {
    let text_len = text_content.trim().len();

    // Heuristic: if text extraction returned < 50 chars from a file > 10KB,
    // it's likely a scanned/image PDF.
    if file_size > 10_000 && text_len < 50 {
        return true;
    }

    // Also flag if text-to-size ratio is extremely low (< 0.1%)
    if file_size > 0 {
        let ratio = text_len as f64 / file_size as f64;
        if ratio < 0.001 && file_size > 5_000 {
            return true;
        }
    }

    false
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // UT-014f: MIME type detection
    #[test]
    fn test_detect_mime_type_images() {
        assert_eq!(detect_mime_type("photo.jpg"), Some("image/jpeg"));
        assert_eq!(detect_mime_type("photo.jpeg"), Some("image/jpeg"));
        assert_eq!(detect_mime_type("image.png"), Some("image/png"));
        assert_eq!(detect_mime_type("anim.gif"), Some("image/gif"));
        assert_eq!(detect_mime_type("photo.webp"), Some("image/webp"));
        assert_eq!(detect_mime_type("scan.tiff"), Some("image/tiff"));
        assert_eq!(detect_mime_type("scan.bmp"), Some("image/bmp"));
    }

    #[test]
    fn test_detect_mime_type_pdf() {
        assert_eq!(detect_mime_type("document.pdf"), Some("application/pdf"));
    }

    #[test]
    fn test_detect_mime_type_unsupported() {
        assert_eq!(detect_mime_type("file.txt"), None);
        assert_eq!(detect_mime_type("data.csv"), None);
        assert_eq!(detect_mime_type("doc.docx"), None);
    }

    // UT-014g: OCR capability check
    #[test]
    fn test_is_ocr_capable() {
        assert!(is_ocr_capable("scan.png"));
        assert!(is_ocr_capable("receipt.jpg"));
        assert!(is_ocr_capable("document.pdf"));
        assert!(!is_ocr_capable("data.csv"));
        assert!(!is_ocr_capable("readme.md"));
    }

    // UT-014h: Image file detection
    #[test]
    fn test_is_image_file() {
        assert!(is_image_file("photo.jpg"));
        assert!(is_image_file("image.png"));
        assert!(!is_image_file("document.pdf"));
        assert!(!is_image_file("data.csv"));
    }

    // UT-014i: Vision request builder
    #[test]
    fn test_build_vision_request_structure() {
        let data = b"fake image data";
        let body = build_vision_request(data, "image/png", "gemini-2.5-flash", "Extract text");

        assert_eq!(body["model"], "gemini-2.5-flash");
        assert_eq!(body["temperature"], 0.1);
        assert_eq!(body["max_tokens"], 16000);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);

        // System message
        assert_eq!(messages[0]["role"], "system");

        // User message with text + image_url
        let content = messages[1]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");

        // Verify base64 data URI
        let image_url = content[1]["image_url"]["url"].as_str().unwrap();
        assert!(image_url.starts_with("data:image/png;base64,"));
    }

    // UT-014j: Scanned PDF detection heuristic
    #[test]
    fn test_is_likely_scanned_pdf() {
        // Large file with almost no text = scanned
        assert!(is_likely_scanned_pdf("", 50_000));
        assert!(is_likely_scanned_pdf("   ", 100_000));
        assert!(is_likely_scanned_pdf("a few words", 500_000));

        // Normal file with good text = not scanned
        assert!(!is_likely_scanned_pdf(
            "This is a normal PDF with lots of readable text content spread across multiple lines.",
            50_000
        ));

        // Small file = not scanned (could be a simple doc)
        assert!(!is_likely_scanned_pdf("", 500));
    }
}
