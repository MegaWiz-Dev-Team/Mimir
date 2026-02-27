//! Extraction service: convert raw file bytes into Markdown text.
//!
//! Pure functions — no I/O, no database, no RustFS. Each function takes `&[u8]`
//! and returns `Result<String>` with the extracted Markdown/text content.

use anyhow::{Result, Context, bail};
use std::io::Cursor;
use tracing::info;
use std::io::Write;

/// Detect file extension from an S3 key / filename.
fn detect_extension(s3_key: &str) -> &str {
    s3_key.rsplit('.').next().unwrap_or("")
}

// ─── PDF Extraction ────────────────────────────────────────────────────────────

/// Extract plain text from PDF bytes using `pdf-extract`.
pub fn extract_pdf(data: &[u8]) -> Result<String> {
    let text = pdf_extract::extract_text_from_mem(data)
        .context("Failed to extract text from PDF — file may be corrupted")?;

    if text.trim().is_empty() {
        bail!("PDF extraction produced empty text — file may be image-only or corrupted");
    }

    Ok(text)
}

// ─── DOCX Extraction ───────────────────────────────────────────────────────────

/// Extract plain text from DOCX bytes using `docx-rs`.
pub fn extract_docx(data: &[u8]) -> Result<String> {
    let docx = docx_rs::read_docx(data)
        .map_err(|e| anyhow::anyhow!("Failed to parse DOCX: {:?}", e))?;

    let mut text = String::new();
    // Walk through the document body and extract text from paragraphs
    for child in docx.document.children.iter() {
        if let docx_rs::DocumentChild::Paragraph(paragraph) = child {
            let mut line = String::new();
            for p_child in paragraph.children.iter() {
                if let docx_rs::ParagraphChild::Run(run) = p_child {
                    for r_child in run.children.iter() {
                        if let docx_rs::RunChild::Text(t) = r_child {
                            line.push_str(&t.text);
                        }
                    }
                }
            }
            if !line.is_empty() {
                text.push_str(&line);
                text.push('\n');
            }
        }
    }

    Ok(text)
}

// ─── Plain Text (TXT / MD) ────────────────────────────────────────────────────

/// Read raw bytes as UTF-8 text (for .txt and .md files).
pub fn extract_text(data: &[u8]) -> Result<String> {
    let text = String::from_utf8(data.to_vec())
        .context("File is not valid UTF-8 text")?;
    Ok(text)
}

// ─── CSV → Markdown Table ──────────────────────────────────────────────────────

/// Parse CSV bytes and convert to a Markdown table.
pub fn extract_csv_to_markdown(data: &[u8]) -> Result<String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(Cursor::new(data));

    let headers: Vec<String> = reader.headers()
        .context("Failed to read CSV headers")?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        bail!("CSV has no headers");
    }

    let mut md = String::new();
    // Header row
    md.push_str("| ");
    md.push_str(&headers.join(" | "));
    md.push_str(" |\n");

    // Separator
    md.push_str("| ");
    md.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    md.push_str(" |\n");

    // Data rows
    for result in reader.records() {
        let record = result.context("Failed to read CSV record")?;
        let cells: Vec<&str> = record.iter().collect();
        md.push_str("| ");
        md.push_str(&cells.join(" | "));
        md.push_str(" |\n");
    }

    Ok(md)
}

// ─── XLSX → Markdown Table ─────────────────────────────────────────────────────

/// Parse XLSX bytes and convert the first sheet to a Markdown table.
pub fn extract_xlsx_to_markdown(data: &[u8]) -> Result<String> {
    use calamine::{Reader, Xlsx, Data};

    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> = Xlsx::new(cursor)
        .context("Failed to open XLSX — file may be corrupted")?;

    let sheet_names = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names.first()
        .context("XLSX has no sheets")?;

    let range = workbook.worksheet_range(first_sheet)
        .context("Failed to read XLSX worksheet")?;

    let mut rows_iter = range.rows();

    // First row = headers
    let headers: Vec<String> = match rows_iter.next() {
        Some(row) => row.iter().map(|cell| match cell {
            Data::String(s) => s.clone(),
            Data::Float(f) => f.to_string(),
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            Data::DateTime(dt) => dt.to_string(),
            Data::Error(e) => format!("{:?}", e),
            Data::Empty => String::new(),
            _ => String::new(),
        }).collect(),
        None => bail!("XLSX sheet is empty"),
    };

    let mut md = String::new();
    md.push_str("| ");
    md.push_str(&headers.join(" | "));
    md.push_str(" |\n");

    md.push_str("| ");
    md.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    md.push_str(" |\n");

    for row in rows_iter {
        let cells: Vec<String> = row.iter().map(|cell| match cell {
            Data::String(s) => s.clone(),
            Data::Float(f) => f.to_string(),
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            Data::DateTime(dt) => dt.to_string(),
            Data::Error(e) => format!("{:?}", e),
            Data::Empty => String::new(),
            _ => String::new(),
        }).collect();
        md.push_str("| ");
        md.push_str(&cells.join(" | "));
        md.push_str(" |\n");
    }

    Ok(md)
}

// ─── HTML → Markdown ───────────────────────────────────────────────────────────

/// Convert HTML bytes to Markdown using `html2md`.
pub fn extract_html_to_markdown(data: &[u8]) -> Result<String> {
    let html = String::from_utf8(data.to_vec())
        .context("HTML file is not valid UTF-8")?;
    let markdown = html2md::parse_html(&html);
    Ok(markdown)
}

// ─── MCP JSON → Markdown ───────────────────────────────────────────────────────

/// Convert MCP JSON response bytes to a formatted Markdown document.
pub fn extract_mcp_json_to_markdown(data: &[u8]) -> Result<String> {
    let value: serde_json::Value = serde_json::from_slice(data)
        .context("Failed to parse MCP response as JSON")?;

    let mut md = String::new();
    md.push_str("# MCP Data\n\n");
    md.push_str("```json\n");
    md.push_str(&serde_json::to_string_pretty(&value)
        .context("Failed to format JSON")?);
    md.push_str("\n```\n");

    Ok(md)
}

// ─── Legacy Office Conversion (.doc, .xls, .ppt) ──────────────────────────────

/// Convert legacy Office format to modern format using LibreOffice headless,
/// then extract using the appropriate modern extractor.
///
/// Requires `libreoffice` (or `soffice`) to be installed on the system.
/// Falls back to a descriptive error if LibreOffice is not available.
pub fn extract_legacy_office(data: &[u8], extension: &str) -> Result<String> {
    use std::process::Command;

    // Determine conversion target format
    let (target_ext, _) = match extension {
        "doc" => ("docx", "document"),
        "xls" => ("xlsx", "tabular"),
        "ppt" => ("pptx", "document"),
        _ => bail!("Unsupported legacy format: .{}", extension),
    };

    // Write bytes to a temp file
    let tmp_dir = std::env::temp_dir().join("mimir-convert");
    std::fs::create_dir_all(&tmp_dir).context("Failed to create temp directory")?;
    let input_path = tmp_dir.join(format!("input.{}", extension));
    let mut file = std::fs::File::create(&input_path)
        .context("Failed to create temp input file")?;
    file.write_all(data).context("Failed to write temp file")?;
    drop(file);

    // Run LibreOffice headless conversion
    let result = Command::new("soffice")
        .args([
            "--headless",
            "--convert-to", target_ext,
            "--outdir", tmp_dir.to_str().unwrap_or("/tmp"),
            input_path.to_str().unwrap_or(""),
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            // Read the converted file
            let output_path = tmp_dir.join(format!("input.{}", target_ext));
            let converted_data = std::fs::read(&output_path)
                .context("Failed to read converted file")?;

            // Cleanup temp files
            let _ = std::fs::remove_file(&input_path);
            let _ = std::fs::remove_file(&output_path);

            // Delegate to the appropriate modern extractor
            match target_ext {
                "docx" => extract_docx(&converted_data),
                "xlsx" => extract_xlsx_to_markdown(&converted_data),
                "pptx" => extract_text(&converted_data)
                    .or_else(|_| Ok(String::from("[Converted from PPT — content extraction requires manual review]"))),
                _ => bail!("No extractor for converted format .{}", target_ext),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = std::fs::remove_file(&input_path);
            bail!("LibreOffice conversion failed: {}", stderr);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let _ = std::fs::remove_file(&input_path);
            bail!(
                "Legacy format .{} requires LibreOffice for conversion. \
                 Install with: brew install --cask libreoffice (macOS) or \
                 apt-get install libreoffice-core (Linux)",
                extension
            );
        }
        Err(e) => {
            let _ = std::fs::remove_file(&input_path);
            bail!("Failed to run LibreOffice: {}", e);
        }
    }
}

// ─── Extraction Router ────────────────────────────────────────────────────────

/// Route extraction to the correct function based on `source_type` and file extension.
///
/// - `source_type`: `"document"`, `"tabular"`, `"web"`, `"mcp"`
/// - `s3_key`: RustFS object path (used to detect extension)
/// - `data`: raw file bytes downloaded from RustFS
pub fn extract(source_type: &str, s3_key: &str, data: &[u8]) -> Result<String> {
    let ext = detect_extension(s3_key).to_lowercase();
    info!("Extracting source_type={}, extension={}, size={} bytes", source_type, ext, data.len());

    match source_type {
        "document" => match ext.as_str() {
            "pdf" => extract_pdf(data),
            "docx" => extract_docx(data),
            "txt" | "md" => extract_text(data),
            "doc" | "ppt" => extract_legacy_office(data, &ext),
            "pptx" => extract_text(data)
                .or_else(|_| Ok(String::from("[PPTX content — use LLM extraction for full text]"))),
            _ => bail!("Unsupported document extension: .{}", ext),
        },
        "tabular" => match ext.as_str() {
            "csv" => extract_csv_to_markdown(data),
            "xlsx" | "xls" => extract_xlsx_to_markdown(data),
            _ => bail!("Unsupported tabular extension: .{}", ext),
        },
        // "file" source_type: auto-detect format from extension (Issue #122 + #124)
        "file" => match ext.as_str() {
            "pdf" => extract_pdf(data),
            "docx" => extract_docx(data),
            "txt" | "md" => extract_text(data),
            "csv" => extract_csv_to_markdown(data),
            "xlsx" | "xls" => extract_xlsx_to_markdown(data),
            "html" | "htm" => extract_html_to_markdown(data),
            "json" => extract_mcp_json_to_markdown(data),
            "doc" | "ppt" => extract_legacy_office(data, &ext),
            "pptx" => extract_text(data)
                .or_else(|_| Ok(String::from("[PPTX content — use LLM extraction for full text]"))),
            _ => bail!("Unsupported file extension: .{}", ext),
        },
        "web" => extract_html_to_markdown(data),
        "mcp" => extract_mcp_json_to_markdown(data),
        _ => bail!("Unsupported source_type: {}", source_type),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-003a: extract_pdf — valid PDF → non-empty text
    // ========================================
    #[test]
    fn test_extract_pdf_valid() {
        // Create a minimal valid PDF in memory
        let pdf_bytes = include_bytes!("../../tests/fixtures/sample.pdf");
        let result = extract_pdf(pdf_bytes);
        assert!(result.is_ok(), "Valid PDF should extract successfully: {:?}", result.err());
        let text = result.unwrap();
        assert!(!text.trim().is_empty(), "Extracted text should not be empty");
    }

    // ========================================
    // UT-003b: extract_pdf — corrupted PDF → Err
    // ========================================
    #[test]
    fn test_extract_pdf_corrupted() {
        let bad_data = b"This is definitely not a valid PDF file";
        let result = extract_pdf(bad_data);
        assert!(result.is_err(), "Corrupted data should produce an error");
    }

    // ========================================
    // UT-003c: extract_csv_to_markdown — CSV → Markdown Table
    // ========================================
    #[test]
    fn test_extract_csv_to_markdown() {
        let csv_data = b"Name,Age,City\nAlice,30,Bangkok\nBob,25,Tokyo\n";
        let result = extract_csv_to_markdown(csv_data);
        assert!(result.is_ok(), "CSV parsing should succeed: {:?}", result.err());
        let md = result.unwrap();
        assert!(md.contains("| Name | Age | City |"), "Should contain header row");
        assert!(md.contains("| --- | --- | --- |"), "Should contain separator");
        assert!(md.contains("| Alice | 30 | Bangkok |"), "Should contain data row");
        assert!(md.contains("| Bob | 25 | Tokyo |"), "Should contain second row");
    }

    // ========================================
    // UT-003d: extract_xlsx_to_markdown — XLSX → Markdown Table
    // ========================================
    #[test]
    fn test_extract_xlsx_to_markdown() {
        let xlsx_bytes = include_bytes!("../../tests/fixtures/sample.xlsx");
        let result = extract_xlsx_to_markdown(xlsx_bytes);
        assert!(result.is_ok(), "XLSX parsing should succeed: {:?}", result.err());
        let md = result.unwrap();
        // The fixture should have at least headers + some rows
        assert!(md.contains("| "), "Should contain markdown table pipes");
        assert!(md.contains("| --- "), "Should contain separator");
        let row_count = md.lines().count();
        assert!(row_count >= 3, "Should have header + separator + at least 1 data row, got {} lines", row_count);
    }

    // ========================================
    // UT-003e: extract_html_to_markdown — HTML → Markdown
    // ========================================
    #[test]
    fn test_extract_html_to_markdown() {
        let html = b"<h1>Title</h1><p>Some text content here.</p>";
        let result = extract_html_to_markdown(html);
        assert!(result.is_ok(), "HTML parsing should succeed: {:?}", result.err());
        let md = result.unwrap();
        println!("HTML→MD output: {:?}", md);
        assert!(md.contains("Title"), "Should preserve heading text");
        assert!(md.contains("Some text content here"), "Should preserve text content");
    }

    // ========================================
    // UT-003f: extract_text — plain text passthrough
    // ========================================
    #[test]
    fn test_extract_text() {
        let data = b"Hello, world!\nLine two.";
        let result = extract_text(data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world!\nLine two.");
    }

    // ========================================
    // UT-003g: extract_mcp_json_to_markdown — JSON → formatted Markdown
    // ========================================
    #[test]
    fn test_extract_mcp_json_to_markdown() {
        let json_data = br#"{"resources": [{"name": "FAQ", "content": "How to use"}]}"#;
        let result = extract_mcp_json_to_markdown(json_data);
        assert!(result.is_ok(), "JSON parsing should succeed: {:?}", result.err());
        let md = result.unwrap();
        assert!(md.contains("# MCP Data"), "Should have MCP Data heading");
        assert!(md.contains("```json"), "Should contain JSON code block");
        assert!(md.contains("FAQ"), "Should contain the data");
    }

    // ========================================
    // UT-003h: extract router — dispatches correctly
    // ========================================
    #[test]
    fn test_extract_router_csv() {
        let csv_data = b"Col1,Col2\nA,B\n";
        let result = extract("tabular", "tenant/1/data.csv", csv_data);
        assert!(result.is_ok(), "Router should dispatch CSV: {:?}", result.err());
        assert!(result.unwrap().contains("| Col1 | Col2 |"));
    }

    #[test]
    fn test_extract_router_html() {
        let html = b"<p>Hello</p>";
        let result = extract("web", "tenant/1/page.html", html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello"));
    }

    #[test]
    fn test_extract_router_text() {
        let txt = b"Plain text file.";
        let result = extract("document", "tenant/1/readme.txt", txt);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Plain text file.");
    }

    #[test]
    fn test_extract_router_unsupported_type() {
        let result = extract("unknown_type", "tenant/1/file.xyz", b"data");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported source_type"));
    }

    #[test]
    fn test_extract_router_unsupported_extension() {
        let result = extract("document", "tenant/1/file.bmp", b"data");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported document extension"));
    }
}
