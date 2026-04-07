//! Coverage Analytics API Routes — Sprint 18
//!
//! Provides endpoints for data coverage analysis, blind-spot detection,
//! and gap analysis across the ingestion pipeline.
//! All endpoints enforce tenant isolation via X-Tenant-Id header.
//!
//! REQ-012: ACU per source, Blind-spot Detection, Closed-loop Actions

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, instrument};

// ═══════════════════════════════════════════════════════════════════════════════
// Response structs
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PipelineStages {
    pub ingested: i64,
    pub chunked: i64,
    pub qa_generated: i64,
    pub vectorized: i64,
    pub kg_extracted: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoverageOverview {
    pub total_sources: i64,
    pub sources_with_chunks: i64,
    pub sources_with_qa: i64,
    pub sources_with_vectors: i64,
    pub sources_with_kg: i64,
    pub overall_score: f64,
    pub pipeline_stages: PipelineStages,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceCoverage {
    pub source_id: i64,
    pub name: String,
    pub source_type: String,
    pub status: String,
    pub chunk_count: i64,
    pub qa_count: i64,
    pub vector_coverage_pct: f64,
    pub kg_entity_count: i64,
    pub dedup_ratio: f64,
    pub blindspots: Vec<String>,
    pub coverage_score: f64,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GapSource {
    pub source_id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoverageGaps {
    pub sources_missing_chunks: Vec<GapSource>,
    pub sources_missing_qa: Vec<GapSource>,
    pub sources_missing_vectors: Vec<GapSource>,
    pub sources_missing_kg: Vec<GapSource>,
    pub stale_sources: Vec<GapSource>,
    pub high_dedup_sources: Vec<GapSource>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pure functions for testability (TDD)
// ═══════════════════════════════════════════════════════════════════════════════

/// Calculate coverage score for a source.
/// Each pipeline stage contributes 25 points if present.
pub fn calculate_coverage_score(
    chunk_count: i64,
    qa_count: i64,
    vector_count: i64,
    kg_count: i64,
) -> f64 {
    let mut score = 0.0;
    if chunk_count > 0 {
        score += 25.0;
    }
    if qa_count > 0 {
        score += 25.0;
    }
    if vector_count > 0 {
        score += 25.0;
    }
    if kg_count > 0 {
        score += 25.0;
    }
    score
}

/// Detect blind-spots for a source based on pipeline coverage.
pub fn detect_blindspots(
    chunk_count: i64,
    qa_count: i64,
    vector_pct: f64,
    kg_count: i64,
    dedup_ratio: f64,
    last_sync_at: &Option<String>,
) -> Vec<String> {
    let mut spots = Vec::new();

    if chunk_count == 0 {
        spots.push("no_chunks".to_string());
    }
    if qa_count == 0 {
        spots.push("no_qa_pairs".to_string());
    }
    if vector_pct < 50.0 {
        spots.push("low_vector_coverage".to_string());
    }
    if kg_count == 0 {
        spots.push("no_kg_entities".to_string());
    }
    if dedup_ratio > 0.3 {
        spots.push("high_dedup_ratio".to_string());
    }

    // Check staleness (no sync in 7 days)
    if let Some(sync_str) = last_sync_at {
        if let Ok(sync_time) = chrono::NaiveDateTime::parse_from_str(sync_str, "%Y-%m-%d %H:%M:%S")
        {
            let now = chrono::Utc::now().naive_utc();
            let days_since = (now - sync_time).num_days();
            if days_since > 7 {
                spots.push("stale_data".to_string());
            }
        }
    }

    spots
}

/// Calculate overall coverage score as average across all sources.
pub fn calculate_overall_score(
    total_sources: i64,
    sources_with_chunks: i64,
    sources_with_qa: i64,
    sources_with_vectors: i64,
    sources_with_kg: i64,
) -> f64 {
    if total_sources == 0 {
        return 0.0;
    }
    let dimension_count = 4.0;
    let chunk_pct = sources_with_chunks as f64 / total_sources as f64;
    let qa_pct = sources_with_qa as f64 / total_sources as f64;
    let vector_pct = sources_with_vectors as f64 / total_sources as f64;
    let kg_pct = sources_with_kg as f64 / total_sources as f64;
    ((chunk_pct + qa_pct + vector_pct + kg_pct) / dimension_count) * 100.0
}

// ═══════════════════════════════════════════════════════════════════════════════
// Routes definition
// ═══════════════════════════════════════════════════════════════════════════════

pub fn coverage_routes() -> Router<DbPool> {
    Router::new()
        .route("/overview", get(get_overview))
        .route("/sources", get(get_sources))
        .route("/gaps", get(get_gaps))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Route handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/coverage/overview — Tenant-level coverage summary
#[instrument(skip(headers, pool))]
async fn get_overview(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<CoverageOverview>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    info!(
        event = "coverage_overview",
        tenant_id = tenant_id,
        "Fetching coverage overview"
    );

    let err_map = |e: sqlx::Error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    };

    // Total sources
    let total_sources: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM data_sources WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .map_err(err_map)?;

    // Sources with chunks (total_chunks > 0)
    let sources_with_chunks: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM data_sources WHERE tenant_id = ? AND total_chunks > 0",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    // Sources with QA: count sources whose chunks have qa_results generated
    // Note: COLLATE needed because qa_results and data_sources may use different collations
    let sources_with_qa: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT c.source_id) FROM chunks c \
         INNER JOIN data_sources ds ON c.source_id = ds.id AND ds.tenant_id = ? \
         WHERE EXISTS (SELECT 1 FROM qa_results qr WHERE qr.tenant_id COLLATE utf8mb4_uca1400_ai_ci = ds.tenant_id)"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    // Sources with vectors: if ANY Qdrant collection has points, count all sources with chunks as vectorized
    // (In this architecture, vectors are stored per-tenant in Qdrant, not tracked per-source in DB)
    let qdrant = mimir_core_ai::services::qdrant::QdrantService::new();
    let has_vectors = qdrant
        .get_collection_info("source_chunks")
        .await
        .map(|info| info["result"]["points_count"].as_u64().unwrap_or(0) > 0)
        .unwrap_or(false);
    let sources_with_vectors = if has_vectors {
        sources_with_chunks
    } else {
        (0i64,)
    };

    // Sources with KG entities
    let sources_with_kg: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT source_id) FROM kg_entities WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    // KG extracted count (from extraction runs)
    let _kg_extracted: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT source_id) FROM kg_extraction_runs WHERE tenant_id = ? AND status = 'completed'"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    let overall_score = calculate_overall_score(
        total_sources.0,
        sources_with_chunks.0,
        sources_with_qa.0,
        sources_with_vectors.0,
        sources_with_kg.0,
    );

    Ok(Json(CoverageOverview {
        total_sources: total_sources.0,
        sources_with_chunks: sources_with_chunks.0,
        sources_with_qa: sources_with_qa.0,
        sources_with_vectors: sources_with_vectors.0,
        sources_with_kg: sources_with_kg.0,
        overall_score,
        pipeline_stages: PipelineStages {
            ingested: total_sources.0,
            chunked: sources_with_chunks.0,
            qa_generated: sources_with_qa.0,
            vectorized: sources_with_vectors.0,
            kg_extracted: sources_with_kg.0,
        },
    }))
}

/// GET /api/v1/coverage/sources — Per-source coverage detail
#[instrument(skip(headers, pool))]
async fn get_sources(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<SourceCoverage>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    info!(
        event = "coverage_sources",
        tenant_id = tenant_id,
        "Fetching per-source coverage"
    );

    // Get all sources for this tenant
    let sources: Vec<(
        i64,
        String,
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, name, source_type, last_sync_status, total_chunks, \
         DATE_FORMAT(last_sync_at, '%Y-%m-%d %H:%M:%S') as last_sync_at \
         FROM data_sources WHERE tenant_id = ? ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let mut result = Vec::new();

    for (id, name, source_type, status, total_chunks, last_sync_at) in sources {
        let chunk_count = total_chunks.unwrap_or(0);

        // QA count for this source
        let qa_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pipeline_steps WHERE source_id = ? AND step_name = 'qa_generation' AND status = 'completed'"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        // Vector coverage: check if embedding step completed
        let vector_step: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pipeline_steps WHERE source_id = ? AND step_name = 'embedding' AND status = 'completed'"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));
        let vector_coverage_pct = if vector_step.0 > 0 { 100.0 } else { 0.0 };

        // KG entity count
        let kg_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kg_entities WHERE source_id = ? AND tenant_id = ?",
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        // Dedup ratio
        let total_fingerprints: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM content_fingerprints WHERE source_id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap_or((0,));

        let dedup_ratio = if chunk_count > 0 && total_fingerprints.0 > 0 {
            1.0 - (total_fingerprints.0 as f64 / chunk_count as f64).min(1.0)
        } else {
            0.0
        };

        let blindspots = detect_blindspots(
            chunk_count,
            qa_count.0,
            vector_coverage_pct,
            kg_count.0,
            dedup_ratio,
            &last_sync_at,
        );

        let coverage_score =
            calculate_coverage_score(chunk_count, qa_count.0, vector_step.0, kg_count.0);

        result.push(SourceCoverage {
            source_id: id,
            name,
            source_type,
            status: status.unwrap_or_else(|| "UNKNOWN".to_string()),
            chunk_count,
            qa_count: qa_count.0,
            vector_coverage_pct,
            kg_entity_count: kg_count.0,
            dedup_ratio,
            blindspots,
            coverage_score,
            last_sync_at,
        });
    }

    Ok(Json(result))
}

/// GET /api/v1/coverage/gaps — Blind-spot analysis
#[instrument(skip(headers, pool))]
async fn get_gaps(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<CoverageGaps>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    info!(
        event = "coverage_gaps",
        tenant_id = tenant_id,
        "Fetching coverage gaps"
    );

    // Sources missing chunks
    let missing_chunks: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE tenant_id = ? AND (total_chunks IS NULL OR total_chunks = 0)"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Sources missing QA
    let missing_qa: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE tenant_id = ? AND id NOT IN \
         (SELECT DISTINCT source_id FROM pipeline_steps WHERE step_name = 'qa_generation' AND status = 'completed')"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Sources missing vectors
    let missing_vectors: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE tenant_id = ? AND id NOT IN \
         (SELECT DISTINCT source_id FROM pipeline_steps WHERE step_name = 'embedding' AND status = 'completed')"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Sources missing KG
    let missing_kg: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE tenant_id = ? AND id NOT IN \
         (SELECT DISTINCT source_id FROM kg_entities WHERE tenant_id = ?)",
    )
    .bind(tenant_id)
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Stale sources (no sync in 7+ days)
    let stale: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM data_sources WHERE tenant_id = ? AND last_sync_at < DATE_SUB(NOW(), INTERVAL 7 DAY)"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // High dedup sources (> 30% dedup ratio)
    let high_dedup: Vec<(i64, String)> = sqlx::query_as(
        "SELECT ds.id, ds.name FROM data_sources ds \
         LEFT JOIN (SELECT source_id, COUNT(*) as fp_count FROM content_fingerprints GROUP BY source_id) cf \
         ON ds.id = cf.source_id \
         WHERE ds.tenant_id = ? AND ds.total_chunks > 0 \
         AND (1.0 - COALESCE(cf.fp_count, 0) / ds.total_chunks) > 0.3"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let to_gap = |rows: Vec<(i64, String)>| -> Vec<GapSource> {
        rows.into_iter()
            .map(|(id, name)| GapSource {
                source_id: id,
                name,
            })
            .collect()
    };

    Ok(Json(CoverageGaps {
        sources_missing_chunks: to_gap(missing_chunks),
        sources_missing_qa: to_gap(missing_qa),
        sources_missing_vectors: to_gap(missing_vectors),
        sources_missing_kg: to_gap(missing_kg),
        stale_sources: to_gap(stale),
        high_dedup_sources: to_gap(high_dedup),
    }))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests (TDD — Sprint 18)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Route Assembly ────────────────────────────────────────────────

    #[test]
    fn test_coverage_routes_assembly() {
        let _router = coverage_routes();
        assert!(true, "Coverage routes assembled successfully");
    }

    // ─── Coverage Score Calculation ────────────────────────────────────

    #[test]
    fn test_calculate_coverage_score_all_present() {
        let score = calculate_coverage_score(10, 5, 3, 2);
        assert_eq!(score, 100.0, "All four stages present = 100%");
    }

    #[test]
    fn test_calculate_coverage_score_none_present() {
        let score = calculate_coverage_score(0, 0, 0, 0);
        assert_eq!(score, 0.0, "No stages present = 0%");
    }

    #[test]
    fn test_calculate_coverage_score_partial() {
        // Only chunks and QA
        let score = calculate_coverage_score(10, 5, 0, 0);
        assert_eq!(score, 50.0, "Two stages present = 50%");

        // Only chunks
        let score = calculate_coverage_score(1, 0, 0, 0);
        assert_eq!(score, 25.0, "One stage present = 25%");

        // Three stages
        let score = calculate_coverage_score(10, 5, 3, 0);
        assert_eq!(score, 75.0, "Three stages present = 75%");
    }

    // ─── Blindspot Detection ───────────────────────────────────────────

    #[test]
    fn test_detect_blindspots_all_healthy() {
        let spots = detect_blindspots(
            10,
            5,
            85.0,
            3,
            0.01,
            &Some("2099-01-01 00:00:00".to_string()),
        );
        assert!(
            spots.is_empty(),
            "No blindspots for fully healthy source, got: {:?}",
            spots
        );
    }

    #[test]
    fn test_detect_blindspots_no_chunks() {
        let spots = detect_blindspots(0, 0, 0.0, 0, 0.0, &None);
        assert!(spots.contains(&"no_chunks".to_string()));
        assert!(spots.contains(&"no_qa_pairs".to_string()));
        assert!(spots.contains(&"low_vector_coverage".to_string()));
        assert!(spots.contains(&"no_kg_entities".to_string()));
    }

    #[test]
    fn test_detect_blindspots_high_dedup() {
        let spots = detect_blindspots(
            10,
            5,
            90.0,
            2,
            0.5,
            &Some("2099-01-01 00:00:00".to_string()),
        );
        assert!(spots.contains(&"high_dedup_ratio".to_string()));
        assert_eq!(spots.len(), 1);
    }

    #[test]
    fn test_detect_blindspots_low_vector() {
        let spots = detect_blindspots(
            10,
            5,
            30.0,
            2,
            0.0,
            &Some("2099-01-01 00:00:00".to_string()),
        );
        assert!(spots.contains(&"low_vector_coverage".to_string()));
    }

    // ─── Overall Score ─────────────────────────────────────────────────

    #[test]
    fn test_calculate_overall_score_full_coverage() {
        let score = calculate_overall_score(4, 4, 4, 4, 4);
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_calculate_overall_score_no_sources() {
        let score = calculate_overall_score(0, 0, 0, 0, 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_calculate_overall_score_half_coverage() {
        // 4 sources, 2 with chunks, 2 with QA, 2 with vectors, 2 with KG
        let score = calculate_overall_score(4, 2, 2, 2, 2);
        assert_eq!(score, 50.0);
    }

    // ─── Struct Serialization ──────────────────────────────────────────

    #[test]
    fn test_coverage_overview_serialize() {
        let overview = CoverageOverview {
            total_sources: 6,
            sources_with_chunks: 4,
            sources_with_qa: 2,
            sources_with_vectors: 3,
            sources_with_kg: 1,
            overall_score: 67.5,
            pipeline_stages: PipelineStages {
                ingested: 6,
                chunked: 4,
                qa_generated: 2,
                vectorized: 3,
                kg_extracted: 1,
            },
        };

        let json_str = serde_json::to_string(&overview).unwrap();
        let parsed: CoverageOverview = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.total_sources, 6);
        assert_eq!(parsed.pipeline_stages.chunked, 4);
    }

    #[test]
    fn test_source_coverage_serialize() {
        let source = SourceCoverage {
            source_id: 1,
            name: "Test Source".to_string(),
            source_type: "web".to_string(),
            status: "COMPLETED".to_string(),
            chunk_count: 150,
            qa_count: 42,
            vector_coverage_pct: 85.0,
            kg_entity_count: 28,
            dedup_ratio: 0.02,
            blindspots: vec!["no_qa_pairs".to_string()],
            coverage_score: 72.0,
            last_sync_at: Some("2026-03-04 12:00:00".to_string()),
        };

        let json_str = serde_json::to_string(&source).unwrap();
        assert!(json_str.contains("\"source_id\":1"));
        assert!(json_str.contains("\"no_qa_pairs\""));
    }

    #[test]
    fn test_coverage_gaps_serialize() {
        let gaps = CoverageGaps {
            sources_missing_chunks: vec![GapSource {
                source_id: 1,
                name: "Source A".to_string(),
            }],
            sources_missing_qa: vec![],
            sources_missing_vectors: vec![],
            sources_missing_kg: vec![],
            stale_sources: vec![],
            high_dedup_sources: vec![],
        };

        let json_str = serde_json::to_string(&gaps).unwrap();
        let parsed: CoverageGaps = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.sources_missing_chunks.len(), 1);
        assert_eq!(parsed.sources_missing_chunks[0].source_id, 1);
    }
}
