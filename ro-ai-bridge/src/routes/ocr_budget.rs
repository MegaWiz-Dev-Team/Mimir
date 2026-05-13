//! B-50m — OCR cost guard.
//!
//! Two responsibilities:
//!
//! 1. **Aggregation**: `current_month_spend(pool, tenant_id)` rolls up
//!    `ocr_documents.cost_usd` for the current calendar month (from the local
//!    `ocr_documents` table written by B-50e). Used as the live counter for
//!    enforcement.
//!
//! 2. **Pre-call gate**: `check_budget(policy, current_spend, estimated_cost)`
//!    decides if a cloud OCR call may proceed. PHI-strict tenants are blocked
//!    regardless of budget. Local (free) engines always pass.
//!
//! Why SQL aggregation and not Laminar?
//!
//! Hot-path budget enforcement must be deterministic + same-blast-radius as
//! Mimir itself. Laminar is for VISUALIZATION (dashboards, cost drill-down,
//! month-over-month trends) — wire its dashboard link in admin UI separately.
//! If Laminar is degraded, OCR still works correctly per policy.

use mimir_core_ai::services::db::DbPool;
use serde::Serialize;
use std::fmt;
use tracing::warn;

use crate::routes::ocr_audit::OcrTenantPolicy;

/// Sum of `cost_usd` from `ocr_documents` for the given tenant since the
/// start of the current calendar month (UTC). Failed-extraction rows have
/// `cost_usd=0` so they're naturally excluded. Returns 0.0 on DB error
/// (warn-logged) — fail-open here is intentional so a transient DB hiccup
/// doesn't lock out every tenant; the call still gets audited.
pub async fn current_month_spend(pool: &DbPool, tenant_id: &str) -> f64 {
    let row: Result<Option<(Option<f64>,)>, _> = sqlx::query_as(
        r#"
        SELECT CAST(SUM(cost_usd) AS DOUBLE) AS spend
        FROM ocr_documents
        WHERE tenant_id = ?
          AND created_at >= DATE_FORMAT(NOW(), '%Y-%m-01')
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some((Some(spend),))) => spend,
        Ok(_) => 0.0, // no rows this month
        Err(e) => {
            warn!(
                "current_month_spend lookup failed for tenant={}: {} — assuming 0.0 (fail-open)",
                tenant_id, e
            );
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierIntent {
    /// Caller is asking for a Tier 1 local engine (free). Budget check skipped.
    Local,
    /// Caller is asking for a Tier 2/3 cloud engine. Budget check applies.
    Cloud,
    /// Caller didn't pin an engine. Let downstream router decide; budget gate
    /// applies pessimistically (i.e., assume worst case = cloud).
    Unknown,
}

impl TierIntent {
    /// Heuristic from request hints. Mirrors the engine_id_for() mapping in
    /// `ocr.rs` but operates on the user-facing inputs we have before
    /// delegating to Syn.
    pub fn from_hints(engine_override: Option<&str>, high_stakes: bool) -> Self {
        if let Some(eng) = engine_override {
            let e = eng.to_ascii_lowercase();
            if e.contains("paddleocr") || e.contains("typhoon") || e.contains("local") {
                return TierIntent::Local;
            }
            if e.contains("gemini") || e.contains("cloud") {
                return TierIntent::Cloud;
            }
        }
        if high_stakes {
            // Curator-marked high-stakes is the Gemini-Pro path per ADR-006.
            return TierIntent::Cloud;
        }
        // No engine pinned, no high-stakes hint: smart-router default is
        // local PaddleOCR. Treat as Unknown — we'll still pre-check budget
        // because the router CAN escalate to cloud on low-confidence, and
        // we don't want a late 402 mid-flight.
        TierIntent::Unknown
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    /// Pass: not a cloud call, no cap configured, or sufficient headroom.
    Allow,
    /// PHI strict policy hard-blocks cloud regardless of budget.
    BlockedPhiStrict,
    /// Tenant has a cap and current spend + estimated cost would exceed it.
    BlockedBudgetExceeded {
        // Caller can include these in the audit row + 402 body.
    },
}

/// Estimate the USD cost of an upcoming cloud call. Conservative defaults
/// when pricing isn't known (assume worst case Flash = $0.005/page-equivalent
/// to avoid letting an unknown-cost call slip past a small remaining budget).
///
/// Note: real cost is computed AFTER the call from token counts; this is a
/// pre-call upper-bound estimate for the gate decision.
pub fn estimate_pre_call_cost(intent: TierIntent, high_stakes: bool) -> f64 {
    match intent {
        TierIntent::Local => 0.0,
        TierIntent::Cloud => {
            // Pro is ~50x Flash. Pessimistic estimate per ADR-006:
            //   Flash:  ~$0.005 / page
            //   Pro:    ~$0.20 / page (worst case multi-page)
            if high_stakes {
                0.20
            } else {
                0.005
            }
        }
        TierIntent::Unknown => 0.005, // assume cloud-Flash worst case
    }
}

#[derive(Debug)]
pub enum BudgetCheckError {
    PhiStrict,
    BudgetExceeded {
        spent: f64,
        cap: f64,
        estimated: f64,
    },
}

impl fmt::Display for BudgetCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhiStrict => write!(
                f,
                "PHI strict policy: cloud OCR forbidden for this tenant"
            ),
            Self::BudgetExceeded { spent, cap, estimated } => write!(
                f,
                "monthly cloud OCR budget exceeded: spent ${:.4} of ${:.2} cap; this call would add ${:.4}",
                spent, cap, estimated
            ),
        }
    }
}

impl std::error::Error for BudgetCheckError {}

/// Pre-call gate. Local-tier intents always pass. Cloud-tier intents are
/// gated by policy.phi_strict (hard block) and policy.monthly_budget_usd
/// (cap; 0.0 = "no cap configured" per migration default).
pub fn check_budget(
    policy: &OcrTenantPolicy,
    intent: TierIntent,
    current_spend: f64,
    estimated_cost: f64,
) -> Result<(), BudgetCheckError> {
    match intent {
        TierIntent::Local => Ok(()),
        TierIntent::Cloud | TierIntent::Unknown => {
            if policy.phi_strict {
                return Err(BudgetCheckError::PhiStrict);
            }
            // cap = 0.0 means "no cap configured" — Sprint 50 migration default.
            // A tenant who sets cap=0 intentionally should also flip
            // cloud_flash_enabled/cloud_pro_enabled off; the cap-zero case is
            // ambiguous on its own.
            if policy.monthly_budget_usd > 0.0
                && current_spend + estimated_cost >= policy.monthly_budget_usd
            {
                return Err(BudgetCheckError::BudgetExceeded {
                    spent: current_spend,
                    cap: policy.monthly_budget_usd,
                    estimated: estimated_cost,
                });
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with_cap(phi_strict: bool, cap: f64) -> OcrTenantPolicy {
        OcrTenantPolicy {
            tenant_id: "t1".into(),
            phi_strict,
            cloud_flash_enabled: true,
            cloud_pro_enabled: true,
            monthly_budget_usd: cap,
            pii_mode: "mask-and-send".into(),
        }
    }

    #[test]
    fn local_intent_always_passes_regardless_of_cap_or_phi() {
        let p = policy_with_cap(true, 0.0);
        assert!(check_budget(&p, TierIntent::Local, 1000.0, 100.0).is_ok());
    }

    #[test]
    fn cloud_blocked_by_phi_strict() {
        let p = policy_with_cap(true, 100.0);
        assert!(matches!(
            check_budget(&p, TierIntent::Cloud, 0.0, 0.005),
            Err(BudgetCheckError::PhiStrict)
        ));
    }

    #[test]
    fn cloud_passes_when_within_cap() {
        let p = policy_with_cap(false, 10.0);
        assert!(check_budget(&p, TierIntent::Cloud, 5.0, 0.005).is_ok());
    }

    #[test]
    fn cloud_blocked_when_over_cap() {
        let p = policy_with_cap(false, 10.0);
        match check_budget(&p, TierIntent::Cloud, 9.999, 0.005) {
            Err(BudgetCheckError::BudgetExceeded { spent, cap, estimated }) => {
                assert!((spent - 9.999).abs() < 1e-6);
                assert!((cap - 10.0).abs() < 1e-6);
                assert!((estimated - 0.005).abs() < 1e-6);
            }
            other => panic!("expected BudgetExceeded, got {:?}", other),
        }
    }

    #[test]
    fn cap_zero_means_no_cap_configured() {
        let p = policy_with_cap(false, 0.0);
        // even with spend > 0, cap=0 means "no cap" — allow
        assert!(check_budget(&p, TierIntent::Cloud, 999.0, 0.005).is_ok());
    }

    #[test]
    fn exact_boundary_is_rejected() {
        // spend + estimated_cost == cap must trigger BudgetExceeded (not slip through).
        // This is the critical Sprint 54 cloud-enablement gate — the hard stop must
        // fire at exactly the cap boundary, not just past it.
        let p = policy_with_cap(false, 10.0);
        assert!(matches!(
            check_budget(&p, TierIntent::Cloud, 9.995, 0.005),
            Err(BudgetCheckError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn one_cent_under_cap_passes() {
        let p = policy_with_cap(false, 10.0);
        // spend=9.994 + est=0.005 = 9.999 < 10.0 — within cap
        assert!(check_budget(&p, TierIntent::Cloud, 9.994, 0.005).is_ok());
    }

    #[test]
    fn tier_intent_from_hints() {
        assert_eq!(TierIntent::from_hints(Some("paddleocr-local"), false), TierIntent::Local);
        assert_eq!(TierIntent::from_hints(Some("typhoon-local"), false), TierIntent::Local);
        assert_eq!(TierIntent::from_hints(Some("gemini-3-flash"), false), TierIntent::Cloud);
        assert_eq!(TierIntent::from_hints(Some("gemini-3.1-pro"), false), TierIntent::Cloud);
        assert_eq!(TierIntent::from_hints(None, true), TierIntent::Cloud);
        assert_eq!(TierIntent::from_hints(None, false), TierIntent::Unknown);
    }

    #[test]
    fn pre_call_estimate_higher_for_pro_path() {
        assert!(estimate_pre_call_cost(TierIntent::Cloud, true)
            > estimate_pre_call_cost(TierIntent::Cloud, false));
        assert_eq!(estimate_pre_call_cost(TierIntent::Local, false), 0.0);
    }
}
