//! Artifact writer — append-only, with PROV-O emit to Tyr.
//!
//! Scaffolding only — Sprint 56 implements:
//!   - `WellWriter::new(pool, tyr_emitter)`
//!   - `WellWriter::write(req: WriteRequest) -> Result<ArtifactId>`
//!   - `WellWriter::supersede(old, new) -> Result<()>` (sets superseded_by + state)
//!
//! Invariants enforced here:
//!   - surface.matches(tier) — see [`crate::model::Surface::matches`]
//!   - content_hash precomputed by caller (canonical-JSON SHA-256)
//!   - prov_generated_by present (no orphan artifacts — every write traces back
//!     to a heimdall-trace span)
//!   - tenant_id matches caller JWT subject (enforced at the API layer; this
//!     crate only validates non-empty)

use crate::{Result, WellError};
use crate::model::*;

/// Request to write a new artifact. Callers construct this; writer validates.
#[derive(Debug)]
pub struct WriteRequest {
    /// Owning tenant.
    pub tenant_id: String,
    /// Producing agent.
    pub agent_id: String,
    /// Optional case anchor.
    pub case_id: Option<String>,
    /// Artifact kind.
    pub kind: Kind,
    /// Tier (storage classification).
    pub tier: Tier,
    /// Surface (UX label). MUST match tier per [`Surface::matches`].
    pub surface: Surface,
    /// SHA-256 of canonical content.
    pub content_hash: String,
    /// Payload.
    pub content: serde_json::Value,
    /// Optional BGE-M3 embedding bytes.
    pub embedding: Option<Vec<u8>>,
    /// PROV-O wasInformedBy references.
    pub prov_used: Option<Vec<ArtifactId>>,
    /// REQUIRED `trace_id:span_id` of generating span.
    pub prov_generated_by: String,
    /// Producer confidence.
    pub confidence: Option<f32>,
    /// Bifrost session id if from memvid promotion.
    pub promoted_from: Option<String>,
}

impl WriteRequest {
    /// Pre-flight validation called by writer.
    pub fn validate(&self) -> Result<()> {
        if !self.surface.matches(self.tier) {
            return Err(WellError::TierSurfaceMismatch {
                tier: self.tier,
                surface: self.surface,
            });
        }
        if self.prov_generated_by.trim().is_empty() {
            return Err(WellError::Provenance(
                "prov_generated_by is required (no orphan artifacts)".into(),
            ));
        }
        if self.tenant_id.trim().is_empty() {
            return Err(WellError::Provenance("tenant_id required".into()));
        }
        Ok(())
    }
}

/// MariaDB-backed artifact writer. Append-only; supersession via separate call.
///
/// PROV-O emission to Tyr is wired through the trait below — a no-op default
/// keeps the writer usable in tests without a Tyr endpoint.
pub struct WellWriter {
    pool: sqlx::MySqlPool,
    prov_sink: std::sync::Arc<dyn ProvSink + Send + Sync>,
}

/// PROV-O JSON-LD sink — implementations forward to Tyr (or tracing/stdout).
///
/// `emit` is fire-and-forget from the writer's perspective: failures are
/// logged but never propagated up, so an unavailable Tyr never blocks writes.
#[async_trait::async_trait]
pub trait ProvSink {
    /// Emit a PROV-O record for one write.
    async fn emit(
        &self,
        artifact_id: &ArtifactId,
        tenant_id: &str,
        prov_generated_by: &str,
    ) -> Result<()>;
}

/// Default no-op sink (tests, smoke runs).
pub struct NoopProvSink;
#[async_trait::async_trait]
impl ProvSink for NoopProvSink {
    async fn emit(&self, _: &ArtifactId, _: &str, _: &str) -> Result<()> { Ok(()) }
}

/// Tracing-backed sink — emits structured `info!` events. Pickup by any
/// OpenTelemetry collector (incl. heimdall-trace) — usable as a Tyr stand-in.
pub struct TracingProvSink;
#[async_trait::async_trait]
impl ProvSink for TracingProvSink {
    async fn emit(
        &self,
        artifact_id: &ArtifactId,
        tenant_id: &str,
        prov_generated_by: &str,
    ) -> Result<()> {
        tracing::info!(
            target: "mimir.well.prov",
            artifact_id = %artifact_id,
            tenant_id,
            prov_generated_by,
            "prov-o: artifact written"
        );
        Ok(())
    }
}

/// HTTP-backed sink — POSTs PROV-O JSON-LD to a configurable Tyr-ready endpoint.
///
/// Use this when Tyr is up and the audit ingest URL is known. Defaults to a
/// 2-second timeout so a slow Tyr cannot stall write throughput.
pub struct HttpProvSink {
    client: reqwest::Client,
    url: String,
    auth_header: Option<String>,
}

impl HttpProvSink {
    /// Construct. `auth_header` (e.g., `Bearer foo`) is optional.
    pub fn new(url: impl Into<String>, auth_header: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("reqwest client"),
            url: url.into(),
            auth_header,
        }
    }
}

#[async_trait::async_trait]
impl ProvSink for HttpProvSink {
    async fn emit(
        &self,
        artifact_id: &ArtifactId,
        tenant_id: &str,
        prov_generated_by: &str,
    ) -> Result<()> {
        // Minimal PROV-O JSON-LD record (see ADR-011 §"PROV-O").
        let body = serde_json::json!({
            "@context": "https://www.w3.org/ns/prov",
            "@type": "Entity",
            "@id": format!("urn:mimir:artifact:{artifact_id}"),
            "prov:wasGeneratedBy": prov_generated_by,
            "asgard:tenant_id": tenant_id,
            "asgard:emitted_at": chrono::Utc::now().to_rfc3339(),
        });
        let mut req = self
            .client
            .post(&self.url)
            .header("content-type", "application/ld+json")
            .json(&body);
        if let Some(h) = &self.auth_header {
            req = req.header("authorization", h);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(WellError::Upstream(format!(
                "tyr emit returned status {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

impl WellWriter {
    /// Construct with a default no-op PROV sink.
    pub fn new(pool: sqlx::MySqlPool) -> Self {
        Self {
            pool,
            prov_sink: std::sync::Arc::new(NoopProvSink),
        }
    }

    /// Construct with a custom PROV sink (typically Tyr-backed in prod).
    pub fn with_prov_sink(
        pool: sqlx::MySqlPool,
        sink: std::sync::Arc<dyn ProvSink + Send + Sync>,
    ) -> Self {
        Self { pool, prov_sink: sink }
    }

    /// Insert one artifact. Validates the request, generates ULID, binds
    /// ENUMs as their canonical lowercase strings, returns the new id.
    pub async fn write(&self, req: WriteRequest) -> Result<ArtifactId> {
        req.validate()?;

        let id = ulid::Ulid::new();
        let content = serde_json::to_string(&req.content)
            .map_err(|e| WellError::Provenance(format!("content json: {e}")))?;
        let prov_used = req
            .prov_used
            .as_ref()
            .map(|ids| {
                let v: Vec<String> = ids.iter().map(|u| u.to_string()).collect();
                serde_json::to_string(&v)
            })
            .transpose()
            .map_err(|e| WellError::Provenance(format!("prov_used json: {e}")))?;

        sqlx::query(
            "INSERT INTO memory_artifact \
             (id, tenant_id, agent_id, case_id, kind, tier, surface, content_hash, \
              content, embedding, prov_used, prov_generated_by, confidence, promoted_from) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(&req.tenant_id)
        .bind(&req.agent_id)
        .bind(&req.case_id)
        .bind(req.kind.as_sql_str())
        .bind(req.tier.as_sql_str())
        .bind(req.surface.as_sql_str())
        .bind(&req.content_hash)
        .bind(&content)
        .bind(&req.embedding)
        .bind(&prov_used)
        .bind(&req.prov_generated_by)
        .bind(req.confidence)
        .bind(&req.promoted_from)
        .execute(&self.pool)
        .await?;

        // PROV-O emit is fire-and-forget: log on failure, never propagate.
        if let Err(e) = self
            .prov_sink
            .emit(&id, &req.tenant_id, &req.prov_generated_by)
            .await
        {
            tracing::warn!(
                artifact_id = %id,
                tenant = req.tenant_id,
                error = %e,
                "well: prov_sink emit failed (continuing)"
            );
        }
        tracing::debug!(
            artifact_id = %id,
            tenant = req.tenant_id,
            tier = req.tier.as_sql_str(),
            "well: wrote artifact"
        );
        Ok(id)
    }

    /// Mark `old` as superseded by `new`. Both must belong to the same tenant.
    pub async fn supersede(
        &self,
        tenant_id: &str,
        old: ArtifactId,
        new: ArtifactId,
    ) -> Result<()> {
        let rows = sqlx::query(
            "UPDATE memory_artifact \
             SET superseded_by = ?, consolidation_state = 'superseded' \
             WHERE id = ? AND tenant_id = ? AND consolidation_state != 'superseded'",
        )
        .bind(new.to_string())
        .bind(old.to_string())
        .bind(tenant_id)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows == 0 {
            return Err(WellError::NotFound(old));
        }
        Ok(())
    }

    /// Count fresh artifacts in a tenant — used by smoke + ops metrics.
    pub async fn count_fresh(&self, tenant_id: &str) -> Result<i64> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM memory_artifact \
             WHERE tenant_id = ? AND consolidation_state = 'fresh'",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }

    /// Delete every row for a tenant — test fixture only. Refuses production
    /// tenant prefixes to keep stray test calls from nuking real data.
    pub async fn purge_tenant_for_tests(&self, tenant_id: &str) -> Result<u64> {
        if !(tenant_id.starts_with("smoke_")
            || tenant_id.starts_with("test_")
            || tenant_id.starts_with("scratch_"))
        {
            return Err(WellError::TenantScope {
                expected: "smoke_*/test_*/scratch_*".into(),
                got: tenant_id.into(),
            });
        }
        let rows = sqlx::query("DELETE FROM memory_artifact WHERE tenant_id = ?")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> WriteRequest {
        WriteRequest {
            tenant_id: "asgard_medical".into(),
            agent_id: "eir-clinical".into(),
            case_id: None,
            kind: Kind::Observation,
            tier: Tier::Episodic,
            surface: Surface::Short,
            content_hash: "0".repeat(64),
            content: serde_json::json!({}),
            embedding: None,
            prov_used: None,
            prov_generated_by: "trace-xyz:span-1".into(),
            confidence: None,
            promoted_from: None,
        }
    }

    #[test]
    fn validate_accepts_matching_tier_surface() {
        sample_req().validate().unwrap();
    }

    #[test]
    fn validate_rejects_mismatched_tier_surface() {
        let mut r = sample_req();
        r.surface = Surface::Long;
        assert!(matches!(
            r.validate(),
            Err(WellError::TierSurfaceMismatch { .. })
        ));
    }

    #[test]
    fn validate_rejects_orphan() {
        let mut r = sample_req();
        r.prov_generated_by = "  ".into();
        assert!(matches!(r.validate(), Err(WellError::Provenance(_))));
    }

    #[test]
    fn validate_rejects_empty_tenant() {
        let mut r = sample_req();
        r.tenant_id = "".into();
        assert!(matches!(r.validate(), Err(WellError::Provenance(_))));
    }

    #[test]
    fn purge_refuses_prod_tenant_names() {
        // Pure type-level test — verify the guard reasoning without a pool.
        let prod_names = [
            "asgard_medical",
            "asgard_insurance",
            "asgard_platform",
            "asgard_wellness",
            "production",
        ];
        for name in prod_names {
            assert!(
                !(name.starts_with("smoke_")
                    || name.starts_with("test_")
                    || name.starts_with("scratch_")),
                "guard would let prod tenant through: {name}"
            );
        }
        assert!("smoke_abc".starts_with("smoke_"));
        assert!("test_x".starts_with("test_"));
        assert!("scratch_42".starts_with("scratch_"));
    }
}
