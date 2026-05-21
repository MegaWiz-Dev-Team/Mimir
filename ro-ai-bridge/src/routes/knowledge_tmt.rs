//! TMT/TMLT relationship-traversal endpoints — drug & lab graph reasoning.
//!
//! The value of the TMT relationship graph (167K edges across the 8-layer
//! dm+d hierarchy SUBS→VTM→GP→{GPU,TP}→…) is *reasoning*, not flat lookup:
//!   - any concept → its generic (GP) and active substance (SUBS)
//!   - generic → all trade products (TP) sharing it = substitution candidates
//! TMLT has a single PANELtoITEM relationship (e.g. CBC → WBC/RBC/Hgb/…).
//!
//!   POST /api/v1/knowledge/tmt/resolve   — drug graph neighborhood
//!   POST /api/v1/knowledge/tmlt/expand   — panel ⇄ item expansion
//!
//! These complement the flat FULLTEXT search at /api/v1/knowledge/search.
//! IDs are validated as strictly alphanumeric (TMT/TMLT id space) so they can
//! be interpolated into SQL safely, matching the codebase's format! style.

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use tracing::warn;

pub fn knowledge_tmt_routes() -> Router<DbPool> {
    Router::new().route("/resolve", post(tmt_resolve))
}

pub fn knowledge_tmlt_routes() -> Router<DbPool> {
    Router::new().route("/expand", post(tmlt_expand))
}

type RouteError = (StatusCode, Json<JsonValue>);

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn bad_id() -> RouteError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_id", "hint": "id must be alphanumeric, ≤20 chars"})),
    )
}

fn db_error(e: sqlx::Error) -> RouteError {
    warn!("tmt/tmlt traversal query error: {e}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "query_failed", "detail": e.to_string()})),
    )
}

fn not_found(id: &str) -> RouteError {
    (StatusCode::NOT_FOUND, Json(json!({"error": "not_found", "id": id})))
}

// ─── TMT resolve ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ResolveReq {
    tmt_id: String,
    /// BFS depth over the (undirected) relationship graph. Capped at 6.
    #[serde(default = "default_hops")]
    max_hops: u32,
}
fn default_hops() -> u32 {
    4
}

/// Given any TMT concept, walk the relationship graph and return its drug
/// neighborhood grouped by layer. The key outputs for callers (Eir
/// substitution, insurance NLEM matching, Syn Rx OCR normalization) are
/// `substances`, `generics`, and `brand_alternatives`.
async fn tmt_resolve(
    State(pool): State<DbPool>,
    Json(req): Json<ResolveReq>,
) -> Result<Json<JsonValue>, RouteError> {
    if !valid_id(&req.tmt_id) {
        return Err(bad_id());
    }
    let hops = req.max_hops.min(6);

    // Root concept.
    let root = sqlx::query(
        "SELECT tmt_id, concept_type, fsn FROM tmt_codes \
         WHERE tenant_id IS NULL AND tmt_id = ? LIMIT 1",
    )
    .bind(&req.tmt_id)
    .fetch_optional(&pool)
    .await
    .map_err(db_error)?;
    let Some(root) = root else {
        return Err(not_found(&req.tmt_id));
    };
    let root_id: String = root.get("tmt_id");
    let root_type: String = root.get("concept_type");
    let root_fsn: String = root.get("fsn");

    // BFS over undirected relationship graph, bounded by `hops`.
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(root_id.clone());
    let mut frontier: Vec<String> = vec![root_id.clone()];
    for _ in 0..hops {
        if frontier.is_empty() {
            break;
        }
        // ids are alphanumeric-validated on insert (data is alphanumeric, but
        // filter defensively so the IN-list can be interpolated safely).
        let in_list = frontier
            .iter()
            .filter(|id| valid_id(id))
            .map(|id| format!("'{id}'"))
            .collect::<Vec<_>>()
            .join(",");
        if in_list.is_empty() {
            break;
        }
        let sql = format!(
            "SELECT from_id, to_id FROM tmt_relationships \
             WHERE tenant_id IS NULL AND (from_id IN ({in_list}) OR to_id IN ({in_list}))"
        );
        let edges = sqlx::query(&sql).fetch_all(&pool).await.map_err(db_error)?;
        let mut next = Vec::new();
        for r in &edges {
            for col in ["from_id", "to_id"] {
                let nb: String = r.get(col);
                if visited.insert(nb.clone()) {
                    next.push(nb);
                }
            }
        }
        frontier = next;
    }
    visited.remove(&root_id);

    // Hydrate neighbor concepts.
    let neighbors = if visited.is_empty() {
        Vec::new()
    } else {
        let in_list = visited
            .iter()
            .filter(|id| valid_id(id))
            .map(|id| format!("'{id}'"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT tmt_id, concept_type, fsn FROM tmt_codes \
             WHERE tenant_id IS NULL AND tmt_id IN ({in_list})"
        );
        sqlx::query(&sql).fetch_all(&pool).await.map_err(db_error)?
    };

    let mut by_type: HashMap<String, Vec<JsonValue>> = HashMap::new();
    for r in &neighbors {
        let id: String = r.get("tmt_id");
        let ct: String = r.get("concept_type");
        let fsn: String = r.get("fsn");
        by_type
            .entry(ct.clone())
            .or_default()
            .push(json!({"tmt_id": id, "concept_type": ct, "fsn": fsn}));
    }
    let take = |t: &str| by_type.get(t).cloned().unwrap_or_default();

    Ok(Json(json!({
        "root": {"tmt_id": root_id, "concept_type": root_type, "fsn": root_fsn},
        "substances": take("SUBS"),
        "generics": take("GP"),
        "brand_alternatives": take("TP"),
        "related": {
            "VTM": take("VTM"),
            "GPU": take("GPU"),
            "GPP": take("GPP"),
            "TPU": take("TPU"),
            "TPP": take("TPP"),
        },
        "max_hops": hops,
    })))
}

// ─── TMLT expand ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ExpandReq {
    tmlt_id: String,
}

/// PANEL → constituent ITEMs (e.g. CBC → WBC/RBC/Hgb), or ITEM → parent PANELs.
async fn tmlt_expand(
    State(pool): State<DbPool>,
    Json(req): Json<ExpandReq>,
) -> Result<Json<JsonValue>, RouteError> {
    if !valid_id(&req.tmlt_id) {
        return Err(bad_id());
    }

    let root = sqlx::query(
        "SELECT tmlt_id, concept_type, fsn FROM tmlt_codes \
         WHERE tenant_id IS NULL AND tmlt_id = ? LIMIT 1",
    )
    .bind(&req.tmlt_id)
    .fetch_optional(&pool)
    .await
    .map_err(db_error)?;
    let Some(root) = root else {
        return Err(not_found(&req.tmlt_id));
    };
    let root_id: String = root.get("tmlt_id");
    let root_type: String = root.get("concept_type");
    let root_fsn: String = root.get("fsn");

    // PANEL → items via panel_id; ITEM → panels via item_id.
    let (join_col, filter_col, role) = if root_type == "PANEL" {
        ("item_id", "panel_id", "items")
    } else {
        ("panel_id", "item_id", "panels")
    };
    let sql = format!(
        "SELECT c.tmlt_id, c.concept_type, c.fsn \
         FROM tmlt_relationships r \
         JOIN tmlt_codes c ON c.tmlt_id = r.{join_col} AND c.tenant_id IS NULL \
         WHERE r.tenant_id IS NULL AND r.{filter_col} = ? \
         ORDER BY c.tmlt_id"
    );
    let rows = sqlx::query(&sql)
        .bind(&root_id)
        .fetch_all(&pool)
        .await
        .map_err(db_error)?;
    let members = rows
        .iter()
        .map(|r| {
            json!({
                "tmlt_id": r.get::<String, _>("tmlt_id"),
                "concept_type": r.get::<String, _>("concept_type"),
                "fsn": r.get::<String, _>("fsn"),
            })
        })
        .collect::<Vec<_>>();

    let mut out = serde_json::Map::new();
    out.insert(
        "root".into(),
        json!({"tmlt_id": root_id, "concept_type": root_type, "fsn": root_fsn}),
    );
    out.insert(role.into(), JsonValue::Array(members));
    Ok(Json(JsonValue::Object(out)))
}
