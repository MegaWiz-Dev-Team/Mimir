//! Clinical safety pruner — medication safety (DDI + contraindication).
//!
//! Sits between the LLM's medication suggestion and the clinician. For a proposed
//! drug + patient context (current drugs, conditions), it resolves each name to a
//! PrimeKG node (via the normalizer + `primekg_lookup_entity`) and checks for a
//! `DRUG_DRUG` / `CONTRAINDICATION` edge. Any hit is flagged with KG provenance.
//!
//! DESIGN NOTES (from live testing against real PrimeKG):
//! - Entity resolution — not the graph query — is the dominant failure mode, so
//!   every name goes through `DrugDiseaseNormalizer` first (brand/lay -> canonical).
//! - `Decision::Unresolved` is a SAFETY state: if a name can't be mapped to a
//!   PrimeKG node we cannot verify it, so it must surface to the clinician and
//!   NEVER be reported as a clean pass.
//! - PrimeKG `DRUG_DRUG` carries no severity. Findings therefore have no severity
//!   yet; a DDInter-sourced severity gate (eval-only, never in the product KG)
//!   is layered on later to cut over-prune.

use crate::normalizer::{DrugDiseaseNormalizer, EntityKind};
use anyhow::Result;
use mimir_core_ai::services::neo4j::Neo4jService;

/// Patient state the proposed drug is checked against.
#[derive(Debug, Clone, Default)]
pub struct PatientContext {
    pub current_drugs: Vec<String>,
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    DrugDrug,
    Contraindication,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: FindingKind,
    pub proposed_drug: String,
    pub against: String,           // context drug/condition as written
    pub resolved_proposed: String, // canonical PrimeKG name matched
    pub resolved_against: String,
    pub kg_source: String,         // edge provenance
}

/// Outcome of a pruner check. `Unresolved` is a SAFETY state, not a pass.
#[derive(Debug, Clone)]
pub enum Decision {
    Pass,
    Flag(Vec<Finding>),
    Unresolved(Vec<String>),
}

pub struct PrimeKgPruner<'a> {
    neo4j: &'a Neo4jService,
    normalizer: DrugDiseaseNormalizer,
}

impl<'a> PrimeKgPruner<'a> {
    pub fn new(neo4j: &'a Neo4jService) -> Self {
        Self {
            neo4j,
            normalizer: DrugDiseaseNormalizer::seed(),
        }
    }

    /// Resolve a name to (entity_index, canonical_name). Normalizes first
    /// (brand/lay -> PrimeKG-canonical), then `primekg_lookup_entity`.
    async fn resolve(&self, name: &str, kind: EntityKind) -> Result<Option<(i64, String)>> {
        let canon = self.normalizer.canonical(name, kind);
        let ty = match kind {
            EntityKind::Drug => "drug",
            EntityKind::Disease => "disease",
        };
        let hits = self
            .neo4j
            .primekg_lookup_entity(canon.as_ref(), Some(ty), 1)
            .await?;
        Ok(hits.first().and_then(|v| {
            let idx = v.get("entity_index").and_then(|x| x.as_i64())?;
            let nm = v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or(name)
                .to_string();
            Some((idx, nm))
        }))
    }

    /// Check a proposed drug against the patient's current drugs + conditions.
    pub async fn check(&self, proposed_drug: &str, ctx: &PatientContext) -> Result<Decision> {
        let Some((p_idx, p_name)) = self.resolve(proposed_drug, EntityKind::Drug).await? else {
            // Can't identify the proposed drug → cannot verify anything.
            return Ok(Decision::Unresolved(vec![proposed_drug.to_string()]));
        };

        let mut findings = Vec::new();
        let mut unresolved = Vec::new();

        for d in &ctx.current_drugs {
            match self.resolve(d, EntityKind::Drug).await? {
                Some((d_idx, d_name)) => {
                    if let Some(src) = self
                        .neo4j
                        .primekg_relation_exists(p_idx, d_idx, "DRUG_DRUG")
                        .await?
                    {
                        findings.push(Finding {
                            kind: FindingKind::DrugDrug,
                            proposed_drug: proposed_drug.to_string(),
                            against: d.clone(),
                            resolved_proposed: p_name.clone(),
                            resolved_against: d_name,
                            kg_source: src,
                        });
                    }
                }
                None => unresolved.push(d.clone()),
            }
        }

        for c in &ctx.conditions {
            match self.resolve(c, EntityKind::Disease).await? {
                Some((c_idx, c_name)) => {
                    if let Some(src) = self
                        .neo4j
                        .primekg_relation_exists(p_idx, c_idx, "CONTRAINDICATION")
                        .await?
                    {
                        findings.push(Finding {
                            kind: FindingKind::Contraindication,
                            proposed_drug: proposed_drug.to_string(),
                            against: c.clone(),
                            resolved_proposed: p_name.clone(),
                            resolved_against: c_name,
                            kg_source: src,
                        });
                    }
                }
                None => unresolved.push(c.clone()),
            }
        }

        if !findings.is_empty() {
            Ok(Decision::Flag(findings))
        } else if !unresolved.is_empty() {
            // Nothing flagged, but some entities were not verifiable — surface it,
            // do NOT report a clean pass.
            Ok(Decision::Unresolved(unresolved))
        } else {
            Ok(Decision::Pass)
        }
    }
}
