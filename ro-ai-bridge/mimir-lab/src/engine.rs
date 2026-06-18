//! DuckDB engine wrapper.
//!
//! Design choices that keep this robust + safe by default:
//!   1. **Read-only guard** — [`Engine::query_readonly`] rejects anything that is
//!      not a SELECT/WITH/DESCRIBE/SUMMARIZE/EXPLAIN/PRAGMA/SHOW statement.
//!   2. **CAST-to-VARCHAR fetch** — every projected column is `CAST` to VARCHAR
//!      in SQL and read as `Option<String>`, avoiding `ValueRef` variant churn
//!      across DuckDB versions.
//!   3. **Query timeout** — [`Engine::query_readonly_timeout`] interrupts a
//!      runaway query via DuckDB's interrupt handle + a watchdog thread.
//!   4. **Audit** — every public query is recorded to the attached
//!      [`AuditSink`] (Tyr-ingestible), with outcome ok/timeout/denied/error.

use crate::audit::{AuditContext, AuditEvent, AuditSink, NoopAuditSink};
use crate::error::{LabError, Result};
use crate::schema::{ColumnSchema, TableSchema};
use duckdb::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

pub struct Engine {
    conn: Connection,
    audit: Arc<dyn AuditSink>,
    ctx: AuditContext,
}

/// Result of a capped read-only query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    /// Rows of stringified cell values (NULL → `None`).
    pub rows: Vec<Vec<Option<String>>>,
    /// True if more rows existed beyond `row_cap` and were dropped.
    pub truncated: bool,
}

impl Engine {
    /// In-memory engine (ingest scratch space, tests). No-op audit by default.
    pub fn in_memory() -> Result<Self> {
        Ok(Self::wrap(Connection::open_in_memory()?))
    }

    /// File-backed engine (a tenant's persistent catalog db).
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self::wrap(Connection::open(path)?))
    }

    fn wrap(conn: Connection) -> Self {
        Self {
            conn,
            audit: Arc::new(NoopAuditSink),
            ctx: AuditContext::default(),
        }
    }

    /// Attach an audit sink (e.g. `TracingAuditSink` → Tyr).
    pub fn with_audit(mut self, sink: Arc<dyn AuditSink>) -> Self {
        self.audit = sink;
        self
    }

    /// Attribute audited actions to a tenant/actor.
    pub fn with_context(mut self, tenant_id: Option<String>, actor: Option<String>) -> Self {
        self.ctx = AuditContext { tenant_id, actor };
        self
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run DDL/DML (used by ingest). Returns affected-row count where applicable.
    pub fn execute(&self, sql: &str) -> Result<usize> {
        Ok(self.conn.execute(sql, [])?)
    }

    /// Single scalar `u64` (e.g. `SELECT COUNT(*) ...`).
    pub fn query_scalar_u64(&self, sql: &str) -> Result<u64> {
        let n: i64 = self.conn.query_row(sql, [], |r| r.get(0))?;
        Ok(n.max(0) as u64)
    }

    /// Infer the schema of an arbitrary SELECT via `DESCRIBE`.
    pub fn describe(&self, select_sql: &str) -> Result<TableSchema> {
        let describe = format!("DESCRIBE {select_sql}");
        let mut stmt = self.conn.prepare(&describe)?;
        let mut rows = stmt.query([])?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next()? {
            // DESCRIBE columns: column_name, column_type, null, key, default, extra
            let name: String = row.get(0)?;
            let sql_type: String = row.get(1)?;
            let null_flag: Option<String> = row.get(2).ok().flatten();
            let nullable = null_flag
                .map(|s| s.eq_ignore_ascii_case("YES"))
                .unwrap_or(true);
            columns.push(ColumnSchema {
                name,
                sql_type,
                nullable,
            });
        }
        Ok(TableSchema { columns })
    }

    /// Read-only query, capped at `row_cap` rows. Audited.
    pub fn query_readonly(&self, sql: &str, row_cap: usize) -> Result<QueryResult> {
        let res = self.run_select(sql, row_cap, None);
        self.audit_query(sql, &res);
        res
    }

    /// Read-only query with a wall-clock timeout (interrupts a runaway query).
    /// Audited; a timeout yields [`LabError::Timeout`].
    pub fn query_readonly_timeout(
        &self,
        sql: &str,
        row_cap: usize,
        timeout: Duration,
    ) -> Result<QueryResult> {
        let res = self.run_select(sql, row_cap, Some(timeout));
        self.audit_query(sql, &res);
        res
    }

    /// Internal (un-audited) read-only execution — used by ingest/pii helpers.
    pub(crate) fn run_select(
        &self,
        sql: &str,
        row_cap: usize,
        timeout: Option<Duration>,
    ) -> Result<QueryResult> {
        guard_read_only(sql)?;
        match timeout {
            None => self.run_select_inner(sql, row_cap),
            Some(d) => self.run_with_watchdog(d, || self.run_select_inner(sql, row_cap)),
        }
    }

    fn run_select_inner(&self, sql: &str, row_cap: usize) -> Result<QueryResult> {
        // Strip trailing ';' / whitespace — LLM agents often append them, which breaks both
        // `DESCRIBE {sql}` and the `(... ) AS t` row-cap wrapper (DuckDB "syntax error near ;").
        let sql = sql.trim_end_matches(|c: char| c == ';' || c.is_whitespace());
        let schema = self.describe(sql)?;
        let projection = schema
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "CAST(t.\"{}\" AS VARCHAR) AS c{i}",
                    c.name.replace('"', "\"\"")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let wrapped = format!(
            "SELECT {projection} FROM ({sql}) AS t LIMIT {}",
            row_cap.saturating_add(1)
        );

        let ncols = schema.columns.len();
        let mut stmt = self.conn.prepare(&wrapped)?;
        let mut rows = stmt.query([])?;
        let mut out: Vec<Vec<Option<String>>> = Vec::new();
        while let Some(row) = rows.next()? {
            let mut rec = Vec::with_capacity(ncols);
            for i in 0..ncols {
                rec.push(row.get::<usize, Option<String>>(i)?);
            }
            out.push(rec);
        }

        let truncated = out.len() > row_cap;
        if truncated {
            out.truncate(row_cap);
        }
        Ok(QueryResult {
            columns: schema.columns,
            rows: out,
            truncated,
        })
    }

    /// Run `f`, interrupting the DuckDB connection if it exceeds `d`.
    fn run_with_watchdog<T>(&self, d: Duration, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let (tx, rx) = mpsc::channel::<()>();
        let handle = self.conn.interrupt_handle();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_wd = fired.clone();
        let wd = std::thread::spawn(move || {
            if let Err(mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(d) {
                fired_wd.store(true, Ordering::SeqCst);
                handle.interrupt();
            }
        });
        let res = f();
        let _ = tx.send(()); // tell the watchdog to stand down
        let _ = wd.join();
        match res {
            Err(_) if fired.load(Ordering::SeqCst) => {
                Err(LabError::Timeout(format!("query exceeded {d:?}")))
            }
            other => other,
        }
    }

    fn audit_query(&self, sql: &str, res: &Result<QueryResult>) {
        let outcome = match res {
            Ok(_) => "ok",
            Err(LabError::Timeout(_)) => "timeout",
            Err(LabError::NotReadOnly(_)) => "denied",
            Err(_) => "error",
        };
        let mut target: String = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        // char-boundary-safe truncate (String::truncate panics mid-UTF8-char, e.g. Thai SQL)
        let mut end = target.len().min(200);
        while end > 0 && !target.is_char_boundary(end) {
            end -= 1;
        }
        target.truncate(end);
        self.audit.record(&AuditEvent {
            action: "analytics.query",
            tenant_id: self.ctx.tenant_id.clone(),
            actor: self.ctx.actor.clone(),
            target: Some(target),
            outcome,
            detail: res
                .as_ref()
                .ok()
                .map(|r| format!("rows={} truncated={}", r.rows.len(), r.truncated)),
        });
    }
}

/// Allow only statements that cannot mutate state.
fn guard_read_only(sql: &str) -> Result<()> {
    let head = sql
        .trim_start()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    const ALLOWED: &[&str] = &[
        "SELECT", "WITH", "DESCRIBE", "SUMMARIZE", "EXPLAIN", "PRAGMA", "SHOW", "TABLE", "VALUES",
    ];
    if ALLOWED.contains(&head.as_str()) {
        Ok(())
    } else {
        Err(LabError::NotReadOnly(head))
    }
}
