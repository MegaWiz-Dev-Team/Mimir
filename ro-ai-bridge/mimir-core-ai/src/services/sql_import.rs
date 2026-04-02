//! SQL Import service: convert tabular data (CSV/XLSX) into SQL DDL + INSERT
//! statements for Dynamic Tables in MariaDB.
//!
//! Pure functions — no I/O, no database connections. Each function produces
//! SQL strings that can be executed by the caller.

use anyhow::{Result, bail};
use regex::Regex;
use std::io::Cursor;
use tracing::info;

// ─── Column Type Detection ─────────────────────────────────────────────────────

/// Supported SQL column types for auto-detection.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlColumnType {
    Decimal,
    Varchar255,
    Date,
}

impl std::fmt::Display for SqlColumnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlColumnType::Decimal => write!(f, "DECIMAL"),
            SqlColumnType::Varchar255 => write!(f, "VARCHAR(255)"),
            SqlColumnType::Date => write!(f, "DATE"),
        }
    }
}

/// Check if a string value looks like a number (integer or decimal).
fn is_numeric(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.parse::<f64>().is_ok()
}

/// Check if a string value looks like a date (YYYY-MM-DD format).
fn is_date(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Match YYYY-MM-DD pattern
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    re.is_match(trimmed)
}

/// Auto-detect SQL column type from sample values.
///
/// Rules:
/// - All numeric → DECIMAL
/// - All date (YYYY-MM-DD) → DATE
/// - Mixed or text → VARCHAR(255)
/// - Empty samples → VARCHAR(255)
pub fn detect_column_type(samples: &[&str]) -> SqlColumnType {
    if samples.is_empty() {
        return SqlColumnType::Varchar255;
    }

    // Filter out empty strings for type detection
    let non_empty: Vec<&str> = samples
        .iter()
        .filter(|s| !s.trim().is_empty())
        .copied()
        .collect();

    if non_empty.is_empty() {
        return SqlColumnType::Varchar255;
    }

    // Check if all non-empty values are numeric
    if non_empty.iter().all(|s| is_numeric(s)) {
        return SqlColumnType::Decimal;
    }

    // Check if all non-empty values are dates
    if non_empty.iter().all(|s| is_date(s)) {
        return SqlColumnType::Date;
    }

    // Fallback: mixed or text
    SqlColumnType::Varchar255
}

// ─── Table Name Sanitization ───────────────────────────────────────────────────

/// Sanitize a tenant_id to only contain safe characters [a-zA-Z0-9_].
fn sanitize_identifier(input: &str) -> Result<String> {
    let re = Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
    if !re.is_match(input) {
        bail!(
            "Identifier contains unsafe characters: '{}'. Only [a-zA-Z0-9_] allowed.",
            input
        );
    }
    Ok(input.to_string())
}

/// Generate a safe dynamic table name: `tenant_{tenant_id}_src_{source_id}`.
///
/// Both tenant_id and source_id are validated against a whitelist regex.
pub fn sanitize_table_name(tenant_id: &str, source_id: i64) -> Result<String> {
    let safe_tenant = sanitize_identifier(tenant_id)?;
    Ok(format!("tenant_{}_src_{}", safe_tenant, source_id))
}

// ─── DDL Generation ────────────────────────────────────────────────────────────

/// Sanitize a column header name for use in SQL.
/// Replaces non-alphanumeric characters with underscores and lowercases.
fn sanitize_column_name(header: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9_]").unwrap();
    let sanitized = re.replace_all(header.trim(), "_").to_string();
    let sanitized = sanitized.trim_matches('_').to_lowercase();
    if sanitized.is_empty() {
        "col".to_string()
    } else {
        sanitized
    }
}

/// Generate a CREATE TABLE DDL statement for a dynamic table.
///
/// Includes:
/// - `id BIGINT AUTO_INCREMENT PRIMARY KEY`
/// - One column per header with auto-detected type
/// - `created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP`
pub fn generate_create_table(
    table_name: &str,
    headers: &[String],
    types: &[SqlColumnType],
) -> String {
    let mut columns = vec!["id BIGINT AUTO_INCREMENT PRIMARY KEY".to_string()];

    for (header, col_type) in headers.iter().zip(types.iter()) {
        let col_name = sanitize_column_name(header);
        columns.push(format!("{} {}", col_name, col_type));
    }

    columns.push("created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string());

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n);",
        table_name,
        columns.join(",\n    ")
    )
}

// ─── Batch INSERT Generation ───────────────────────────────────────────────────

/// Generate batch INSERT SQL statements with placeholder `?` values.
///
/// Returns a vector of (sql_template, rows_for_this_batch) tuples.
/// Each batch contains at most `batch_size` rows.
///
/// Example output SQL:
/// `INSERT INTO table_name (col1, col2) VALUES (?, ?), (?, ?), (?, ?);`
pub fn generate_batch_inserts(
    table_name: &str,
    headers: &[String],
    rows: &[Vec<String>],
    batch_size: usize,
) -> Vec<(String, Vec<Vec<String>>)> {
    let col_names: Vec<String> = headers.iter().map(|h| sanitize_column_name(h)).collect();

    let col_list = col_names.join(", ");
    let placeholder_row = format!("({})", vec!["?"; headers.len()].join(", "));

    let mut batches = Vec::new();

    for chunk in rows.chunks(batch_size) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| placeholder_row.as_str()).collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES {};",
            table_name,
            col_list,
            placeholders.join(", ")
        );
        let batch_rows: Vec<Vec<String>> = chunk.to_vec();
        batches.push((sql, batch_rows));
    }

    batches
}

// ─── CSV Parsing for SQL Mode ──────────────────────────────────────────────────

/// Parse CSV bytes and return (headers, detected_types, rows) for SQL import.
///
/// Reads all rows, samples up to 100 rows to detect column types.
pub fn parse_csv_for_sql(
    data: &[u8],
) -> Result<(Vec<String>, Vec<SqlColumnType>, Vec<Vec<String>>)> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(Cursor::new(data));

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| anyhow::anyhow!("Failed to read CSV headers: {}", e))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        bail!("CSV has no headers");
    }

    let mut all_rows: Vec<Vec<String>> = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| anyhow::anyhow!("Failed to read CSV record: {}", e))?;
        let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        all_rows.push(row);
    }

    // Detect column types from sample rows (up to 100)
    let sample_count = all_rows.len().min(100);
    let mut types = Vec::new();

    for col_idx in 0..headers.len() {
        let samples: Vec<&str> = all_rows
            .iter()
            .take(sample_count)
            .filter_map(|row| row.get(col_idx).map(|s| s.as_str()))
            .collect();
        types.push(detect_column_type(&samples));
    }

    info!(
        "Parsed CSV: {} headers, {} rows, types: {:?}",
        headers.len(),
        all_rows.len(),
        types
    );

    Ok((headers, types, all_rows))
}

// ─── XLSX Parsing for SQL Mode ─────────────────────────────────────────────────

/// Parse XLSX bytes and return (headers, detected_types, rows) for SQL import.
///
/// Reads the first sheet only.
pub fn parse_xlsx_for_sql(
    data: &[u8],
) -> Result<(Vec<String>, Vec<SqlColumnType>, Vec<Vec<String>>)> {
    use calamine::{Data, Reader, Xlsx};

    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> =
        Xlsx::new(cursor).map_err(|e| anyhow::anyhow!("Failed to open XLSX: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names
        .first()
        .ok_or_else(|| anyhow::anyhow!("XLSX has no sheets"))?;

    let range = workbook
        .worksheet_range(first_sheet)
        .map_err(|e| anyhow::anyhow!("Failed to read XLSX worksheet: {}", e))?;

    let mut rows_iter = range.rows();

    // First row = headers
    let headers: Vec<String> = match rows_iter.next() {
        Some(row) => row
            .iter()
            .map(|cell| match cell {
                Data::String(s) => s.clone(),
                Data::Float(f) => f.to_string(),
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(dt) => dt.to_string(),
                Data::Error(e) => format!("{:?}", e),
                Data::Empty => String::new(),
                _ => String::new(),
            })
            .collect(),
        None => bail!("XLSX sheet is empty"),
    };

    if headers.is_empty() {
        bail!("XLSX has no headers");
    }

    // Data rows
    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for row in rows_iter {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                Data::String(s) => s.clone(),
                Data::Float(f) => f.to_string(),
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(dt) => dt.to_string(),
                Data::Error(e) => format!("{:?}", e),
                Data::Empty => String::new(),
                _ => String::new(),
            })
            .collect();
        all_rows.push(cells);
    }

    // Detect column types from sample rows (up to 100)
    let sample_count = all_rows.len().min(100);
    let mut types = Vec::new();

    for col_idx in 0..headers.len() {
        let samples: Vec<&str> = all_rows
            .iter()
            .take(sample_count)
            .filter_map(|row| row.get(col_idx).map(|s| s.as_str()))
            .collect();
        types.push(detect_column_type(&samples));
    }

    info!(
        "Parsed XLSX: {} headers, {} rows, types: {:?}",
        headers.len(),
        all_rows.len(),
        types
    );

    Ok((headers, types, all_rows))
}

// ─── High-Level SQL Import Orchestration ───────────────────────────────────────

/// Result of processing tabular data in SQL mode.
#[derive(Debug)]
pub struct SqlImportResult {
    /// The CREATE TABLE DDL statement.
    pub create_table_ddl: String,
    /// Batch INSERT statements with their corresponding row data.
    /// Each tuple: (SQL template with ? placeholders, rows of values)
    pub insert_batches: Vec<(String, Vec<Vec<String>>)>,
    /// Dynamic table name (e.g. `tenant_abc123_src_42`)
    pub table_name: String,
    /// Total number of rows to insert
    pub total_rows: usize,
}

/// Process tabular data (CSV or XLSX) for SQL import mode.
///
/// Returns `SqlImportResult` containing DDL and INSERT statements ready
/// for execution against MariaDB.
pub fn process_tabular_for_sql(
    s3_key: &str,
    data: &[u8],
    tenant_id: &str,
    source_id: i64,
) -> Result<SqlImportResult> {
    let ext = s3_key.rsplit('.').next().unwrap_or("").to_lowercase();

    info!(
        "SQL Import: processing ext={}, tenant={}, source_id={}",
        ext, tenant_id, source_id
    );

    // Parse tabular data
    let (headers, types, rows) = match ext.as_str() {
        "csv" => parse_csv_for_sql(data)?,
        "xlsx" | "xls" => parse_xlsx_for_sql(data)?,
        _ => bail!("Unsupported tabular extension for SQL import: .{}", ext),
    };

    // Generate safe table name
    let table_name = sanitize_table_name(tenant_id, source_id)?;

    // Generate DDL
    let create_table_ddl = generate_create_table(&table_name, &headers, &types);

    // Generate batch INSERTs (1000 rows per batch)
    let total_rows = rows.len();
    let insert_batches = generate_batch_inserts(&table_name, &headers, &rows, 1000);

    info!(
        "SQL Import ready: table={}, {} rows in {} batches",
        table_name,
        total_rows,
        insert_batches.len()
    );

    Ok(SqlImportResult {
        create_table_ddl,
        insert_batches,
        table_name,
        total_rows,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-004a: detect_column_type — all numeric → DECIMAL
    // ========================================
    #[test]
    fn test_detect_column_type_numeric() {
        let samples = vec!["123", "456", "789"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Decimal);
    }

    #[test]
    fn test_detect_column_type_decimal_numbers() {
        let samples = vec!["12.5", "99.9", "0.01"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Decimal);
    }

    #[test]
    fn test_detect_column_type_negative_numbers() {
        let samples = vec!["-10", "42", "-3.14"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Decimal);
    }

    // ========================================
    // UT-004b: detect_column_type — all text → VARCHAR(255)
    // ========================================
    #[test]
    fn test_detect_column_type_text() {
        let samples = vec!["hello", "world"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Varchar255);
    }

    // ========================================
    // UT-004c: detect_column_type — all dates → DATE
    // ========================================
    #[test]
    fn test_detect_column_type_date() {
        let samples = vec!["2026-01-01", "2026-02-15"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Date);
    }

    // ========================================
    // UT-004d: detect_column_type — mixed → VARCHAR(255)
    // ========================================
    #[test]
    fn test_detect_column_type_mixed() {
        let samples = vec!["123", "hello", "456"];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Varchar255);
    }

    #[test]
    fn test_detect_column_type_empty() {
        let samples: Vec<&str> = vec![];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Varchar255);
    }

    #[test]
    fn test_detect_column_type_all_empty_strings() {
        let samples = vec!["", "  ", ""];
        assert_eq!(detect_column_type(&samples), SqlColumnType::Varchar255);
    }

    // ========================================
    // Table Name Sanitization
    // ========================================
    #[test]
    fn test_sanitize_table_name_valid() {
        let result = sanitize_table_name("abc123", 42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "tenant_abc123_src_42");
    }

    #[test]
    fn test_sanitize_table_name_with_underscores() {
        let result = sanitize_table_name("tenant_one", 5);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "tenant_tenant_one_src_5");
    }

    #[test]
    fn test_sanitize_table_name_unsafe_characters() {
        let result = sanitize_table_name("abc; DROP TABLE", 1);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unsafe characters")
        );
    }

    #[test]
    fn test_sanitize_table_name_sql_injection() {
        let result = sanitize_table_name("abc'--", 1);
        assert!(result.is_err());
    }

    // ========================================
    // UT-004e: generate_create_table — DDL generation
    // ========================================
    #[test]
    fn test_generate_create_table() {
        let headers = vec!["name".to_string(), "age".to_string()];
        let types = vec![SqlColumnType::Varchar255, SqlColumnType::Decimal];
        let ddl = generate_create_table("tenant_1_src_5", &headers, &types);

        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS tenant_1_src_5"));
        assert!(ddl.contains("name VARCHAR(255)"));
        assert!(ddl.contains("age DECIMAL"));
        assert!(ddl.contains("id BIGINT AUTO_INCREMENT PRIMARY KEY"));
        assert!(ddl.contains("created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP"));
    }

    #[test]
    fn test_generate_create_table_with_date() {
        let headers = vec!["name".to_string(), "birth_date".to_string()];
        let types = vec![SqlColumnType::Varchar255, SqlColumnType::Date];
        let ddl = generate_create_table("tenant_abc_src_10", &headers, &types);

        assert!(ddl.contains("name VARCHAR(255)"));
        assert!(ddl.contains("birth_date DATE"));
    }

    #[test]
    fn test_sanitize_column_name() {
        assert_eq!(sanitize_column_name("My Column!"), "my_column");
        assert_eq!(sanitize_column_name("  Name  "), "name");
        assert_eq!(sanitize_column_name("price ($)"), "price");
    }

    // ========================================
    // Batch INSERT generation
    // ========================================
    #[test]
    fn test_generate_batch_inserts() {
        let headers = vec!["name".to_string(), "age".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
            vec!["Charlie".to_string(), "35".to_string()],
        ];

        let batches = generate_batch_inserts("test_table", &headers, &rows, 2);

        // Should produce 2 batches: [2 rows, 1 row]
        assert_eq!(batches.len(), 2);

        // First batch: 2 rows
        assert!(
            batches[0]
                .0
                .contains("INSERT INTO test_table (name, age) VALUES (?, ?), (?, ?)")
        );
        assert_eq!(batches[0].1.len(), 2);

        // Second batch: 1 row
        assert!(
            batches[1]
                .0
                .contains("INSERT INTO test_table (name, age) VALUES (?, ?)")
        );
        assert_eq!(batches[1].1.len(), 1);
    }

    #[test]
    fn test_generate_batch_inserts_exact_batch() {
        let headers = vec!["col1".to_string()];
        let rows = vec![vec!["a".to_string()], vec!["b".to_string()]];

        let batches = generate_batch_inserts("t", &headers, &rows, 2);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].1.len(), 2);
    }

    // ========================================
    // CSV Parsing for SQL
    // ========================================
    #[test]
    fn test_parse_csv_for_sql() {
        let csv_data = b"Name,Age,City\nAlice,30,Bangkok\nBob,25,Tokyo\n";
        let result = parse_csv_for_sql(csv_data);
        assert!(
            result.is_ok(),
            "CSV parsing should succeed: {:?}",
            result.err()
        );

        let (headers, types, rows) = result.unwrap();
        assert_eq!(headers, vec!["Name", "Age", "City"]);
        assert_eq!(types[0], SqlColumnType::Varchar255); // Name
        assert_eq!(types[1], SqlColumnType::Decimal); // Age
        assert_eq!(types[2], SqlColumnType::Varchar255); // City
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_parse_csv_for_sql_with_dates() {
        let csv_data = b"Name,BirthDate\nAlice,2026-01-01\nBob,2026-02-15\n";
        let result = parse_csv_for_sql(csv_data);
        assert!(result.is_ok());

        let (_, types, _) = result.unwrap();
        assert_eq!(types[0], SqlColumnType::Varchar255); // Name
        assert_eq!(types[1], SqlColumnType::Date); // BirthDate
    }

    #[test]
    fn test_parse_csv_for_sql_empty_headers() {
        let csv_data = b"";
        let result = parse_csv_for_sql(csv_data);
        assert!(result.is_err());
    }

    // ========================================
    // XLSX Parsing for SQL
    // ========================================
    #[test]
    fn test_parse_xlsx_for_sql() {
        let xlsx_bytes = include_bytes!("../../tests/fixtures/sample.xlsx");
        let result = parse_xlsx_for_sql(xlsx_bytes);
        assert!(
            result.is_ok(),
            "XLSX parsing should succeed: {:?}",
            result.err()
        );

        let (headers, types, rows) = result.unwrap();
        assert!(!headers.is_empty(), "Should have headers");
        assert_eq!(
            headers.len(),
            types.len(),
            "Types count should match headers"
        );
        assert!(!rows.is_empty(), "Should have data rows");
    }

    // ========================================
    // End-to-end: process_tabular_for_sql
    // ========================================
    #[test]
    fn test_process_tabular_for_sql_csv() {
        let csv_data = b"Name,Age,City\nAlice,30,Bangkok\nBob,25,Tokyo\n";
        let result = process_tabular_for_sql("tenant/1/data.csv", csv_data, "tenant1", 42);
        assert!(
            result.is_ok(),
            "SQL import should succeed: {:?}",
            result.err()
        );

        let import = result.unwrap();
        assert_eq!(import.table_name, "tenant_tenant1_src_42");
        assert!(
            import
                .create_table_ddl
                .contains("CREATE TABLE IF NOT EXISTS tenant_tenant1_src_42")
        );
        assert!(import.create_table_ddl.contains("name VARCHAR(255)"));
        assert!(import.create_table_ddl.contains("age DECIMAL"));
        assert_eq!(import.total_rows, 2);
        assert_eq!(import.insert_batches.len(), 1); // 2 rows < 1000 batch size
    }

    #[test]
    fn test_process_tabular_for_sql_unsupported_ext() {
        let result = process_tabular_for_sql("file.json", b"data", "t1", 1);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported tabular extension")
        );
    }

    #[test]
    fn test_process_tabular_for_sql_unsafe_tenant() {
        let result = process_tabular_for_sql("data.csv", b"H\na\n", "DROP TABLE--", 1);
        assert!(result.is_err());
    }
}
