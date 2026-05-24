//! B-50e — OCR audit row writer + per-tenant OCR policy lookup.
//!
//! Schema in `migrations/sprint50_syn_skuggi_foundation.sql`:
//! - `ocr_documents` — one row per OCR call (any engine, any outcome).
//! - `tenant_configs` — extended with OCR policy columns (cloud opt-in, PHI strict,
//!   monthly budget cap, Skuggi PII mode).
//!
//! This module exposes the writer + policy reader. Enforcement of the policy
//! (e.g. blocking cloud calls when `ocr_phi_strict=true`) is the smart-router's
//! job (B-50b) — this module just surfaces the values so the router has what
//! it needs without re-querying.

use mimir_core_ai::services::db::DbPool;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tracing::warn;
use uuid::Uuid;

/// Per-tenant OCR policy. Loaded once at the top of an OCR request; passed to
/// the router + audit writer.
///
/// Defaults match the migration's defaults — used when the tenant has no row
/// in `tenant_configs` (which shouldn't happen in prod but is safe in dev).
#[derive(Debug, Clone, Serialize)]
pub struct OcrTenantPolicy {
    pub tenant_id: String,
    /// Hard block: never send to cloud regardless of opt-in. Default: true.
    pub phi_strict: bool,
    /// Tier 2 Gemini 3 Flash opt-in. Default: false.
    pub cloud_flash_enabled: bool,
    /// Tier 3 Gemini 3.1 Pro opt-in (requires flash also enabled). Default: false.
    pub cloud_pro_enabled: bool,
    /// Hard monthly budget cap in USD. 0 = no cap configured (treat as 0 budget).
    pub monthly_budget_usd: f64,
    /// Skuggi PII mode: off | detect-only | mask-and-send | block-on-pii.
    pub pii_mode: String,
}

impl OcrTenantPolicy {
    /// Default policy when tenant_configs row is missing. PHI strict by default —
    /// fail-safe to local-only.
    pub fn safe_default(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            phi_strict: true,
            cloud_flash_enabled: false,
            cloud_pro_enabled: false,
            monthly_budget_usd: 0.0,
            pii_mode: "mask-and-send".to_string(),
        }
    }

    /// Convenience: true if this tenant can call cloud OCR at all.
    pub fn cloud_allowed(&self) -> bool {
        !self.phi_strict && (self.cloud_flash_enabled || self.cloud_pro_enabled)
    }
}

/// Look up the OCR policy for a tenant. Returns a safe default if the row is
/// missing (warn-logged) or any column read fails.
pub async fn get_ocr_policy(pool: &DbPool, tenant_id: &str) -> OcrTenantPolicy {
    let row: Result<Option<(bool, bool, bool, f64, String)>, _> = sqlx::query_as(
        r#"
        SELECT
            ocr_phi_strict,
            ocr_cloud_flash_enabled,
            ocr_cloud_pro_enabled,
            CAST(ocr_monthly_cloud_budget_usd AS DOUBLE) AS monthly,
            pii_mode
        FROM tenant_configs
        WHERE tenant_id = ?
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some((phi_strict, cf, cp, budget, mode))) => OcrTenantPolicy {
            tenant_id: tenant_id.to_string(),
            phi_strict,
            cloud_flash_enabled: cf,
            cloud_pro_enabled: cp,
            monthly_budget_usd: budget,
            pii_mode: mode,
        },
        Ok(None) => {
            warn!(
                "tenant_configs row missing for tenant_id={} — using safe defaults (PHI strict, no cloud)",
                tenant_id
            );
            OcrTenantPolicy::safe_default(tenant_id)
        }
        Err(e) => {
            warn!(
                "tenant_configs lookup failed for tenant_id={}: {} — using safe defaults",
                tenant_id, e
            );
            OcrTenantPolicy::safe_default(tenant_id)
        }
    }
}

/// Outcome status for an OCR call. Matches the `status` enum documented in the
/// schema migration.
#[derive(Debug, Clone, Copy)]
pub enum OcrStatus {
    Succeeded,
    EngineFailed,
    PiiBlocked,
    BudgetExceeded,
    PiiStrictBlock,
}

impl OcrStatus {
    fn as_str(self) -> &'static str {
        match self {
            OcrStatus::Succeeded => "succeeded",
            OcrStatus::EngineFailed => "engine_failed",
            OcrStatus::PiiBlocked => "pii_blocked",
            OcrStatus::BudgetExceeded => "budget_exceeded",
            OcrStatus::PiiStrictBlock => "pii_strict_block",
        }
    }
}

/// Builder for an audit row. Use the builder so callers don't have to remember
/// the 14-column parameter order and can leave optional fields unset.
#[derive(Debug)]
pub struct OcrAuditRow<'a> {
    pub tenant_id: &'a str,
    pub image_bytes: &'a [u8],
    pub engine_used: &'a str,
    pub engine_version: Option<&'a str>,
    pub router_reason: Option<&'a str>,
    pub extracted_text: Option<&'a str>,
    pub confidence: Option<f64>,
    pub bbox_count: Option<i32>,
    pub cost_usd: f64,
    pub latency_ms: Option<i32>,
    pub pii_redacted: bool,
    pub status: OcrStatus,
    pub status_message: Option<&'a str>,
    pub image_path: Option<&'a str>,
    pub requested_by: Option<&'a str>,
    /// ADR-002 Stage 2 — serialized `Vec<OcrRegion>` (region_id, bbox, text,
    /// confidence) JSON. Persisted into `ocr_documents.regions_json` so a
    /// dispute / replay / prompt-regression session can reconstruct exactly
    /// what the OCR engine saw — bbox by bbox — without re-running the engine
    /// against the original image. None for engines without geometry
    /// (Apple Vision today, cloud Gemini text-only) and for any path that
    /// runs before Syn surfaces regions.
    pub regions_json: Option<&'a str>,
}

/// Insert an audit row. Returns the generated UUID (also persists in the row).
///
/// Failures are logged and swallowed — audit writing should never fail the user
/// request. If a row can't be inserted, the OCR call has already produced its
/// result; we keep that result and continue.
pub async fn insert_ocr_audit(pool: &DbPool, row: OcrAuditRow<'_>) -> String {
    let id = Uuid::new_v4().to_string();
    let image_sha256 = hash_image_sha256(row.image_bytes);

    let result = sqlx::query(
        r#"
        INSERT INTO ocr_documents (
            id, tenant_id, image_sha256, image_path,
            engine_used, engine_version, router_reason,
            extracted_text, confidence, bbox_count,
            cost_usd, latency_ms, pii_redacted,
            status, status_message, requested_by,
            regions_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(row.tenant_id)
    .bind(&image_sha256)
    .bind(row.image_path)
    .bind(row.engine_used)
    .bind(row.engine_version)
    .bind(row.router_reason)
    .bind(row.extracted_text)
    .bind(row.confidence)
    .bind(row.bbox_count)
    .bind(row.cost_usd)
    .bind(row.latency_ms)
    .bind(row.pii_redacted)
    .bind(row.status.as_str())
    .bind(row.status_message)
    .bind(row.requested_by)
    .bind(row.regions_json)
    .execute(pool)
    .await;

    if let Err(e) = result {
        warn!(
            "ocr_documents insert failed (tenant={}, engine={}, status={}): {}",
            row.tenant_id,
            row.engine_used,
            row.status.as_str(),
            e
        );
    }

    id
}

/// SHA-256 hex digest of the image bytes. Used as the audit row's
/// `image_sha256` fingerprint — allows dedup detection + replay without
/// retaining the bytes here.
pub fn hash_image_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for byte in digest.iter() {
        // {:02x} → lowercase hex, zero-padded width 2
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_value() {
        // sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            hash_image_sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn safe_default_blocks_cloud() {
        let p = OcrTenantPolicy::safe_default("t1");
        assert!(p.phi_strict);
        assert!(!p.cloud_allowed());
    }

    #[test]
    fn cloud_allowed_only_when_phi_off_and_one_tier_opted_in() {
        let mut p = OcrTenantPolicy::safe_default("t1");
        p.phi_strict = false;
        assert!(!p.cloud_allowed(), "no tier opted in");
        p.cloud_flash_enabled = true;
        assert!(p.cloud_allowed(), "flash opted in");
        p.cloud_flash_enabled = false;
        p.cloud_pro_enabled = true;
        assert!(p.cloud_allowed(), "pro opted in");
        p.phi_strict = true;
        assert!(!p.cloud_allowed(), "phi_strict overrides tiers");
    }
}
