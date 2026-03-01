//! External DB Connector Service (Issue #152)
//!
//! Connect to external MySQL/PostgreSQL/SQLite databases.
//! Features: query sandboxing, schema discovery, data import.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Column};
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Types & Models
// ═══════════════════════════════════════════════════════════════════════════════

/// Supported external database types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
    Postgres,
    Sqlite,
}

impl std::fmt::Display for DbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbType::Mysql => write!(f, "mysql"),
            DbType::Postgres => write!(f, "postgres"),
            DbType::Sqlite => write!(f, "sqlite"),
        }
    }
}

/// Parsed connection info extracted from a connection string
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub db_type: DbType,
    pub host: String,
    pub port: Option<u16>,
    pub database: String,
    pub user: Option<String>,
}

/// Table schema discovered from an external database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count_estimate: Option<i64>,
}

/// Column info from schema discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
}

/// Request to test a DB connection
#[derive(Debug, Deserialize)]
pub struct TestConnectionRequest {
    pub name: String,
    pub db_type: DbType,
    pub connection_string: String,
}

/// Request to discover schema
#[derive(Debug, Deserialize)]
pub struct DiscoverSchemaRequest {
    pub connection_string: String,
    pub db_type: DbType,
}

/// Request to import data from external DB
#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub connection_string: String,
    pub db_type: DbType,
    pub query: String,
    pub source_name: String,
}

/// Import result
#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub rows_imported: usize,
    pub columns: Vec<String>,
    pub markdown_preview: String,
    pub total_chars: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query Sandboxing — PURE FUNCTIONS (TDD-testable)
// ═══════════════════════════════════════════════════════════════════════════════

/// Dangerous SQL keywords that indicate DDL or DML write operations
const DANGEROUS_KEYWORDS: &[&str] = &[
    "DROP", "ALTER", "CREATE", "TRUNCATE",
    "INSERT", "UPDATE", "DELETE",
    "GRANT", "REVOKE",
    "EXEC", "EXECUTE",
    "CALL",
    "LOAD", "INTO OUTFILE", "INTO DUMPFILE",
];

/// Validate that a SQL query is safe (read-only SELECT).
///
/// Rejects:
/// - DDL: CREATE, ALTER, DROP, TRUNCATE
/// - DML writes: INSERT, UPDATE, DELETE
/// - Privilege: GRANT, REVOKE
/// - Execution: EXEC, EXECUTE, CALL
/// - File ops: LOAD, INTO OUTFILE/DUMPFILE
/// - Comment injection: --, /* */
/// - Semicolons (multi-statement)
pub fn validate_query(query: &str) -> Result<()> {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        bail!("Query cannot be empty");
    }

    // Check for comment-based injection
    if trimmed.contains("--") {
        bail!("SQL comments (--) are not allowed in queries");
    }
    if trimmed.contains("/*") || trimmed.contains("*/") {
        bail!("SQL block comments (/* */) are not allowed in queries");
    }

    // Check for semicolons (multi-statement attack)
    // Allow trailing semicolon only
    let without_trailing = trimmed.trim_end_matches(';').trim();
    if without_trailing.contains(';') {
        bail!("Multiple statements (;) are not allowed");
    }

    // Normalize to uppercase for keyword checking
    let upper = trimmed.to_uppercase();

    // Must start with SELECT or WITH (CTEs)
    let first_word = upper.split_whitespace().next().unwrap_or("");
    if first_word != "SELECT" && first_word != "WITH" && first_word != "EXPLAIN" {
        bail!("Only SELECT, WITH (CTE), and EXPLAIN queries are allowed. Found: {}", first_word);
    }

    // Check for dangerous keywords anywhere in the query
    for keyword in DANGEROUS_KEYWORDS {
        // Use word boundary detection: keyword must be surrounded by non-alphanumeric chars
        let pattern = format!(r"\b{}\b", keyword);
        if let Ok(re) = regex::Regex::new(&pattern) {
            if re.is_match(&upper) {
                bail!("Dangerous keyword '{}' detected in query", keyword);
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Connection String Parsing — PURE FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Parse a connection string and extract metadata.
///
/// Supports formats:
/// - `mysql://user:pass@host:3306/dbname`
/// - `mariadb://user:pass@host:3306/dbname`  (alias for mysql)
/// - `postgres://user:pass@host:5432/dbname`  
/// - `sqlite:///path/to/file.db` or `sqlite://path/to/file.db`
pub fn parse_connection_string(conn_str: &str) -> Result<ConnectionInfo> {
    let trimmed = conn_str.trim();

    if trimmed.is_empty() {
        bail!("Connection string cannot be empty");
    }

    // Detect DB type from scheme
    let (db_type, rest) = if trimmed.starts_with("mysql://") || trimmed.starts_with("mariadb://") {
        let skip = if trimmed.starts_with("mariadb://") { 10 } else { 8 };
        (DbType::Mysql, &trimmed[skip..])
    } else if trimmed.starts_with("postgres://") || trimmed.starts_with("postgresql://") {
        let skip = if trimmed.starts_with("postgresql://") { 13 } else { 11 };
        (DbType::Postgres, &trimmed[skip..])
    } else if trimmed.starts_with("sqlite://") {
        return Ok(ConnectionInfo {
            db_type: DbType::Sqlite,
            host: "local".to_string(),
            port: None,
            database: trimmed[9..].trim_start_matches('/').to_string(),
            user: None,
        });
    } else {
        bail!("Unsupported connection scheme. Use mysql://, mariadb://, postgres://, or sqlite://");
    };

    // Parse user:pass@host:port/database
    let (user, host_part) = if rest.contains('@') {
        let parts: Vec<&str> = rest.splitn(2, '@').collect();
        let user = parts[0].split(':').next().unwrap_or("").to_string();
        (Some(user), parts[1])
    } else {
        (None, rest)
    };

    // Parse host:port/database
    let (host_port, database) = if host_part.contains('/') {
        let parts: Vec<&str> = host_part.splitn(2, '/').collect();
        (parts[0], parts[1].to_string())
    } else {
        (host_part, String::new())
    };

    let (host, port) = if host_port.contains(':') {
        let parts: Vec<&str> = host_port.splitn(2, ':').collect();
        (parts[0].to_string(), parts[1].parse::<u16>().ok())
    } else {
        (host_port.to_string(), None)
    };

    // Remove query params from database name
    let database = database.split('?').next().unwrap_or("").to_string();

    Ok(ConnectionInfo {
        db_type,
        host,
        port,
        database,
        user,
    })
}

/// Validate a connection config has all required fields
pub fn validate_connection_config(req: &TestConnectionRequest) -> Result<()> {
    if req.name.trim().is_empty() {
        bail!("Connection name cannot be empty");
    }
    if req.name.len() > 100 {
        bail!("Connection name too long (max 100 chars)");
    }
    if req.connection_string.trim().is_empty() {
        bail!("Connection string cannot be empty");
    }

    // Validate connection string is parseable
    let info = parse_connection_string(&req.connection_string)?;

    // Verify db_type matches
    if info.db_type != req.db_type {
        bail!(
            "Connection string scheme ({}) doesn't match specified db_type ({})",
            info.db_type, req.db_type
        );
    }

    // SQLite must have a database path
    if info.db_type == DbType::Sqlite && info.database.is_empty() {
        bail!("SQLite connection must include a file path");
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Schema Discovery Queries — PURE FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Build the SQL query to discover tables and columns for a given DB type.
///
/// Returns (tables_query, columns_query_template) where the columns query
/// has a `{TABLE_NAME}` placeholder.
pub fn build_schema_query(db_type: &DbType) -> (String, String) {
    match db_type {
        DbType::Mysql => (
            // List all user tables
            "SELECT TABLE_NAME, TABLE_ROWS FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME".to_string(),
            // List columns for a specific table
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_KEY FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{TABLE_NAME}' ORDER BY ORDINAL_POSITION".to_string(),
        ),
        DbType::Postgres => (
            "SELECT tablename AS table_name, NULL AS table_rows FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename".to_string(),
            "SELECT column_name, data_type, is_nullable, CASE WHEN pk.column_name IS NOT NULL THEN 'PRI' ELSE '' END AS column_key FROM information_schema.columns c LEFT JOIN (SELECT ku.column_name FROM information_schema.table_constraints tc JOIN information_schema.key_column_usage ku ON tc.constraint_name = ku.constraint_name WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_name = '{TABLE_NAME}') pk ON c.column_name = pk.column_name WHERE c.table_schema = 'public' AND c.table_name = '{TABLE_NAME}' ORDER BY c.ordinal_position".to_string(),
        ),
        DbType::Sqlite => (
            "SELECT name AS table_name, NULL AS table_rows FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name".to_string(),
            "SELECT name AS column_name, type AS data_type, CASE WHEN \"notnull\" = 0 THEN 'YES' ELSE 'NO' END AS is_nullable, CASE WHEN pk = 1 THEN 'PRI' ELSE '' END AS column_key FROM pragma_table_info('{TABLE_NAME}')".to_string(),
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Data Conversion — PURE FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Convert tabular rows into a markdown table string.
///
/// Produces a standard markdown table with header row and alignment row.
/// Limits to first 100 rows for preview.
pub fn rows_to_markdown(columns: &[String], rows: &[Vec<String>]) -> String {
    if columns.is_empty() {
        return String::new();
    }

    let mut md = String::new();

    // Header row
    md.push_str("| ");
    md.push_str(&columns.join(" | "));
    md.push_str(" |\n");

    // Alignment row
    md.push_str("| ");
    md.push_str(&columns.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    md.push_str(" |\n");

    // Data rows (max 100 for preview)
    let max_rows = rows.len().min(100);
    for row in rows.iter().take(max_rows) {
        md.push_str("| ");
        // Ensure row has same number of columns, pad with empty if needed
        let cells: Vec<String> = columns.iter().enumerate().map(|(i, _)| {
            row.get(i).cloned().unwrap_or_default()
                .replace('|', "\\|")  // escape pipes in values
                .replace('\n', " ")   // remove newlines
        }).collect();
        md.push_str(&cells.join(" | "));
        md.push_str(" |\n");
    }

    if rows.len() > 100 {
        md.push_str(&format!("\n*... and {} more rows*\n", rows.len() - 100));
    }

    md
}

// ═══════════════════════════════════════════════════════════════════════════════
// DB Operations (require pool) — NOT pure, tested via integration
// ═══════════════════════════════════════════════════════════════════════════════

/// Test a database connection by connecting and running a simple query.
pub async fn test_connection(conn_str: &str, db_type: &DbType) -> Result<String> {
    let pool = create_pool(conn_str, db_type).await?;
    
    let version_query = match db_type {
        DbType::Mysql => "SELECT VERSION() AS version",
        DbType::Postgres => "SELECT version() AS version",
        DbType::Sqlite => "SELECT sqlite_version() AS version",
    };

    let row: (String,) = sqlx::query_as(version_query)
        .fetch_one(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Connection test failed: {}", e))?;

    pool.close().await;
    info!(db_type = %db_type, version = %row.0, "External DB connection test successful");
    Ok(row.0)
}

/// Discover schema (tables + columns) from an external database.
pub async fn discover_schema(conn_str: &str, db_type: &DbType) -> Result<Vec<TableSchema>> {
    let pool = create_pool(conn_str, db_type).await?;
    let (tables_query, columns_template) = build_schema_query(db_type);

    // Fetch tables
    let table_rows: Vec<(String, Option<i64>)> = sqlx::query_as(&tables_query)
        .fetch_all(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Schema discovery failed: {}", e))?;

    let mut schemas = Vec::new();

    for (table_name, row_count) in &table_rows {
        let col_query = columns_template.replace("{TABLE_NAME}", table_name);
        
        let col_rows: Vec<(String, String, String, String)> = sqlx::query_as(&col_query)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

        let columns: Vec<ColumnInfo> = col_rows.iter().map(|(name, dtype, nullable, key)| {
            ColumnInfo {
                name: name.clone(),
                data_type: dtype.clone(),
                is_nullable: nullable.to_uppercase() == "YES",
                is_primary_key: key.to_uppercase() == "PRI",
            }
        }).collect();

        schemas.push(TableSchema {
            table_name: table_name.clone(),
            columns,
            row_count_estimate: *row_count,
        });
    }

    pool.close().await;
    info!(tables = schemas.len(), "Schema discovery complete");
    Ok(schemas)
}

/// Execute a read-only query and return results as markdown.
pub async fn execute_import_query(
    conn_str: &str,
    db_type: &DbType,
    query: &str,
) -> Result<ImportResult> {
    // Sandbox the query first
    validate_query(query)?;

    let pool = create_pool(conn_str, db_type).await?;

    // Execute as raw query and collect rows
    let rows = sqlx::query(query)
        .fetch_all(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Query execution failed: {}", e))?;

    if rows.is_empty() {
        pool.close().await;
        return Ok(ImportResult {
            rows_imported: 0,
            columns: vec![],
            markdown_preview: "*(No rows returned)*".to_string(),
            total_chars: 0,
        });
    }

    // Extract column names from first row
    let columns: Vec<String> = rows[0].columns().iter()
        .map(|c| c.name().to_string())
        .collect();

    // Convert rows to string vectors
    let string_rows: Vec<Vec<String>> = rows.iter().map(|row| {
        columns.iter().enumerate().map(|(i, _)| {
            // Try to get value as string; fallback to debug repr
            row.try_get::<String, _>(i)
                .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
                .or_else(|_| row.try_get::<f64, _>(i).map(|v| v.to_string()))
                .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
                .unwrap_or_else(|_| "NULL".to_string())
        }).collect()
    }).collect();

    let markdown = rows_to_markdown(&columns, &string_rows);
    let total_chars = markdown.len();
    let row_count = string_rows.len();

    pool.close().await;
    info!(rows = row_count, columns = columns.len(), "Import query executed");

    Ok(ImportResult {
        rows_imported: row_count,
        columns,
        markdown_preview: markdown,
        total_chars,
    })
}

/// Create a connection pool for an external database.
async fn create_pool(conn_str: &str, db_type: &DbType) -> Result<sqlx::AnyPool> {
    use sqlx::any::AnyPoolOptions;

    let pool = AnyPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(conn_str)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to {} database: {}", db_type, e))?;

    Ok(pool)
}

/// Save a connection config to the database
pub async fn save_connection(
    pool: &sqlx::MySqlPool,
    tenant_id: &str,
    req: &TestConnectionRequest,
    test_status: &str,
) -> Result<i64> {
    let result = sqlx::query(
        r#"INSERT INTO external_db_connections 
           (tenant_id, name, db_type, connection_string, last_tested_at, last_test_status)
           VALUES (?, ?, ?, ?, NOW(), ?)
           ON DUPLICATE KEY UPDATE 
           connection_string = VALUES(connection_string),
           last_tested_at = NOW(),
           last_test_status = VALUES(last_test_status)"#
    )
    .bind(tenant_id)
    .bind(&req.name)
    .bind(req.db_type.to_string())
    .bind(&req.connection_string)
    .bind(test_status)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id() as i64)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests — Pure function tests (no database required)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-014k: validate_query — rejects dangerous DDL/DML
    // ========================================
    #[test]
    fn test_validate_query_rejects_drop() {
        assert!(validate_query("DROP TABLE users").is_err());
    }

    #[test]
    fn test_validate_query_rejects_alter() {
        assert!(validate_query("ALTER TABLE users ADD col INT").is_err());
    }

    #[test]
    fn test_validate_query_rejects_insert() {
        assert!(validate_query("INSERT INTO users VALUES (1)").is_err());
    }

    #[test]
    fn test_validate_query_rejects_delete() {
        assert!(validate_query("DELETE FROM users WHERE id = 1").is_err());
    }

    #[test]
    fn test_validate_query_rejects_update() {
        assert!(validate_query("UPDATE users SET name = 'x'").is_err());
    }

    #[test]
    fn test_validate_query_rejects_truncate() {
        assert!(validate_query("TRUNCATE TABLE users").is_err());
    }

    #[test]
    fn test_validate_query_rejects_create() {
        assert!(validate_query("CREATE TABLE evil (id INT)").is_err());
    }

    #[test]
    fn test_validate_query_rejects_grant() {
        assert!(validate_query("GRANT ALL ON *.* TO 'evil'").is_err());
    }

    // ========================================
    // UT-014l: validate_query — allows safe queries
    // ========================================
    #[test]
    fn test_validate_query_allows_select() {
        assert!(validate_query("SELECT * FROM users").is_ok());
    }

    #[test]
    fn test_validate_query_allows_select_with_where() {
        assert!(validate_query("SELECT id, name FROM users WHERE age > 18").is_ok());
    }

    #[test]
    fn test_validate_query_allows_with_cte() {
        assert!(validate_query("WITH cte AS (SELECT 1) SELECT * FROM cte").is_ok());
    }

    #[test]
    fn test_validate_query_allows_explain() {
        assert!(validate_query("EXPLAIN SELECT * FROM users").is_ok());
    }

    #[test]
    fn test_validate_query_allows_trailing_semicolon() {
        assert!(validate_query("SELECT 1;").is_ok());
    }

    // ========================================
    // UT-014m: validate_query — rejects injection
    // ========================================
    #[test]
    fn test_validate_query_rejects_comment_dash() {
        assert!(validate_query("SELECT * FROM users -- DROP TABLE users").is_err());
    }

    #[test]
    fn test_validate_query_rejects_block_comment() {
        assert!(validate_query("SELECT * FROM users /* DROP TABLE evil */").is_err());
    }

    #[test]
    fn test_validate_query_rejects_multi_statement() {
        assert!(validate_query("SELECT 1; DROP TABLE users").is_err());
    }

    #[test]
    fn test_validate_query_rejects_empty() {
        assert!(validate_query("").is_err());
        assert!(validate_query("   ").is_err());
    }

    // ========================================
    // UT-014n: parse_connection_string
    // ========================================
    #[test]
    fn test_parse_mysql_connection() {
        let info = parse_connection_string("mysql://admin:pass@db.example.com:3306/mydb").unwrap();
        assert_eq!(info.db_type, DbType::Mysql);
        assert_eq!(info.host, "db.example.com");
        assert_eq!(info.port, Some(3306));
        assert_eq!(info.database, "mydb");
        assert_eq!(info.user, Some("admin".to_string()));
    }

    #[test]
    fn test_parse_postgres_connection() {
        let info = parse_connection_string("postgres://user:pass@localhost:5432/analytics").unwrap();
        assert_eq!(info.db_type, DbType::Postgres);
        assert_eq!(info.host, "localhost");
        assert_eq!(info.port, Some(5432));
        assert_eq!(info.database, "analytics");
    }

    #[test]
    fn test_parse_postgresql_scheme() {
        let info = parse_connection_string("postgresql://user:pass@host/db").unwrap();
        assert_eq!(info.db_type, DbType::Postgres);
    }

    #[test]
    fn test_parse_sqlite_connection() {
        let info = parse_connection_string("sqlite:///tmp/data.db").unwrap();
        assert_eq!(info.db_type, DbType::Sqlite);
        assert_eq!(info.host, "local");
        assert_eq!(info.database, "tmp/data.db");
        assert_eq!(info.user, None);
    }

    #[test]
    fn test_parse_empty_connection_string() {
        assert!(parse_connection_string("").is_err());
    }

    #[test]
    fn test_parse_unsupported_scheme() {
        assert!(parse_connection_string("mongodb://localhost/db").is_err());
    }

    #[test]
    fn test_parse_connection_with_query_params() {
        let info = parse_connection_string("mysql://u:p@host/db?ssl=true").unwrap();
        assert_eq!(info.database, "db");
    }

    // ========================================
    // UT-014o: build_schema_query per DB type
    // ========================================
    #[test]
    fn test_build_schema_query_mysql() {
        let (tables_q, cols_q) = build_schema_query(&DbType::Mysql);
        assert!(tables_q.contains("information_schema.TABLES"));
        assert!(tables_q.contains("TABLE_SCHEMA = DATABASE()"));
        assert!(cols_q.contains("{TABLE_NAME}"));
    }

    #[test]
    fn test_build_schema_query_postgres() {
        let (tables_q, cols_q) = build_schema_query(&DbType::Postgres);
        assert!(tables_q.contains("pg_tables"));
        assert!(tables_q.contains("schemaname = 'public'"));
        assert!(cols_q.contains("{TABLE_NAME}"));
    }

    #[test]
    fn test_build_schema_query_sqlite() {
        let (tables_q, cols_q) = build_schema_query(&DbType::Sqlite);
        assert!(tables_q.contains("sqlite_master"));
        assert!(cols_q.contains("pragma_table_info"));
    }

    // ========================================
    // UT-014p: rows_to_markdown
    // ========================================
    #[test]
    fn test_rows_to_markdown_basic() {
        let columns = vec!["Name".to_string(), "Age".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];

        let md = rows_to_markdown(&columns, &rows);
        assert!(md.contains("| Name | Age |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alice | 30 |"));
        assert!(md.contains("| Bob | 25 |"));
    }

    #[test]
    fn test_rows_to_markdown_escapes_pipes() {
        let columns = vec!["Value".to_string()];
        let rows = vec![vec!["test|value".to_string()]];
        let md = rows_to_markdown(&columns, &rows);
        assert!(md.contains("test\\|value"));
    }

    #[test]
    fn test_rows_to_markdown_empty() {
        let md = rows_to_markdown(&[], &[]);
        assert!(md.is_empty());
    }

    #[test]
    fn test_rows_to_markdown_padded_rows() {
        let columns = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let rows = vec![vec!["1".to_string()]]; // short row
        let md = rows_to_markdown(&columns, &rows);
        assert!(md.contains("| 1 |  |  |"));
    }

    // ========================================
    // UT-014q: validate_connection_config
    // ========================================
    #[test]
    fn test_validate_config_valid() {
        let req = TestConnectionRequest {
            name: "My MySQL DB".to_string(),
            db_type: DbType::Mysql,
            connection_string: "mysql://user:pass@localhost/db".to_string(),
        };
        assert!(validate_connection_config(&req).is_ok());
    }

    #[test]
    fn test_validate_config_empty_name() {
        let req = TestConnectionRequest {
            name: "".to_string(),
            db_type: DbType::Mysql,
            connection_string: "mysql://user:pass@localhost/db".to_string(),
        };
        assert!(validate_connection_config(&req).is_err());
    }

    #[test]
    fn test_validate_config_empty_connection() {
        let req = TestConnectionRequest {
            name: "Test".to_string(),
            db_type: DbType::Mysql,
            connection_string: "".to_string(),
        };
        assert!(validate_connection_config(&req).is_err());
    }

    #[test]
    fn test_validate_config_type_mismatch() {
        let req = TestConnectionRequest {
            name: "Test".to_string(),
            db_type: DbType::Postgres,
            connection_string: "mysql://user:pass@localhost/db".to_string(),
        };
        assert!(validate_connection_config(&req).is_err());
    }

    #[test]
    fn test_validate_config_name_too_long() {
        let req = TestConnectionRequest {
            name: "x".repeat(101),
            db_type: DbType::Mysql,
            connection_string: "mysql://u:p@h/db".to_string(),
        };
        assert!(validate_connection_config(&req).is_err());
    }

    // ========================================
    // UT-015a: parse mariadb:// as mysql alias
    // ========================================
    #[test]
    fn test_parse_mariadb_connection() {
        let info = parse_connection_string("mariadb://admin:pass@db.example.com:3306/mydb").unwrap();
        assert_eq!(info.db_type, DbType::Mysql);
        assert_eq!(info.host, "db.example.com");
        assert_eq!(info.port, Some(3306));
        assert_eq!(info.database, "mydb");
        assert_eq!(info.user, Some("admin".to_string()));
    }

    #[test]
    fn test_validate_config_mariadb_matches_mysql() {
        let req = TestConnectionRequest {
            name: "My MariaDB".to_string(),
            db_type: DbType::Mysql,
            connection_string: "mariadb://user:pass@localhost/db".to_string(),
        };
        assert!(validate_connection_config(&req).is_ok());
    }
}
