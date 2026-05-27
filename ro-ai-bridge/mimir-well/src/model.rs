//! Core types — Artifact, Tier, Surface, Kind, ConsolidationState.
//!
//! Mirrors `memory_artifact` table from sprint56_mimir_well_schema.sql.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// ULID identifier for an artifact. Sortable + K-safe.
pub type ArtifactId = Ulid;

/// Tulving 3-tier classification — the primary storage axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Time-anchored events — "what happened in case A23".
    Episodic,
    /// Tenant-level facts — "asgard_medical trusts ESC-2024".
    Semantic,
    /// Decision graphs — "how Underwriter handled BMI>35+smoker".
    Procedural,
}

/// UX-facing surface label — neo4j-labs/agent-memory nomenclature.
/// Maps 1:1 with [`Tier`] but used in UI + MCP tool descriptions
/// because Tulving terms are too academic for analyst users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    /// Surface label for Episodic.
    Short,
    /// Surface label for Semantic.
    Long,
    /// Surface label for Procedural.
    Reasoning,
}

impl Surface {
    /// Required pairing per ADR-011 §D1.
    pub fn matches(self, tier: Tier) -> bool {
        matches!(
            (self, tier),
            (Surface::Short, Tier::Episodic)
                | (Surface::Long, Tier::Semantic)
                | (Surface::Reasoning, Tier::Procedural)
        )
    }

    /// String value matching the `surface` ENUM column.
    pub fn as_sql_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Long => "long",
            Self::Reasoning => "reasoning",
        }
    }
}

impl Tier {
    /// String value matching the `tier` ENUM column.
    pub fn as_sql_str(self) -> &'static str {
        match self {
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Procedural => "procedural",
        }
    }
}

impl Kind {
    /// String value matching the `kind` ENUM column.
    pub fn as_sql_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Abstraction => "abstraction",
            Self::Skill => "skill",
            Self::Correction => "correction",
            Self::Reference => "reference",
        }
    }
}

impl ConsolidationState {
    /// String value matching the `consolidation_state` ENUM column.
    pub fn as_sql_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Reviewed => "reviewed",
            Self::Superseded => "superseded",
            Self::Contradicted => "contradicted",
        }
    }
}

impl From<Tier> for Surface {
    fn from(tier: Tier) -> Self {
        match tier {
            Tier::Episodic => Surface::Short,
            Tier::Semantic => Surface::Long,
            Tier::Procedural => Surface::Reasoning,
        }
    }
}

/// Artifact taxonomy from PROV-AGENT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Raw observation captured during a session.
    Observation,
    /// Derived generalization across observations.
    Abstraction,
    /// Procedural know-how (sequence of steps).
    Skill,
    /// Curator/user correction applied on top of a prior artifact.
    Correction,
    /// Pointer to external/canonical source (guideline, codeset).
    Reference,
}

/// Consolidation lifecycle — drives mimir-curator queue routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsolidationState {
    /// Newly written, awaiting consolidator pass.
    Fresh,
    /// Curator reviewed (kept_both decision).
    Reviewed,
    /// Replaced by a newer artifact (`superseded_by` filled).
    Superseded,
    /// Active contradiction logged against another artifact.
    Contradicted,
}

fn parse_tier(s: &str) -> Result<Tier, String> {
    match s {
        "episodic" => Ok(Tier::Episodic),
        "semantic" => Ok(Tier::Semantic),
        "procedural" => Ok(Tier::Procedural),
        other => Err(format!("unknown tier: {other}")),
    }
}
fn parse_surface(s: &str) -> Result<Surface, String> {
    match s {
        "short" => Ok(Surface::Short),
        "long" => Ok(Surface::Long),
        "reasoning" => Ok(Surface::Reasoning),
        other => Err(format!("unknown surface: {other}")),
    }
}
fn parse_kind(s: &str) -> Result<Kind, String> {
    match s {
        "observation" => Ok(Kind::Observation),
        "abstraction" => Ok(Kind::Abstraction),
        "skill" => Ok(Kind::Skill),
        "correction" => Ok(Kind::Correction),
        "reference" => Ok(Kind::Reference),
        other => Err(format!("unknown kind: {other}")),
    }
}
fn parse_consolidation(s: &str) -> Result<ConsolidationState, String> {
    match s {
        "fresh" => Ok(ConsolidationState::Fresh),
        "reviewed" => Ok(ConsolidationState::Reviewed),
        "superseded" => Ok(ConsolidationState::Superseded),
        "contradicted" => Ok(ConsolidationState::Contradicted),
        other => Err(format!("unknown consolidation_state: {other}")),
    }
}

/// In-memory representation of a `memory_artifact` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// ULID primary key.
    pub id: ArtifactId,
    /// Owning tenant.
    pub tenant_id: String,
    /// Producing agent (e.g., `eir-clinical`, `underwriter`).
    pub agent_id: String,
    /// Optional case/session anchor.
    pub case_id: Option<String>,
    /// Kind per [`Kind`].
    pub kind: Kind,
    /// Storage tier.
    pub tier: Tier,
    /// UX surface label.
    pub surface: Surface,
    /// SHA-256 of canonical content — auto-merge key.
    pub content_hash: String,
    /// Artifact payload (structure varies by kind).
    pub content: serde_json::Value,
    /// BGE-M3 embedding (raw bytes). None until embedded.
    pub embedding: Option<Vec<u8>>,
    /// PROV-O `used` references.
    pub prov_used: Option<Vec<ArtifactId>>,
    /// `trace_id:span_id` of generating activity.
    pub prov_generated_by: Option<String>,
    /// Producer confidence [0.000, 1.000].
    pub confidence: Option<f32>,
    /// Bifrost session id if promoted from memvid scratchpad.
    pub promoted_from: Option<String>,
    /// Consolidation lifecycle state.
    pub consolidation_state: ConsolidationState,
    /// Replacement artifact (when consolidation_state = Superseded).
    pub superseded_by: Option<ArtifactId>,
    /// Insert timestamp (UTC).
    pub created_at: DateTime<Utc>,
}

impl Artifact {
    /// Construct from a `memory_artifact` row. Returns an error if any ENUM
    /// or ULID column is malformed (which would indicate schema/code drift).
    pub fn from_row(row: &sqlx::mysql::MySqlRow) -> std::result::Result<Self, String> {
        use sqlx::Row;
        let id_s: String = row.try_get("id").map_err(|e| e.to_string())?;
        let id = id_s.parse::<ArtifactId>().map_err(|e| format!("id: {e}"))?;
        let tier_s: String = row.try_get("tier").map_err(|e| e.to_string())?;
        let surface_s: String = row.try_get("surface").map_err(|e| e.to_string())?;
        let kind_s: String = row.try_get("kind").map_err(|e| e.to_string())?;
        let cs_s: String = row
            .try_get("consolidation_state")
            .map_err(|e| e.to_string())?;
        // MariaDB JSON columns surface as BLOB via sqlx — decode bytes → str → JSON.
        let content_b: Vec<u8> = row.try_get("content").map_err(|e| e.to_string())?;
        let content: serde_json::Value =
            serde_json::from_slice(&content_b).map_err(|e| format!("content: {e}"))?;
        let prov_used_b: Option<Vec<u8>> =
            row.try_get("prov_used").map_err(|e| e.to_string())?;
        let prov_used = prov_used_b
            .map(|b| -> std::result::Result<Vec<ArtifactId>, String> {
                let v: Vec<String> =
                    serde_json::from_slice(&b).map_err(|e| format!("prov_used json: {e}"))?;
                v.into_iter()
                    .map(|x| x.parse::<ArtifactId>().map_err(|e| format!("ulid: {e}")))
                    .collect()
            })
            .transpose()?;
        let superseded_by_s: Option<String> =
            row.try_get("superseded_by").map_err(|e| e.to_string())?;
        let superseded_by = superseded_by_s
            .map(|s| s.parse::<ArtifactId>().map_err(|e| format!("ulid: {e}")))
            .transpose()?;

        Ok(Artifact {
            id,
            tenant_id: row.try_get("tenant_id").map_err(|e| e.to_string())?,
            agent_id: row.try_get("agent_id").map_err(|e| e.to_string())?,
            case_id: row.try_get("case_id").map_err(|e| e.to_string())?,
            kind: parse_kind(&kind_s)?,
            tier: parse_tier(&tier_s)?,
            surface: parse_surface(&surface_s)?,
            content_hash: row.try_get("content_hash").map_err(|e| e.to_string())?,
            content,
            embedding: row.try_get("embedding").map_err(|e| e.to_string())?,
            prov_used,
            prov_generated_by: row.try_get("prov_generated_by").map_err(|e| e.to_string())?,
            confidence: {
                // DECIMAL(4,3) needs CAST AS DOUBLE in the query (see SELECT_COLUMNS)
                // — sqlx-mysql lacks native DECIMAL support without the bigdecimal feature.
                let v: Option<f64> = row.try_get("confidence").map_err(|e| e.to_string())?;
                v.map(|x| x as f32)
            },
            promoted_from: row.try_get("promoted_from").map_err(|e| e.to_string())?,
            consolidation_state: parse_consolidation(&cs_s)?,
            superseded_by,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_tier_pairing() {
        assert!(Surface::Short.matches(Tier::Episodic));
        assert!(Surface::Long.matches(Tier::Semantic));
        assert!(Surface::Reasoning.matches(Tier::Procedural));
        assert!(!Surface::Short.matches(Tier::Semantic));
        assert!(!Surface::Long.matches(Tier::Procedural));
        assert!(!Surface::Reasoning.matches(Tier::Episodic));
    }

    #[test]
    fn surface_from_tier_is_canonical() {
        for t in [Tier::Episodic, Tier::Semantic, Tier::Procedural] {
            assert!(Surface::from(t).matches(t));
        }
    }

    #[test]
    fn enum_sql_strings_match_serde() {
        // as_sql_str() and serde lowercase form must agree — keeps schema
        // ENUM and JSON wire format in lock-step.
        for t in [Tier::Episodic, Tier::Semantic, Tier::Procedural] {
            let json = serde_json::to_value(t).unwrap();
            assert_eq!(t.as_sql_str(), json.as_str().unwrap());
        }
        for s in [Surface::Short, Surface::Long, Surface::Reasoning] {
            let json = serde_json::to_value(s).unwrap();
            assert_eq!(s.as_sql_str(), json.as_str().unwrap());
        }
        for k in [
            Kind::Observation,
            Kind::Abstraction,
            Kind::Skill,
            Kind::Correction,
            Kind::Reference,
        ] {
            let json = serde_json::to_value(k).unwrap();
            assert_eq!(k.as_sql_str(), json.as_str().unwrap());
        }
        for c in [
            ConsolidationState::Fresh,
            ConsolidationState::Reviewed,
            ConsolidationState::Superseded,
            ConsolidationState::Contradicted,
        ] {
            let json = serde_json::to_value(c).unwrap();
            assert_eq!(c.as_sql_str(), json.as_str().unwrap());
        }
    }
}
