//! Artifact reader — search + supersession chain navigation.
//!
//! Scaffolding only — Sprint 56 implements:
//!   - `WellReader::search(SearchQuery) -> Vec<Artifact>` (semantic via BGE-M3
//!     + tier/surface filter + tenant scope)
//!   - `WellReader::supersession_chain(id) -> Vec<Artifact>` (walks
//!     superseded_by until terminal)
//!   - `WellReader::touched_by(id) -> Vec<SpanRef>` (joins through Neo4j
//!     :TOUCHED edges — see `touched` module)

use crate::model::*;

/// Search filter — tier/surface optional, tenant required.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Required tenant scope.
    pub tenant_id: String,
    /// Optional tier filter.
    pub tier: Option<Tier>,
    /// Optional surface filter (redundant with tier when both supplied).
    pub surface: Option<Surface>,
    /// Free text query — embedded by caller before SQL.
    pub query_text: String,
    /// Limit results (default 20 enforced at API).
    pub limit: u32,
}

/// Reference to a heimdall-trace span (returned by `touched_by`).
#[derive(Debug, Clone)]
pub struct SpanRef {
    /// OTel trace id.
    pub trace_id: String,
    /// OTel span id within the trace.
    pub span_id: String,
    /// Role of the touch — see ADR-011 §D2.
    pub role: TouchRole,
    /// Timestamp of touch.
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Touch role — matches `asgard.well.touched[].role` span attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchRole {
    /// Artifact read.
    Used,
    /// Artifact created.
    Generated,
    /// Existing artifact updated.
    Refined,
    /// Existing artifact contradicted.
    Contradicted,
}

use crate::{Result, WellError};

// NOTE: All SELECT queries cast confidence: `CAST(confidence AS DOUBLE)`.
// sqlx-mysql lacks native DECIMAL reading without the `bigdecimal`/`rust_decimal`
// feature; the cast keeps the read path dependency-light.

/// MariaDB-backed artifact reader.
///
/// Neo4j-backed queries (touched_by, supersession across :REFINES edges) are
/// kept in a separate trait — this struct handles the MariaDB hot path.
pub struct WellReader {
    pool: sqlx::MySqlPool,
}

impl WellReader {
    /// Construct.
    pub fn new(pool: sqlx::MySqlPool) -> Self {
        Self { pool }
    }

    /// Fetch one artifact by id, scoped to tenant. `None` if not found.
    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        id: crate::model::ArtifactId,
    ) -> Result<Option<Artifact>> {
        let row: Option<sqlx::mysql::MySqlRow> =
            sqlx::query("SELECT id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, content, embedding, prov_used, prov_generated_by, CAST(confidence AS DOUBLE) AS confidence, promoted_from, consolidation_state, superseded_by, created_at FROM memory_artifact WHERE tenant_id = ? AND id = ?")
                .bind(tenant_id)
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        row.map(|r| Artifact::from_row(&r).map_err(WellError::Provenance))
            .transpose()
    }

    /// List artifacts by tenant + optional tier/surface filter.
    /// Newest first; pass `limit` ≤ 100 in callers (no DB-side cap enforced).
    pub async fn list_by_tenant(
        &self,
        tenant_id: &str,
        tier: Option<crate::model::Tier>,
        surface: Option<crate::model::Surface>,
        limit: i64,
    ) -> Result<Vec<Artifact>> {
        // Build a deterministic query — avoid runtime SQL string concat for
        // safety. Branch on the four possible filter combinations.
        let rows = match (tier, surface) {
            (Some(t), Some(s)) => {
                sqlx::query(
                    "SELECT id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, content, embedding, prov_used, prov_generated_by, CAST(confidence AS DOUBLE) AS confidence, promoted_from, consolidation_state, superseded_by, created_at FROM memory_artifact \
                     WHERE tenant_id = ? AND tier = ? AND surface = ? \
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(tenant_id)
                .bind(t.as_sql_str())
                .bind(s.as_sql_str())
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            (Some(t), None) => {
                sqlx::query(
                    "SELECT id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, content, embedding, prov_used, prov_generated_by, CAST(confidence AS DOUBLE) AS confidence, promoted_from, consolidation_state, superseded_by, created_at FROM memory_artifact \
                     WHERE tenant_id = ? AND tier = ? \
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(tenant_id)
                .bind(t.as_sql_str())
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            (None, Some(s)) => {
                sqlx::query(
                    "SELECT id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, content, embedding, prov_used, prov_generated_by, CAST(confidence AS DOUBLE) AS confidence, promoted_from, consolidation_state, superseded_by, created_at FROM memory_artifact \
                     WHERE tenant_id = ? AND surface = ? \
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(tenant_id)
                .bind(s.as_sql_str())
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            (None, None) => {
                sqlx::query(
                    "SELECT id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, content, embedding, prov_used, prov_generated_by, CAST(confidence AS DOUBLE) AS confidence, promoted_from, consolidation_state, superseded_by, created_at FROM memory_artifact \
                     WHERE tenant_id = ? \
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(tenant_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
        };

        rows.iter()
            .map(|r| Artifact::from_row(r).map_err(WellError::Provenance))
            .collect()
    }

    /// Walk the supersession chain starting at `id`. Returns artifacts in
    /// order from `id` → newer. Stops at the terminal artifact (no
    /// superseded_by) or after 32 hops (cycle guard).
    pub async fn supersession_chain(
        &self,
        tenant_id: &str,
        id: crate::model::ArtifactId,
    ) -> Result<Vec<Artifact>> {
        let mut chain = Vec::new();
        let mut cursor = Some(id);
        for _ in 0..32 {
            let Some(cur) = cursor else { break };
            let Some(art) = self.get_by_id(tenant_id, cur).await? else {
                break;
            };
            cursor = art.superseded_by;
            chain.push(art);
        }
        Ok(chain)
    }
}

use crate::model::Artifact;
