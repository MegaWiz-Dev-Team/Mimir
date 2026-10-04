//! Versioned FHIR R5 store on SQLite — ADR-006 Amendment 1.
//!
//! Three tables, one write path:
//!
//! - `resource` — current state of each `(type, id)`;
//! - `resource_history` — append-only snapshot of every version, with the
//!   SHA-256 of the exact JSON stored;
//! - `audit_chain` — append-only, hash-chained log; each entry carries the
//!   history row's SHA-256, so the chain is the **integrity** authority and the
//!   history table is the **retrieval** path.
//!
//! Every create/update writes all three inside **one transaction**; if any
//! insert fails (e.g. the audit row is rejected) nothing is written. History and
//! audit rows cannot be updated or deleted (SQLite triggers). `meta.versionId`
//! and `meta.lastUpdated` are assigned by the store on write; values supplied by
//! callers are overwritten.
//!
//! Supported reads: `read`, `vread`, `history`, and `search` over a small, explicit
//! parameter set (unknown parameters are an error, never silently ignored).

use std::fmt::Write as _;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::datatypes::{Id, Instant, Meta};
use crate::resources::FhirResource;

/// Store errors.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Underlying SQLite failure (includes rejected audit rows → rollback).
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// JSON (de)serialization failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// `update` on a resource that does not exist.
    #[error("{0}/{1} not found")]
    NotFound(String, String),
    /// `If-Match` version did not equal the current version.
    #[error("version conflict on {resource}: expected {expected}, current {current}")]
    VersionConflict {
        /// `Type/id`.
        resource: String,
        /// Version the writer last saw.
        expected: u64,
        /// Version actually current.
        current: u64,
    },
    /// `create` with an id that already exists.
    #[error("{0}/{1} already exists")]
    AlreadyExists(String, String),
    /// Search parameter not supported for this resource.
    #[error("unsupported search parameter {0:?}")]
    UnsupportedSearchParam(String),
    /// The agent (who performed the write) must be named.
    #[error("agent must be a non-empty reference (e.g. Practitioner/123)")]
    MissingAgent,
    /// Internal invariant broken (e.g. a generated id or instant failed validation).
    #[error("invariant: {0}")]
    Invariant(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, StoreError>;

/// One audit-chain entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Monotonic sequence.
    pub seq: i64,
    /// Instant of the write.
    pub ts: String,
    /// `create` or `update`.
    pub action: String,
    /// Resource type.
    pub resource_type: String,
    /// Resource id.
    pub id: String,
    /// Version written.
    pub version_id: u64,
    /// SHA-256 of the stored history JSON.
    pub sha256: String,
    /// Who wrote it.
    pub agent: String,
    /// Previous entry's hash.
    pub prev_hash: String,
    /// This entry's hash.
    pub hash: String,
}

/// First `prev_hash` of the chain.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

const SCHEMA: &str = r"
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS resource (
    type TEXT NOT NULL, id TEXT NOT NULL, version_id INTEGER NOT NULL,
    last_updated TEXT NOT NULL, json TEXT NOT NULL,
    PRIMARY KEY (type, id)
);
CREATE TABLE IF NOT EXISTS resource_history (
    type TEXT NOT NULL, id TEXT NOT NULL, version_id INTEGER NOT NULL,
    last_updated TEXT NOT NULL, json TEXT NOT NULL, sha256 TEXT NOT NULL,
    PRIMARY KEY (type, id, version_id)
);
CREATE TABLE IF NOT EXISTS audit_chain (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('create', 'update')),
    type TEXT NOT NULL, id TEXT NOT NULL, version_id INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    agent TEXT NOT NULL CHECK (length(agent) > 0),
    prev_hash TEXT NOT NULL, hash TEXT NOT NULL UNIQUE
);
CREATE TRIGGER IF NOT EXISTS history_no_update BEFORE UPDATE ON resource_history
BEGIN SELECT RAISE(ABORT, 'resource_history is append-only'); END;
CREATE TRIGGER IF NOT EXISTS history_no_delete BEFORE DELETE ON resource_history
BEGIN SELECT RAISE(ABORT, 'resource_history is append-only'); END;
CREATE TRIGGER IF NOT EXISTS audit_no_update BEFORE UPDATE ON audit_chain
BEGIN SELECT RAISE(ABORT, 'audit_chain is append-only'); END;
CREATE TRIGGER IF NOT EXISTS audit_no_delete BEFORE DELETE ON audit_chain
BEGIN SELECT RAISE(ABORT, 'audit_chain is append-only'); END;
";

fn sha256_hex(s: &str) -> String {
    use std::fmt::Write as _;
    Sha256::digest(s.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

#[allow(clippy::too_many_arguments)]
fn entry_hash(
    prev: &str,
    ts: &str,
    action: &str,
    rtype: &str,
    id: &str,
    version: u64,
    sha: &str,
    agent: &str,
) -> String {
    sha256_hex(&format!(
        "{prev}\n{ts}\n{action}\n{rtype}\n{id}\n{version}\n{sha}\n{agent}"
    ))
}

/// Token search over an array of `{system, <key>}` objects (`identifier`, `meta.tag`):
/// `system|value` matches both, a bare `value` matches any system.
fn push_token(sql: &mut String, args: &mut Vec<String>, array: &str, key: &str, value: &str) {
    let (system, val) = match value.split_once('|') {
        Some((s, v)) => (Some(s), v),
        None => (None, value),
    };
    let _ = write!(
        sql,
        " AND EXISTS (SELECT 1 FROM json_each(r.json, '{array}') AS t \
         WHERE json_extract(t.value, '$.{key}') = ?"
    );
    args.push(val.to_string());
    if let Some(s) = system {
        sql.push_str(" AND json_extract(t.value, '$.system') = ?");
        args.push(s.to_string());
    }
    sql.push(')');
}

fn now_instant() -> Result<Instant> {
    let s = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Instant::new(s).map_err(|e| StoreError::Invariant(format!("instant: {e:?}")))
}

/// A versioned FHIR store.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) a store file.
    ///
    /// # Errors
    /// SQLite open / schema errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    /// In-memory store (tests).
    ///
    /// # Errors
    /// SQLite errors.
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Create a resource. Assigns an id if absent, sets `meta.versionId = 1` and
    /// `meta.lastUpdated`, and writes current + history + audit atomically.
    ///
    /// # Errors
    /// [`StoreError::AlreadyExists`], [`StoreError::MissingAgent`], SQLite/JSON errors.
    pub fn create<T: FhirResource>(&mut self, mut resource: T, agent: &str) -> Result<T> {
        if agent.trim().is_empty() {
            return Err(StoreError::MissingAgent);
        }
        let id = if let Some(id) = resource.id() {
            id.clone()
        } else {
            let id = Id::new(uuid::Uuid::new_v4().to_string())
                .map_err(|e| StoreError::Invariant(format!("id: {e:?}")))?;
            resource.set_id(id.clone());
            id
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM resource WHERE type = ?1 AND id = ?2",
                params![T::RESOURCE_TYPE, id.as_ref()],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if exists {
            return Err(StoreError::AlreadyExists(
                T::RESOURCE_TYPE.into(),
                id.to_string(),
            ));
        }
        let stored = Self::write_version(&tx, resource, &id, 1, "create", agent)?;
        tx.commit()?;
        Ok(stored)
    }

    /// Update a resource to a new version. `if_match` = the version the writer last
    /// saw (FHIR `If-Match`); `None` skips the check.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], [`StoreError::VersionConflict`], [`StoreError::MissingAgent`].
    pub fn update<T: FhirResource>(
        &mut self,
        resource: T,
        if_match: Option<u64>,
        agent: &str,
    ) -> Result<T> {
        if agent.trim().is_empty() {
            return Err(StoreError::MissingAgent);
        }
        let id = resource
            .id()
            .cloned()
            .ok_or_else(|| StoreError::NotFound(T::RESOURCE_TYPE.into(), "<no id>".into()))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT version_id FROM resource WHERE type = ?1 AND id = ?2",
                params![T::RESOURCE_TYPE, id.as_ref()],
                |r| r.get(0),
            )
            .optional()?;
        let current =
            current.ok_or_else(|| StoreError::NotFound(T::RESOURCE_TYPE.into(), id.to_string()))?;
        let current = u64::try_from(current).map_err(|e| StoreError::Invariant(e.to_string()))?;
        if let Some(expected) = if_match {
            if expected != current {
                return Err(StoreError::VersionConflict {
                    resource: format!("{}/{}", T::RESOURCE_TYPE, id),
                    expected,
                    current,
                });
            }
        }
        let stored = Self::write_version(&tx, resource, &id, current + 1, "update", agent)?;
        tx.commit()?;
        Ok(stored)
    }

    fn write_version<T: FhirResource>(
        tx: &rusqlite::Transaction<'_>,
        mut resource: T,
        id: &Id,
        version: u64,
        action: &str,
        agent: &str,
    ) -> Result<T> {
        let ts = now_instant()?;
        let meta = resource.meta_mut().get_or_insert_with(Meta::default);
        meta.version_id = Some(version.to_string());
        meta.last_updated = Some(ts.clone());
        let json = serde_json::to_string(&resource)?;
        let sha = sha256_hex(&json);
        let v = i64::try_from(version).map_err(|e| StoreError::Invariant(e.to_string()))?;
        tx.execute(
            "INSERT INTO resource (type, id, version_id, last_updated, json) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(type, id) DO UPDATE SET version_id = excluded.version_id,
               last_updated = excluded.last_updated, json = excluded.json",
            params![T::RESOURCE_TYPE, id.as_ref(), v, ts.as_ref(), json],
        )?;
        tx.execute(
            "INSERT INTO resource_history (type, id, version_id, last_updated, json, sha256)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![T::RESOURCE_TYPE, id.as_ref(), v, ts.as_ref(), json, sha],
        )?;
        let prev: String = tx
            .query_row(
                "SELECT hash FROM audit_chain ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_else(|| GENESIS.to_string());
        let hash = entry_hash(
            &prev,
            ts.as_ref(),
            action,
            T::RESOURCE_TYPE,
            id.as_ref(),
            version,
            &sha,
            agent,
        );
        tx.execute(
            "INSERT INTO audit_chain (ts, action, type, id, version_id, sha256, agent, prev_hash, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![ts.as_ref(), action, T::RESOURCE_TYPE, id.as_ref(), v, sha, agent, prev, hash],
        )?;
        Ok(resource)
    }

    /// Current version of `Type/id`.
    ///
    /// # Errors
    /// SQLite / JSON errors.
    pub fn read<T: FhirResource>(&self, id: &str) -> Result<Option<T>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM resource WHERE type = ?1 AND id = ?2",
                params![T::RESOURCE_TYPE, id],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(StoreError::from))
            .transpose()
    }

    /// A specific version (`Type/id/_history/version`).
    ///
    /// # Errors
    /// SQLite / JSON errors.
    pub fn vread<T: FhirResource>(&self, id: &str, version: u64) -> Result<Option<T>> {
        let v = i64::try_from(version).map_err(|e| StoreError::Invariant(e.to_string()))?;
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM resource_history WHERE type = ?1 AND id = ?2 AND version_id = ?3",
                params![T::RESOURCE_TYPE, id, v],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(StoreError::from))
            .transpose()
    }

    /// All versions of `Type/id`, newest first (FHIR `_history` order).
    ///
    /// # Errors
    /// SQLite / JSON errors.
    pub fn history<T: FhirResource>(&self, id: &str) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare(
            "SELECT json FROM resource_history WHERE type = ?1 AND id = ?2 ORDER BY version_id DESC",
        )?;
        let rows = stmt.query_map(params![T::RESOURCE_TYPE, id], |r| r.get::<_, String>(0))?;
        rows.map(|j| Ok(serde_json::from_str(&j?)?)).collect()
    }

    /// Search the current versions with AND-combined parameters.
    ///
    /// Supported: `_id`; `identifier` (`system|value` or `value`); `_tag`
    /// (`system|code` or `code`, over `meta.tag`); reference params
    /// `subject`, `patient`, `encounter`, `owner`, `focus`, `requester`, `for`
    /// (exact reference string, e.g. `Patient/123`); token `status`. Newest first.
    ///
    /// # Errors
    /// [`StoreError::UnsupportedSearchParam`] for anything else; SQLite / JSON errors.
    pub fn search<T: FhirResource>(&self, params_in: &[(&str, &str)]) -> Result<Vec<T>> {
        // `r.json` is qualified: inside json_each() an unqualified `json` would
        // resolve to json_each's own hidden `json` column.
        let mut sql = String::from("SELECT r.json FROM resource r WHERE r.type = ?");
        let mut args: Vec<String> = vec![T::RESOURCE_TYPE.to_string()];
        for (name, value) in params_in {
            match *name {
                "_id" => {
                    sql.push_str(" AND r.id = ?");
                    args.push((*value).to_string());
                }
                "status" => {
                    sql.push_str(" AND json_extract(r.json, '$.status') = ?");
                    args.push((*value).to_string());
                }
                "subject" | "patient" | "encounter" | "owner" | "focus" | "requester" | "for" => {
                    let path = if *name == "patient" { "subject" } else { name };
                    let _ = write!(sql, " AND json_extract(r.json, '$.{path}.reference') = ?");
                    args.push((*value).to_string());
                }
                "identifier" => push_token(&mut sql, &mut args, "$.identifier", "value", value),
                "_tag" => push_token(&mut sql, &mut args, "$.meta.tag", "code", value),
                other => return Err(StoreError::UnsupportedSearchParam(other.to_string())),
            }
        }
        sql.push_str(" ORDER BY r.last_updated DESC, r.id");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
            r.get::<_, String>(0)
        })?;
        rows.map(|j| Ok(serde_json::from_str(&j?)?)).collect()
    }

    /// The whole audit chain, oldest first.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn audit_chain(&self) -> Result<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, ts, action, type, id, version_id, sha256, agent, prev_hash, hash
             FROM audit_chain ORDER BY seq",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AuditEntry {
                seq: r.get(0)?,
                ts: r.get(1)?,
                action: r.get(2)?,
                resource_type: r.get(3)?,
                id: r.get(4)?,
                version_id: u64::try_from(r.get::<_, i64>(5)?).unwrap_or(0),
                sha256: r.get(6)?,
                agent: r.get(7)?,
                prev_hash: r.get(8)?,
                hash: r.get(9)?,
            })
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(StoreError::from)
    }

    /// Verify integrity: every chain link re-derives, and every history row's
    /// stored JSON hashes to the SHA-256 recorded **in the chain** for that version.
    /// Returns the number of verified entries, or the first failing `seq`
    /// (`0` = a history row with no chain entry).
    ///
    /// # Errors
    /// SQLite errors.
    pub fn verify(&self) -> Result<std::result::Result<usize, i64>> {
        let chain = self.audit_chain()?;
        let mut prev = GENESIS.to_string();
        for e in &chain {
            let h = entry_hash(
                &prev,
                &e.ts,
                &e.action,
                &e.resource_type,
                &e.id,
                e.version_id,
                &e.sha256,
                &e.agent,
            );
            if e.prev_hash != prev || e.hash != h {
                return Ok(Err(e.seq));
            }
            let v =
                i64::try_from(e.version_id).map_err(|x| StoreError::Invariant(x.to_string()))?;
            let json: Option<String> = self
                .conn
                .query_row(
                    "SELECT json FROM resource_history WHERE type = ?1 AND id = ?2 AND version_id = ?3",
                    params![e.resource_type, e.id, v],
                    |r| r.get(0),
                )
                .optional()?;
            match json {
                Some(j) if sha256_hex(&j) == e.sha256 => {}
                _ => return Ok(Err(e.seq)),
            }
            prev.clone_from(&e.hash);
        }
        let history_rows: i64 =
            self.conn
                .query_row("SELECT count(*) FROM resource_history", [], |r| r.get(0))?;
        if usize::try_from(history_rows).unwrap_or(usize::MAX) != chain.len() {
            return Ok(Err(0));
        }
        Ok(Ok(chain.len()))
    }

    /// Raw connection — for tests that need to simulate tampering.
    #[doc(hidden)]
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}
