//! `:TOUCHED` edge materialization from heimdall-trace OTel spans.
//!
//! ADR-011 §D2. An async worker consumes spans from heimdall-trace
//! (Laminar/ClickHouse), extracts `asgard.well.touched[]` span attributes,
//! and projects them into Neo4j as `(:Span)-[:TOUCHED {role, ts}]->(:Artifact)`.
//!
//! Neo4j stores only the pointer `(trace_id, span_id)` — span content stays
//! in heimdall-trace. The UI deeplinks back via Heimdall JWT proxy.

use serde::{Deserialize, Serialize};

/// Single touch entry on a span — what Bifrost emits per step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchedAttr {
    /// Artifact id (ULID string).
    pub artifact_id: String,
    /// Touch role.
    pub role: TouchRole,
}

/// Touch role — matches `crate::reader::TouchRole` and span attribute schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchRole {
    /// Artifact read by this span.
    Used,
    /// Artifact written by this span.
    Generated,
    /// Artifact updated by this span.
    Refined,
    /// Artifact contradicted by this span — feeds consolidator high-prio queue.
    Contradicted,
}

/// Materialization batch — what the worker writes per Neo4j transaction.
#[derive(Debug, Clone)]
pub struct TouchedBatch {
    /// Trace id (shared across the batch).
    pub trace_id: String,
    /// Span id within the trace.
    pub span_id: String,
    /// Touch entries from the span attribute.
    pub touches: Vec<TouchedAttr>,
    /// Span timestamp.
    pub at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touched_merge_template_has_required_params() {
        for p in ["$tid", "$sid", "$at", "$touches"] {
            assert!(
                TOUCHED_MERGE_CYPHER.contains(p),
                "template missing parameter {p}"
            );
        }
    }

    #[test]
    fn touched_merge_uses_merge_not_create() {
        // Idempotency contract: must MERGE, not CREATE, the :TOUCHED edge.
        assert!(TOUCHED_MERGE_CYPHER.contains("MERGE (s)-[r:TOUCHED"));
        assert!(!TOUCHED_MERGE_CYPHER.contains("CREATE (s)-[r:TOUCHED"));
    }

    #[test]
    fn role_serializes_lowercase() {
        // Span attribute schema (see ADR-011 §D2 example) uses lowercase roles.
        let json = serde_json::to_string(&TouchRole::Generated).unwrap();
        assert_eq!(json, "\"generated\"");
    }
}

/// Cypher MERGE template — proven against live Neo4j 2026-05-23.
///
/// Parameters expected:
///   - `$tid`     trace_id
///   - `$sid`     span_id
///   - `$at`      span timestamp (Neo4j `datetime()` or ISO-8601 string)
///   - `$touches` list of `{artifact_id, role}` maps
///
/// Idempotent: re-running with the same `(span, artifact, role)` triple
/// does not duplicate edges (verified — `count(r) = 1 matched, total unchanged`).
pub const TOUCHED_MERGE_CYPHER: &str = "\
MERGE (s:Span {trace_id: $tid, span_id: $sid})
  ON CREATE SET s.at = $at
WITH s
UNWIND $touches AS t
MATCH (a:Artifact {id: t.artifact_id})
MERGE (s)-[r:TOUCHED {role: t.role}]->(a)
  ON CREATE SET r.ts = $at
RETURN count(r) AS edges_touched";

use crate::Result;

/// Per-touch Cypher — derived from [`TOUCHED_MERGE_CYPHER`] but with a single
/// touch's `$aid`/`$role` bound directly (avoids constructing a BoltList of maps).
/// Idempotency contract is identical.
const PER_TOUCH_MERGE: &str = "\
MERGE (s:Span {trace_id: $tid, span_id: $sid})
  ON CREATE SET s.at = $at
WITH s
MATCH (a:Artifact {id: $aid})
MERGE (s)-[r:TOUCHED {role: $role}]->(a)
  ON CREATE SET r.ts = $at";

/// Materializes `:TOUCHED` edges from heimdall-trace span attributes into Neo4j.
pub struct TouchedMaterializer {
    graph: neo4rs::Graph,
}

impl TouchedMaterializer {
    /// Connect to Neo4j via Bolt URI (e.g., `bolt://localhost:7687`).
    pub async fn connect(uri: &str, user: &str, password: &str) -> Result<Self> {
        let cfg = neo4rs::ConfigBuilder::default()
            .uri(uri)
            .user(user)
            .password(password)
            .db("neo4j")
            .build()
            .map_err(|e| crate::WellError::Provenance(format!("neo4j config: {e}")))?;
        let graph = neo4rs::Graph::connect(cfg)
            .await
            .map_err(|e| crate::WellError::Provenance(format!("neo4j connect: {e}")))?;
        Ok(Self { graph })
    }

    /// Materialize one batch — emits one [`PER_TOUCH_MERGE`] per touch entry.
    /// Returns the number of touch entries processed.
    ///
    /// Idempotent at the Cypher level (MERGE), so safe to retry.
    pub async fn materialize(&self, batch: &TouchedBatch) -> Result<u64> {
        let at_iso = batch.at.to_rfc3339();
        for t in &batch.touches {
            let role = match t.role {
                TouchRole::Used => "used",
                TouchRole::Generated => "generated",
                TouchRole::Refined => "refined",
                TouchRole::Contradicted => "contradicted",
            };
            let q = neo4rs::query(PER_TOUCH_MERGE)
                .param("tid", batch.trace_id.clone())
                .param("sid", batch.span_id.clone())
                .param("at", at_iso.clone())
                .param("aid", t.artifact_id.clone())
                .param("role", role);
            let mut stream = self.graph.execute(q).await?;
            // Drain the result stream so the transaction commits.
            while stream.next().await?.is_some() {}
        }
        Ok(batch.touches.len() as u64)
    }

    /// Test helper — create an :Artifact node so a subsequent materialize()
    /// has a target to MATCH against. Production writer mirrors :Artifact
    /// at write time (V2 work); for V1 + smoke we seed manually.
    pub async fn seed_artifact_for_tests(
        &self,
        id: &str,
        tenant_id: &str,
        tier: &str,
    ) -> Result<()> {
        let q = neo4rs::query(
            "MERGE (a:Artifact {id: $id}) \
             ON CREATE SET a.tenant_id = $tenant, a.tier = $tier",
        )
        .param("id", id)
        .param("tenant", tenant_id)
        .param("tier", tier);
        let mut stream = self.graph.execute(q).await?;
        while stream.next().await?.is_some() {}
        Ok(())
    }

    /// Test helper — delete all :Artifact + :Span where the given test prefix
    /// matches. Refuses non-test prefixes.
    pub async fn purge_test_prefix(&self, prefix: &str) -> Result<u64> {
        if !(prefix.starts_with("01JSMK")
            || prefix.starts_with("smoke")
            || prefix.starts_with("test_"))
        {
            return Err(crate::WellError::TenantScope {
                expected: "01JSMK*/smoke*/test_*".into(),
                got: prefix.into(),
            });
        }
        let q = neo4rs::query(
            "MATCH (n) \
             WHERE (n:Artifact AND n.id STARTS WITH $p) OR (n:Span AND n.trace_id STARTS WITH $p) \
             DETACH DELETE n \
             RETURN count(n) AS deleted",
        )
        .param("p", prefix);
        let mut stream = self.graph.execute(q).await?;
        let mut total: i64 = 0;
        while let Some(row) = stream.next().await? {
            total = row.get("deleted").unwrap_or(0);
        }
        Ok(total as u64)
    }
}
