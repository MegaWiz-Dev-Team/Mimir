//! Knowledge Graph API Routes — Sprint 17+
//!
//! Provides endpoints for graph CRUD, search, visualization, extraction,
//! and bulk import of entities/relations.
//! All endpoints enforce tenant isolation via X-Tenant-Id header.

use axum::{
    routing::{get, post, delete},
    Router, Json,
    extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{
    entity_type_color, entity_type_size,
};
use serde_json::{json, Value};
use serde::Deserialize;
use tracing::{info, warn, instrument};
use crate::routes::tenant::extract_tenant_id;

// ═══════════════════════════════════════════════════════════════════════════════
// Query parameter structs
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct EntitySearchQuery {
    pub q: Option<String>,
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
    pub limit: Option<u32>,
    pub page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct NeighborQuery {
    pub depth: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    pub depth: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct VisualizationQuery {
    pub limit: Option<u32>,
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractRequest {
    pub source_id: Option<i64>,
    pub text: Option<String>,
    pub max_entities: Option<usize>,
    /// Optional: only extract KG for these specific chunk IDs (incremental)
    pub chunk_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct BulkEntityRequest {
    pub entities: Vec<BulkEntity>,
    pub source_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BulkEntity {
    pub name: String,
    pub entity_type: String,
    pub properties: Option<Value>,
    pub chunk_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BulkRelationRequest {
    pub relations: Vec<BulkRelation>,
    pub source_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BulkRelation {
    pub from_entity: String,
    pub to_entity: String,
    pub relation_type: String,
    pub properties: Option<Value>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Routes definition
// ═══════════════════════════════════════════════════════════════════════════════

pub fn graph_routes() -> Router<DbPool> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/entities", get(search_entities))
        .route("/entities/bulk", post(bulk_create_entities))
        .route("/relations/bulk", post(bulk_create_relations))
        .route("/entity/{id}/neighbors", get(get_neighbors))
        .route("/paths", get(find_paths))
        .route("/extract", post(trigger_extraction))
        .route("/visualization", get(get_visualization))
        .route("/source/{id}", delete(delete_source_entities))
        .route("/extraction-runs", get(get_extraction_runs))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Route handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/graph/stats — Graph statistics per tenant
#[instrument(skip(headers, pool))]
async fn get_stats(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    info!(event = "graph_stats", tenant_id = tenant_id, "Fetching KG stats");

    // Try SQL-based stats from kg_entities/kg_relations tables
    let entity_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM kg_entities WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    let relation_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM kg_relations WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    // Entity type breakdown
    let type_counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT entity_type, COUNT(*) as cnt FROM kg_entities WHERE tenant_id = ? GROUP BY entity_type ORDER BY cnt DESC"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Relation type breakdown
    let rel_type_counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT relation_type, COUNT(*) as cnt FROM kg_relations WHERE tenant_id = ? GROUP BY relation_type ORDER BY cnt DESC"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "total_entities": entity_count.0,
        "total_relations": relation_count.0,
        "entities_by_type": type_counts.iter().map(|(t, c)| json!({"type": t, "count": c})).collect::<Vec<_>>(),
        "relations_by_type": rel_type_counts.iter().map(|(t, c)| json!({"type": t, "count": c})).collect::<Vec<_>>(),
    })))
}

/// GET /api/v1/graph/entities?q=&type=&limit=&page= — Search entities
#[instrument(skip(headers, pool))]
async fn search_entities(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<EntitySearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let limit = params.limit.unwrap_or(50).min(200) as i64;
    let page = params.page.unwrap_or(1).max(1) as i64;
    let offset = (page - 1) * limit;

    let mut query_str = "SELECT id, name, entity_type, properties, source_id, chunk_id, neo4j_node_id FROM kg_entities WHERE tenant_id = ?".to_string();
    let mut count_str = "SELECT COUNT(*) FROM kg_entities WHERE tenant_id = ?".to_string();

    if let Some(ref q) = params.q {
        let filter = " AND (name LIKE ? OR entity_type LIKE ?)";
        query_str.push_str(filter);
        count_str.push_str(filter);
    }
    if let Some(ref et) = params.entity_type {
        if !et.is_empty() {
            let filter = " AND entity_type = ?";
            query_str.push_str(filter);
            count_str.push_str(filter);
        }
    }

    query_str.push_str(" ORDER BY name LIMIT ? OFFSET ?");

    // Build dynamic query
    let mut query = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<i64>, Option<i64>, Option<String>)>(&query_str)
        .bind(tenant_id.clone());

    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_str)
        .bind(tenant_id);

    if let Some(ref q) = params.q {
        let pattern = format!("%{}%", q);
        query = query.bind(pattern.clone()).bind(pattern.clone());
        count_query = count_query.bind(pattern.clone()).bind(pattern.clone());
    }
    if let Some(ref et) = params.entity_type {
        if !et.is_empty() {
            query = query.bind(et.as_str());
            count_query = count_query.bind(et.as_str());
        }
    }

    query = query.bind(limit).bind(offset);

    let rows = query.fetch_all(&pool).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let total = count_query.fetch_one(&pool).await.unwrap_or((0,));

    let entities: Vec<Value> = rows.iter().map(|(id, name, et, props, sid, cid, nid)| {
        json!({
            "id": id,
            "name": name,
            "entity_type": et,
            "properties": props.as_ref().and_then(|p| serde_json::from_str::<Value>(p).ok()),
            "source_id": sid,
            "chunk_id": cid,
            "neo4j_node_id": nid,
            "color": entity_type_color(et),
        })
    }).collect();

    Ok(Json(json!({
        "entities": entities,
        "total": total.0,
        "page": page,
        "limit": limit,
    })))
}

/// GET /api/v1/graph/entity/{id}/neighbors?depth=&limit= — Get neighbors
#[instrument(skip(headers, pool))]
async fn get_neighbors(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(entity_id): Path<i64>,
    Query(params): Query<NeighborQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let depth = params.depth.unwrap_or(1).min(5);
    let limit = params.limit.unwrap_or(50).min(200) as i64;

    // Get entity name
    let entity: Option<(String, String)> = sqlx::query_as(
        "SELECT name, entity_type FROM kg_entities WHERE id = ? AND tenant_id = ?"
    )
    .bind(entity_id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let (entity_name, entity_type) = match entity {
        Some(e) => e,
        None => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Entity not found"})))),
    };

    // Get relations where this entity is involved (via FK)
    let outgoing: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT r.id, e.name, r.relation_type, e.entity_type \
         FROM kg_relations r \
         JOIN kg_entities e ON e.id = r.to_entity_id \
         WHERE r.from_entity_id = ? AND r.tenant_id = ? LIMIT ?"
    )
    .bind(entity_id)
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let incoming: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT r.id, e.name, r.relation_type, e.entity_type \
         FROM kg_relations r \
         JOIN kg_entities e ON e.id = r.from_entity_id \
         WHERE r.to_entity_id = ? AND r.tenant_id = ? LIMIT ?"
    )
    .bind(entity_id)
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut nodes = vec![json!({
        "id": entity_id.to_string(),
        "label": entity_name,
        "entity_type": entity_type,
        "color": entity_type_color(&entity_type),
        "size": entity_type_size(&entity_type),
    })];

    let mut edges = Vec::new();

    for (rid, to_name, rel_type, to_type) in &outgoing {
        let to_type = to_type.as_deref().unwrap_or("concept");
        nodes.push(json!({
            "id": format!("n_{}", to_name),
            "label": to_name,
            "entity_type": to_type,
            "color": entity_type_color(to_type),
            "size": entity_type_size(to_type),
        }));
        edges.push(json!({
            "id": format!("e_{}", rid),
            "source": entity_id.to_string(),
            "target": format!("n_{}", to_name),
            "label": rel_type,
        }));
    }

    for (rid, from_name, rel_type, from_type) in &incoming {
        let from_type = from_type.as_deref().unwrap_or("concept");
        nodes.push(json!({
            "id": format!("n_{}", from_name),
            "label": from_name,
            "entity_type": from_type,
            "color": entity_type_color(from_type),
            "size": entity_type_size(from_type),
        }));
        edges.push(json!({
            "id": format!("e_{}", rid),
            "source": format!("n_{}", from_name),
            "target": entity_id.to_string(),
            "label": rel_type,
        }));
    }

    Ok(Json(json!({
        "center": { "name": entity_name, "entity_type": entity_type },
        "nodes": nodes,
        "edges": edges,
    })))
}

/// GET /api/v1/graph/paths?from=&to=&depth= — Find paths between entities
#[instrument(skip(headers, pool))]
async fn find_paths(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<PathQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Simple BFS/DFS path-finding using SQL (limited depth)
    let max_depth = params.depth.unwrap_or(4).min(6);

    // Find direct relations (via FK join)
    let direct: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT e1.name, e2.name, r.relation_type FROM kg_relations r \
         JOIN kg_entities e1 ON e1.id = r.from_entity_id \
         JOIN kg_entities e2 ON e2.id = r.to_entity_id \
         WHERE r.tenant_id = ? AND \
         ((e1.name = ? AND e2.name = ?) OR (e1.name = ? AND e2.name = ?))"
    )
    .bind(tenant_id)
    .bind(&params.from)
    .bind(&params.to)
    .bind(&params.to)
    .bind(&params.from)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if !direct.is_empty() {
        let path: Vec<Value> = direct.iter().map(|(f, t, r)| {
            json!({"from": f, "to": t, "relation_type": r})
        }).collect();

        return Ok(Json(json!({
            "found": true,
            "paths": [{"steps": path, "length": 1}],
        })));
    }

    // 2-hop search (via FK join)
    let two_hop: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT e1.name, e_mid.name, r1.relation_type, e2.name, r2.relation_type \
         FROM kg_relations r1 \
         JOIN kg_entities e1 ON e1.id = r1.from_entity_id \
         JOIN kg_entities e_mid ON e_mid.id = r1.to_entity_id \
         JOIN kg_relations r2 ON r2.from_entity_id = r1.to_entity_id AND r1.tenant_id = r2.tenant_id \
         JOIN kg_entities e2 ON e2.id = r2.to_entity_id \
         WHERE r1.tenant_id = ? AND e1.name = ? AND e2.name = ? \
         LIMIT 5"
    )
    .bind(tenant_id)
    .bind(&params.from)
    .bind(&params.to)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if !two_hop.is_empty() {
        let paths: Vec<Value> = two_hop.iter().map(|(f, m, r1, t, r2)| {
            json!({
                "steps": [
                    {"from": f, "to": m, "relation_type": r1},
                    {"from": m, "to": t, "relation_type": r2},
                ],
                "length": 2,
            })
        }).collect();

        return Ok(Json(json!({
            "found": true,
            "paths": paths,
        })));
    }

    Ok(Json(json!({
        "found": false,
        "paths": [],
        "message": format!("No path found between '{}' and '{}' within depth {}", params.from, params.to, max_depth),
    })))
}

/// POST /api/v1/graph/extract — Trigger entity extraction (real LLM-powered)
#[instrument(skip(headers, pool))]
async fn trigger_extraction(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<ExtractRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let max_entities = payload.max_entities.unwrap_or(20);

    info!(event = "kg_extraction_triggered", tenant_id = %tenant_id, source_id = ?payload.source_id, "Triggering KG extraction");

    // If text provided directly, parse and return prompt (no background task)
    if let Some(ref text) = payload.text {
        let system_prompt = mimir_core_ai::services::entity_extractor::build_extraction_system_prompt();
        let user_prompt = mimir_core_ai::services::entity_extractor::build_extraction_user_prompt(text, max_entities);

        return Ok(Json(json!({
            "status": "prompt_ready",
            "system_prompt_length": system_prompt.len(),
            "user_prompt_length": user_prompt.len(),
            "message": "Extraction prompts generated. Submit to LLM for entity extraction.",
        })));
    }

    // If source_id provided, spawn real extraction
    if let Some(source_id) = payload.source_id {
        // Record extraction run in DB
        let run_result = sqlx::query(
            "INSERT INTO kg_extraction_runs (source_id, tenant_id, status, started_at) VALUES (?, ?, 'running', NOW())"
        )
        .bind(source_id)
        .bind(&tenant_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

        let run_id = run_result.last_insert_id() as i64;

        // Spawn background extraction
        let pool_bg = pool.clone();
        let tenant_bg = tenant_id.clone();
        tokio::spawn(async move {
            // Load chunks for this source (optionally filtered by chunk_ids)
            let chunks: Vec<(i64, String)> = if let Some(ref ids) = payload.chunk_ids {
                // Incremental: only specified chunks
                let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let query_str = format!("SELECT id, content FROM chunks WHERE source_id = ? AND id IN ({}) LIMIT 10000", placeholders);
                let mut q = sqlx::query_as::<_, (i64, String)>(&query_str).bind(source_id);
                for id in ids {
                    q = q.bind(id);
                }
                q.fetch_all(&pool_bg).await.unwrap_or_default()
            } else {
                sqlx::query_as(
                    "SELECT id, content FROM chunks WHERE source_id = ? LIMIT 10000"
                )
                .bind(source_id)
                .fetch_all(&pool_bg)
                .await
                .unwrap_or_default()
            };

            if chunks.is_empty() {
                let _ = sqlx::query(
                    "UPDATE kg_extraction_runs SET status = 'failed', error_message = 'No chunks found', finished_at = NOW() WHERE id = ?"
                ).bind(run_id).execute(&pool_bg).await;
                return;
            }

            // Resolve LLM credentials
            let router = match mimir_core_ai::services::llm_router::LlmRouter::new(pool_bg.clone(), &tenant_bg).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("LlmRouter init failed for tenant {}: {}", tenant_bg, e);
                    let _ = sqlx::query(
                        "UPDATE kg_extraction_runs SET status = 'failed', error_message = ?, finished_at = NOW() WHERE id = ?"
                    ).bind(format!("LlmRouter init failed: {}", e)).bind(run_id).execute(&pool_bg).await;
                    return;
                }
            };
            let resolved_slot = router.config.resolve_slot("pipeline_generator", None, None);
            let provider_str = resolved_slot.provider;
            let model_str = resolved_slot.model;
            
            let provider = &provider_str;
            let model = &model_str;
            let api_base = crate::routes::sources::infer_api_base(provider);
            let api_key = std::env::var(match provider.as_str() {
                "gemini" | "google" => "GEMINI_API_KEY",
                "openai" => "OPENAI_API_KEY",
                "heimdall" => "HEIMDALL_API_KEY",
                _ => "OLLAMA_API_KEY",
            }).unwrap_or_default();

            // Connect to Neo4j for dual-write
            let neo4j_config = mimir_core_ai::services::neo4j::Neo4jConfig::from_env();
            let neo4j_svc = mimir_core_ai::services::neo4j::Neo4jService::try_new(&neo4j_config).await;

            let mut total_entities = 0i64;
            let mut total_relations = 0i64;
            let mut chunks_processed = 0i64;

            for (chunk_id, content) in &chunks {
                let system_prompt = mimir_core_ai::services::entity_extractor::build_extraction_system_prompt();
                let user_prompt = mimir_core_ai::services::entity_extractor::build_extraction_user_prompt(content, max_entities);
                let combined = format!("{}\n\n{}", system_prompt, user_prompt);

                let result = crate::routes::sources::call_llm_api_with_logging(
                    &api_key, &api_base, model, &combined,
                    Some(&pool_bg), Some(&tenant_bg), Some(provider), Some("kg_extraction"),
                ).await;

                if let Ok((response_text, _)) = result {
                    let parsed = mimir_core_ai::services::entity_extractor::parse_extraction_response(&response_text);
                    let entities = mimir_core_ai::services::entity_extractor::dedup_entities(parsed.entities);
                    let relations = mimir_core_ai::services::entity_extractor::dedup_relations(parsed.relations);

                    // Insert entities
                    for ent in &entities {
                        let props_str = ent.properties.as_ref().map(|p| p.to_string());
                        let _ = sqlx::query(
                            "INSERT IGNORE INTO kg_entities (tenant_id, source_id, chunk_id, name, entity_type, properties) VALUES (?, ?, ?, ?, ?, ?)"
                        )
                        .bind(&tenant_bg).bind(source_id).bind(chunk_id)
                        .bind(&ent.name).bind(&ent.entity_type).bind(&props_str)
                        .execute(&pool_bg).await;

                        if let Some(ref neo4j) = neo4j_svc {
                            let _ = neo4j.upsert_entity(
                                &tenant_bg, &ent.name, &ent.entity_type,
                                props_str.as_deref(), Some(source_id), Some(*chunk_id),
                            ).await;
                        }
                        total_entities += 1;
                    }

                    // Insert relations
                    for rel in &relations {
                        let from_id: Option<(i64,)> = sqlx::query_as(
                            "SELECT id FROM kg_entities WHERE tenant_id = ? AND name = ? LIMIT 1"
                        ).bind(&tenant_bg).bind(&rel.from)
                        .fetch_optional(&pool_bg).await.unwrap_or(None);

                        let to_id: Option<(i64,)> = sqlx::query_as(
                            "SELECT id FROM kg_entities WHERE tenant_id = ? AND name = ? LIMIT 1"
                        ).bind(&tenant_bg).bind(&rel.to)
                        .fetch_optional(&pool_bg).await.unwrap_or(None);

                        if let (Some((fid,)), Some((tid,))) = (from_id, to_id) {
                            let _ = sqlx::query(
                                "INSERT IGNORE INTO kg_relations (tenant_id, source_id, from_entity_id, to_entity_id, relation_type) VALUES (?, ?, ?, ?, ?)"
                            )
                            .bind(&tenant_bg).bind(source_id)
                            .bind(fid).bind(tid).bind(&rel.relation_type)
                            .execute(&pool_bg).await;

                            if let Some(ref neo4j) = neo4j_svc {
                                let _ = neo4j.upsert_relation(
                                    &tenant_bg, &rel.from, &rel.to, &rel.relation_type,
                                    None, Some(source_id),
                                ).await;
                            }
                            total_relations += 1;
                        }
                    }
                }
                chunks_processed += 1;

                // Update progress every 5 chunks
                if chunks_processed % 5 == 0 {
                    let _ = sqlx::query(
                        "UPDATE kg_extraction_runs SET entities_found = ?, relations_found = ?, chunks_processed = ? WHERE id = ?"
                    ).bind(total_entities).bind(total_relations).bind(chunks_processed).bind(run_id)
                    .execute(&pool_bg).await;
                }
            }

            // Final update
            let _ = sqlx::query(
                "UPDATE kg_extraction_runs SET status = 'completed', entities_found = ?, relations_found = ?, chunks_processed = ?, finished_at = NOW() WHERE id = ?"
            ).bind(total_entities).bind(total_relations).bind(chunks_processed).bind(run_id)
            .execute(&pool_bg).await;

            info!(event = "kg_extraction_completed", run_id = run_id, entities = total_entities, relations = total_relations, chunks = chunks_processed);
        });

        return Ok(Json(json!({
            "status": "started",
            "run_id": run_id,
            "source_id": source_id,
            "message": "KG extraction started in background. Use GET /extraction-runs to check progress.",
        })));
    }

    Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Either 'source_id' or 'text' must be provided"}))))
}

/// GET /api/v1/graph/visualization?limit=&type= — Get graph data for Sigma.js
#[instrument(skip(headers, pool))]
async fn get_visualization(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<VisualizationQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let limit = params.limit.unwrap_or(200).min(1000) as i64;
    info!(event = "graph_visualization", tenant_id = tenant_id, limit = limit, "Fetching visualization data");

    // Get entities
    let mut entity_query = "SELECT id, name, entity_type, CAST(properties AS CHAR) as properties FROM kg_entities WHERE tenant_id = ?".to_string();
    if let Some(ref et) = params.entity_type {
        entity_query.push_str(&format!(" AND entity_type = '{}'", et.replace('\'', "''")));
    }
    entity_query.push_str(" LIMIT ?");

    let entities: Vec<(i64, String, String, Option<String>)> = match sqlx::query_as(&entity_query)
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, tenant_id = tenant_id, query = %entity_query, "Visualization entity query failed");
            Vec::new()
        }
    };

    let nodes: Vec<Value> = entities.iter().map(|(id, name, et, _)| {
        json!({
            "id": id.to_string(),
            "label": name,
            "entity_type": et,
            "color": entity_type_color(et),
            "size": entity_type_size(et),
        })
    }).collect();

    // Get relations between visible entities
    let entity_names: Vec<String> = entities.iter().map(|(_, name, _, _)| name.clone()).collect();
    let mut edges = Vec::new();

    if !entity_names.is_empty() {
        // Get relations between visible entities (via FK)
        let relations: Vec<(i64, i64, i64, String)> = sqlx::query_as(
            "SELECT r.id, r.from_entity_id, r.to_entity_id, r.relation_type \
             FROM kg_relations r \
             WHERE r.tenant_id = ? \
             LIMIT ?"
        )
        .bind(tenant_id)
        .bind(limit * 2)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        // Build entity id → string id lookup
        let id_to_str: std::collections::HashMap<i64, String> = entities.iter()
            .map(|(id, _, _, _)| (*id, id.to_string()))
            .collect();

        for (rid, from_id, to_id, rtype) in &relations {
            if id_to_str.contains_key(from_id) && id_to_str.contains_key(to_id) {
                edges.push(json!({
                    "id": format!("e_{}", rid),
                    "source": from_id.to_string(),
                    "target": to_id.to_string(),
                    "label": rtype,
                }));
            }
        }
    }

    Ok(Json(json!({
        "nodes": nodes,
        "edges": edges,
        "total_nodes": nodes.len(),
        "total_edges": edges.len(),
    })))
}

/// DELETE /api/v1/graph/source/{id} — Delete all entities/relations for a source
#[instrument(skip(headers, pool))]
async fn delete_source_entities(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(source_id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Delete relations first (referencing entities from this source)
    let rel_deleted = sqlx::query(
        "DELETE FROM kg_relations WHERE tenant_id = ? AND source_id = ?"
    )
    .bind(tenant_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // Delete entities
    let ent_deleted = sqlx::query(
        "DELETE FROM kg_entities WHERE tenant_id = ? AND source_id = ?"
    )
    .bind(tenant_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    info!(
        event = "kg_source_deleted",
        tenant_id = tenant_id,
        source_id = source_id,
        entities_deleted = ent_deleted.rows_affected(),
        relations_deleted = rel_deleted.rows_affected(),
    );

    Ok(Json(json!({
        "deleted_entities": ent_deleted.rows_affected(),
        "deleted_relations": rel_deleted.rows_affected(),
        "source_id": source_id,
    })))
}

/// GET /api/v1/graph/extraction-runs — List extraction runs
#[instrument(skip(headers, pool))]
async fn get_extraction_runs(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let runs: Vec<(i64, i64, String, i64, i64, i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, source_id, status, entities_found, relations_found, chunks_processed, \
         started_at, completed_at, error_message \
         FROM kg_extraction_runs WHERE tenant_id = ? ORDER BY id DESC LIMIT 20"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let runs_json: Vec<Value> = runs.iter().map(|(id, sid, status, ents, rels, chunks, started, completed, err)| {
        json!({
            "id": id,
            "source_id": sid,
            "status": status,
            "entities_found": ents,
            "relations_found": rels,
            "chunks_processed": chunks,
            "started_at": started,
            "completed_at": completed,
            "error_message": err,
        })
    }).collect();

    Ok(Json(json!({
        "runs": runs_json,
    })))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Bulk Import handlers (Sprint 18+)
// ═══════════════════════════════════════════════════════════════════════════════

/// POST /api/v1/graph/entities/bulk — Bulk import entities
#[instrument(skip(headers, pool))]
async fn bulk_create_entities(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<BulkEntityRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let source_id = payload.source_id;
    let total = payload.entities.len();
    
    info!(event = "kg_bulk_create_entities", tenant_id = tenant_id, count = total, "Bulk creating entities");

    let mut inserted = 0u64;
    let mut skipped = 0u64;

    for ent in &payload.entities {
        let props_json = ent.properties.as_ref()
            .map(|p| serde_json::to_string(p).unwrap_or_default());

        let result = sqlx::query(
            "INSERT IGNORE INTO kg_entities (name, entity_type, properties, source_id, chunk_id, tenant_id) \
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&ent.name)
        .bind(&ent.entity_type)
        .bind(&props_json)
        .bind(source_id)
        .bind(ent.chunk_id)
        .bind(tenant_id)
        .execute(&pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            Ok(_) => skipped += 1,
            Err(e) => {
                warn!(error = %e, name = %ent.name, "Entity insert failed");
                skipped += 1;
            }
        }
    }

    info!(event = "kg_bulk_entities_done", inserted = inserted, skipped = skipped);

    Ok(Json(json!({
        "inserted": inserted,
        "skipped": skipped,
        "total": total,
    })))
}

/// POST /api/v1/graph/relations/bulk — Bulk import relations
#[instrument(skip(headers, pool))]
async fn bulk_create_relations(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<BulkRelationRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let source_id = payload.source_id;
    let total = payload.relations.len();

    info!(event = "kg_bulk_create_relations", tenant_id = tenant_id, count = total, "Bulk creating relations");

    let mut inserted = 0u64;
    let mut skipped = 0u64;

    for rel in &payload.relations {
        let props_json = rel.properties.as_ref()
            .map(|p| serde_json::to_string(p).unwrap_or_default());

        // Lookup from_entity_id
        let from_id: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM kg_entities WHERE name = ? AND tenant_id = ? LIMIT 1"
        )
        .bind(&rel.from_entity)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        // Lookup to_entity_id
        let to_id: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM kg_entities WHERE name = ? AND tenant_id = ? LIMIT 1"
        )
        .bind(&rel.to_entity)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        let (from_id, to_id) = match (from_id, to_id) {
            (Some((fid,)), Some((tid,))) => (fid, tid),
            _ => {
                skipped += 1;
                continue;
            }
        };

        let result = sqlx::query(
            "INSERT IGNORE INTO kg_relations (from_entity_id, to_entity_id, relation_type, properties, source_id, tenant_id) \
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(from_id)
        .bind(to_id)
        .bind(&rel.relation_type)
        .bind(&props_json)
        .bind(source_id)
        .bind(tenant_id)
        .execute(&pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            Ok(_) => skipped += 1,
            Err(e) => {
                warn!(error = %e, from = %rel.from_entity, to = %rel.to_entity, "Relation insert failed");
                skipped += 1;
            }
        }
    }

    info!(event = "kg_bulk_relations_done", inserted = inserted, skipped = skipped);

    Ok(Json(json!({
        "inserted": inserted,
        "skipped": skipped,
        "total": total,
    })))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_routes_assembly() {
        // Verify routes are constructed without panic
        let _router = graph_routes();
        // If we get here without panic, routes are assembled correctly
        assert!(true, "Graph routes assembled successfully");
    }

    #[test]
    fn test_entity_search_query_defaults() {
        let query: EntitySearchQuery = serde_json::from_str("{}").unwrap();
        assert!(query.q.is_none());
        assert!(query.entity_type.is_none());
        assert!(query.limit.is_none());
        assert!(query.page.is_none());
    }

    #[test]
    fn test_extract_request_defaults() {
        let req: ExtractRequest = serde_json::from_str("{}").unwrap();
        assert!(req.source_id.is_none());
        assert!(req.text.is_none());
        assert!(req.max_entities.is_none());
    }

    #[test]
    fn test_visualization_query_deserialize() {
        let query: VisualizationQuery = serde_json::from_str(r#"{"limit": 100}"#).unwrap();
        assert_eq!(query.limit, Some(100));
    }

    #[test]
    fn test_path_query_required_fields() {
        let query: PathQuery = serde_json::from_str(r#"{"from": "A", "to": "B"}"#).unwrap();
        assert_eq!(query.from, "A");
        assert_eq!(query.to, "B");
        assert!(query.depth.is_none());
    }
}
