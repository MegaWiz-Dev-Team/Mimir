//! Integration layer: executes the Phase-1 resolution/dedup decisions against
//! Neo4j (via `neo4rs`) and the Heimdall embedder.
//!
//! All decision logic lives in the pure modules ([`super::naming`],
//! [`super::scoring`], [`super::gate`], [`super::plan_phase1_action`]); this file
//! only does I/O: embed text, fetch candidates, write canonical/alias, store the
//! ingest embedding, and propose `DUPLICATE_OF` review edges. It never merges.

use anyhow::{Context, Result};
use neo4rs::{query, Graph};

use super::cypher;
use super::gate::medical_gate;
use super::naming::{self, NameCandidate, NameResolution};
use super::scoring::{self, Band};
use super::{plan_phase1_action, Action, Embedder};

/// Tunables for a resolution pass.
#[derive(Debug, Clone)]
pub struct ResolveParams {
    pub fuzzy_threshold: f32,
    pub semantic_threshold: f32,
    pub candidate_limit: i64,
    /// Who proposed any review edge ("system" for live ingest, "dream" for the
    /// future nightly pass, or a user id).
    pub proposed_by: String,
}

impl Default for ResolveParams {
    fn default() -> Self {
        Self {
            fuzzy_threshold: 0.88,
            semantic_threshold: 0.90,
            candidate_limit: 200,
            proposed_by: "system".to_string(),
        }
    }
}

/// One row of the human review queue.
#[derive(Debug, Clone)]
pub struct ReviewItem {
    pub canonical_name: String,
    pub duplicate_name: String,
    pub confidence: f64,
    pub method: String,
    pub code_match: bool,
    pub rel_id: String,
}

fn f32_to_f64(v: &[f32]) -> Vec<f64> {
    v.iter().map(|x| *x as f64).collect()
}
fn f64_to_f32(v: Vec<f64>) -> Vec<f32> {
    v.into_iter().map(|x| x as f32).collect()
}

/// Store the ingest-time embedding (+ model/dim stamp) on an existing `:Entity`.
pub async fn store_embedding(
    graph: &Graph,
    tenant_id: &str,
    name: &str,
    entity_type: &str,
    embedding: &[f32],
    embed_model: &str,
) -> Result<()> {
    let q = query(cypher::build_store_embedding_cypher())
        .param("name", name)
        .param("entity_type", entity_type)
        .param("tenant_id", tenant_id)
        .param("embedding", f32_to_f64(embedding))
        .param("embed_model", embed_model)
        .param("embed_dim", embedding.len() as i64);
    graph.run(q).await.context("store_embedding")?;
    Ok(())
}

/// Persist the resolved canonical name + alias set on an `:Entity`.
pub async fn set_canonical_and_aliases(
    graph: &Graph,
    tenant_id: &str,
    name: &str,
    entity_type: &str,
    canonical_name: &str,
    aliases: &[String],
) -> Result<()> {
    let q = query(cypher::build_set_canonical_and_aliases_cypher())
        .param("name", name)
        .param("entity_type", entity_type)
        .param("tenant_id", tenant_id)
        .param("canonical_name", canonical_name)
        .param("aliases", aliases.to_vec());
    graph.run(q).await.context("set_canonical_and_aliases")?;
    Ok(())
}

/// Fetch same-type candidates within a tenant (excludes global PrimeKG nodes).
pub async fn find_candidates(
    graph: &Graph,
    tenant_id: &str,
    entity_type: &str,
    limit: i64,
) -> Result<Vec<NameCandidate>> {
    let q = query(cypher::build_find_candidates_cypher())
        .param("tenant_id", tenant_id)
        .param("entity_type", entity_type)
        .param("limit", limit);
    let mut result = graph.execute(q).await.context("find_candidates")?;
    let mut out = Vec::new();
    while let Some(row) = result.next().await? {
        let embedding = row
            .get::<Vec<f64>>("embedding")
            .map(f64_to_f32)
            .unwrap_or_default();
        out.push(NameCandidate {
            canonical_name: row.get::<String>("canonical_name").unwrap_or_default(),
            aliases: row.get::<Vec<String>>("aliases").unwrap_or_default(),
            entity_type: row.get::<String>("entity_type").unwrap_or_default(),
            embedding,
        });
    }
    Ok(out)
}

/// Propose a duplicate pair for human review (idempotent via MERGE).
#[allow(clippy::too_many_arguments)]
pub async fn flag_duplicate(
    graph: &Graph,
    tenant_id: &str,
    entity_type: &str,
    src_name: &str,
    dst_name: &str,
    confidence: f64,
    score_embed: f64,
    score_fuzzy: f64,
    score_method: &str,
    embed_model: &str,
    embed_dim: i64,
    code_match: bool,
    proposed_by: &str,
) -> Result<()> {
    let q = query(cypher::build_flag_duplicate_cypher())
        .param("tenant_id", tenant_id)
        .param("entity_type", entity_type)
        .param("src_name", src_name)
        .param("dst_name", dst_name)
        .param("confidence", confidence)
        .param("score_embed", score_embed)
        .param("score_fuzzy", score_fuzzy)
        .param("score_method", score_method)
        .param("embed_model", embed_model)
        .param("embed_dim", embed_dim)
        .param("code_match", code_match)
        .param("proposed_by", proposed_by);
    graph.run(q).await.context("flag_duplicate")?;
    Ok(())
}

/// Read the pending review queue for a tenant, highest confidence first.
pub async fn review_queue(graph: &Graph, tenant_id: &str, limit: i64) -> Result<Vec<ReviewItem>> {
    let q = query(cypher::build_review_queue_cypher())
        .param("tenant_id", tenant_id)
        .param("limit", limit);
    let mut result = graph.execute(q).await.context("review_queue")?;
    let mut out = Vec::new();
    while let Some(row) = result.next().await? {
        out.push(ReviewItem {
            canonical_name: row.get::<String>("canonical_name").unwrap_or_default(),
            duplicate_name: row.get::<String>("duplicate_name").unwrap_or_default(),
            confidence: row.get::<f64>("confidence").unwrap_or(0.0),
            method: row.get::<String>("method").unwrap_or_default(),
            code_match: row.get::<bool>("code_match").unwrap_or(false),
            rel_id: row.get::<String>("rel_id").unwrap_or_default(),
        });
    }
    Ok(out)
}

/// End-to-end Phase-1 resolution for one already-upserted entity.
///
/// Assumes the `:Entity {name, entity_type, tenant_id}` node already exists (the
/// extraction pipeline upserts it). This then: embeds its full-context text,
/// fetches same-type candidates, runs the pure resolution chain, applies the
/// medical-safety-gated band, and persists the resulting [`Action`] —
/// canonical/alias writes + an optional `DUPLICATE_OF` review edge. Returns the
/// action taken (for logging / Tyr audit upstream).
pub async fn resolve_and_flag(
    graph: &Graph,
    embedder: &dyn Embedder,
    tenant_id: &str,
    raw_name: &str,
    entity_type: &str,
    embed_text: &str,
    code_match: bool,
    params: &ResolveParams,
) -> Result<Action> {
    // 1. Embed full-context text and store it on the node (for dedup + Phase-2 dream pass).
    let embedding = embedder.embed(embed_text).await?;
    store_embedding(graph, tenant_id, raw_name, entity_type, &embedding, embedder.model_id()).await?;

    // 2. Candidates of the same type, then the pure resolution chain. The chain
    //    compares the incoming full-context embedding against candidates' stored
    //    full-context embeddings (same space).
    let candidates = find_candidates(graph, tenant_id, entity_type, params.candidate_limit).await?;
    let candidates: Vec<NameCandidate> = candidates
        .into_iter()
        .filter(|c| c.canonical_name != naming::normalize_entity_name(raw_name))
        .collect();
    let resolution = naming::resolve_chain(
        raw_name,
        entity_type,
        &candidates,
        Some(&embedding),
        params.fuzzy_threshold,
        params.semantic_threshold,
    );

    // 3. Blended dedup band (for the matched candidate), then the medical gate.
    let (score_embed, score_fuzzy, method) = match &resolution {
        NameResolution::Matched { canonical_name, via, score, .. } => match via {
            naming::MatchMethod::Semantic => (*score as f64, scoring::fuzzy_ratio(&naming::normalize_entity_name(raw_name), canonical_name) as f64, "semantic"),
            naming::MatchMethod::Fuzzy => {
                // For a fuzzy match we lack a cheap cosine; treat embedding signal
                // as neutral and let the fuzzy score drive the recorded confidence.
                (0.0, *score as f64, "fuzzy")
            }
            naming::MatchMethod::Exact => (1.0, 1.0, "exact"),
        },
        NameResolution::New { .. } => (0.0, 0.0, "none"),
    };
    let raw_band = scoring::band(scoring::combined_score(score_embed as f32, score_fuzzy as f32));
    let gated_band = medical_gate(raw_band, code_match, entity_type);

    let action = plan_phase1_action(&resolution, gated_band);

    // 4. Persist the action.
    match &action {
        Action::Create { canonical_name } => {
            set_canonical_and_aliases(graph, tenant_id, raw_name, entity_type, canonical_name, &[]).await?;
        }
        Action::AssignCanonical { canonical_name, alias_to_add } => {
            let aliases: Vec<String> = alias_to_add.iter().cloned().collect();
            let merged = naming::merge_alias_set(canonical_name, &[], &aliases);
            set_canonical_and_aliases(graph, tenant_id, canonical_name, entity_type, canonical_name, &merged).await?;
        }
        Action::FlagDuplicate { canonical_name, band } => {
            // Ensure the incoming node carries its own canonical name first.
            set_canonical_and_aliases(graph, tenant_id, raw_name, entity_type, &naming::normalize_entity_name(raw_name), &[]).await?;
            let confidence = match band {
                Band::AutoMerge => 0.95,
                Band::Review => 0.90,
                Band::New => score_embed.max(score_fuzzy),
            };
            flag_duplicate(
                graph,
                tenant_id,
                entity_type,
                &naming::normalize_entity_name(raw_name),
                canonical_name,
                confidence,
                score_embed,
                score_fuzzy,
                method,
                embedder.model_id(),
                embedding.len() as i64,
                code_match,
                &params.proposed_by,
            )
            .await?;
        }
    }

    Ok(action)
}

// ─── Phase 2: merge / review-resolution / dream pass ────────────────────────────

/// Read a node's canonical name + alias set (for computing the merged alias union).
async fn read_aliases(
    graph: &Graph,
    tenant_id: &str,
    name: &str,
    entity_type: &str,
) -> Result<(String, Vec<String>)> {
    let q = query(cypher::build_find_candidates_cypher())
        .param("tenant_id", tenant_id)
        .param("entity_type", entity_type)
        .param("limit", 10_000i64);
    let mut result = graph.execute(q).await.context("read_aliases")?;
    let want = naming::normalize_entity_name(name);
    while let Some(row) = result.next().await? {
        let n = row.get::<String>("name").unwrap_or_default();
        if naming::normalize_entity_name(&n) == want {
            return Ok((
                row.get::<String>("canonical_name").unwrap_or_else(|_| n.clone()),
                row.get::<Vec<String>>("aliases").unwrap_or_default(),
            ));
        }
    }
    Ok((want, vec![]))
}

/// Merge `duplicate` into `survivor` (single atomic statement). Computes the
/// union alias set first (pure), then runs the hand-written merge Cypher: edge
/// redirect (both directions, MERGE-deduped, self-loops dropped), tombstone,
/// name-namespacing (anti-resurrection), and a `MERGED_INTO` audit edge.
#[allow(clippy::too_many_arguments)]
pub async fn merge_entities(
    graph: &Graph,
    tenant_id: &str,
    entity_type: &str,
    survivor: &str,
    duplicate: &str,
    merged_by: &str,
    confidence: f64,
    code_match: bool,
) -> Result<String> {
    let (surv_canon, surv_aliases) = read_aliases(graph, tenant_id, survivor, entity_type).await?;
    let (_dup_canon, dup_aliases) = read_aliases(graph, tenant_id, duplicate, entity_type).await?;
    let mut incoming = dup_aliases;
    incoming.push(duplicate.to_string()); // the duplicate's surface name becomes an alias
    let merged_aliases = naming::merge_alias_set(&surv_canon, &surv_aliases, &incoming);

    let q = query(cypher::build_merge_nodes_cypher())
        .param("survivor", survivor)
        .param("duplicate", duplicate)
        .param("entity_type", entity_type)
        .param("tenant_id", tenant_id)
        .param("merged_aliases", merged_aliases)
        .param("merged_by", merged_by)
        .param("confidence", confidence)
        .param("code_match", code_match);
    let mut result = graph.execute(q).await.context("merge_entities")?;
    if let Some(row) = result.next().await? {
        Ok(row.get::<String>("survivor_id").unwrap_or_default())
    } else {
        Err(anyhow::anyhow!(
            "merge_entities: no survivor returned (nodes missing or identical?)"
        ))
    }
}

/// Record a human review decision on a `DUPLICATE_OF` proposal. On `approve`,
/// also executes the merge (survivor = the canonical `dst_name`).
#[allow(clippy::too_many_arguments)]
pub async fn resolve_duplicate(
    graph: &Graph,
    tenant_id: &str,
    entity_type: &str,
    duplicate_name: &str,
    canonical_name: &str,
    approve: bool,
    decided_by: &str,
    confidence: f64,
    code_match: bool,
) -> Result<()> {
    let status = if approve { "approved" } else { "rejected" };
    let q = query(cypher::build_set_duplicate_status_cypher())
        .param("src_name", duplicate_name)
        .param("dst_name", canonical_name)
        .param("entity_type", entity_type)
        .param("tenant_id", tenant_id)
        .param("status", status)
        .param("decided_by", decided_by);
    graph.run(q).await.context("set_duplicate_status")?;

    if approve {
        merge_entities(
            graph,
            tenant_id,
            entity_type,
            canonical_name,
            duplicate_name,
            decided_by,
            confidence,
            code_match,
        )
        .await?;
    }
    Ok(())
}

/// Recently ingested, non-tombstoned entities (the dream pass's work set).
/// `since` is an ISO-8601 datetime string.
pub async fn find_recent_entities(
    graph: &Graph,
    tenant_id: &str,
    since: &str,
    limit: i64,
) -> Result<Vec<NameCandidate>> {
    let q = query(cypher::build_recent_nodes_cypher())
        .param("tenant_id", tenant_id)
        .param("since", since)
        .param("limit", limit);
    let mut result = graph.execute(q).await.context("find_recent_entities")?;
    let mut out = Vec::new();
    while let Some(row) = result.next().await? {
        out.push(NameCandidate {
            canonical_name: row.get::<String>("canonical_name").unwrap_or_default(),
            aliases: vec![],
            entity_type: row.get::<String>("entity_type").unwrap_or_default(),
            embedding: row.get::<Vec<f64>>("embedding").map(f64_to_f32).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Nightly "dream pass": re-run deduplication over recently ingested nodes only,
/// reusing their stored embeddings (no embedding calls). Entities ingested in the
/// same batch never got compared to each other at ingest time; this closes that
/// gap. Flags new `DUPLICATE_OF` review pairs (idempotent via MERGE); never
/// merges. Returns the number of pairs flagged.
pub async fn dream_pass(
    graph: &Graph,
    tenant_id: &str,
    since: &str,
    params: &ResolveParams,
) -> Result<usize> {
    let recent = find_recent_entities(graph, tenant_id, since, params.candidate_limit).await?;
    let mut flagged = 0usize;
    for node in &recent {
        if node.embedding.is_empty() {
            continue; // can't compare without a stored embedding
        }
        let candidates = find_candidates(graph, tenant_id, &node.entity_type, params.candidate_limit).await?;
        let candidates: Vec<NameCandidate> = candidates
            .into_iter()
            .filter(|c| c.canonical_name != node.canonical_name)
            .collect();
        let resolution = naming::resolve_chain(
            &node.canonical_name,
            &node.entity_type,
            &candidates,
            Some(&node.embedding),
            params.fuzzy_threshold,
            params.semantic_threshold,
        );
        if let NameResolution::Matched { canonical_name, via, score, .. } = &resolution {
            if *via != naming::MatchMethod::Exact {
                let (se, sf, method) = match via {
                    naming::MatchMethod::Semantic => (*score as f64, 0.0, "semantic"),
                    naming::MatchMethod::Fuzzy => (0.0, *score as f64, "fuzzy"),
                    naming::MatchMethod::Exact => unreachable!(),
                };
                flag_duplicate(
                    graph,
                    tenant_id,
                    &node.entity_type,
                    &node.canonical_name,
                    canonical_name,
                    (*score) as f64,
                    se,
                    sf,
                    method,
                    "",
                    node.embedding.len() as i64,
                    false,
                    "dream",
                )
                .await?;
                flagged += 1;
            }
        }
    }
    Ok(flagged)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Integration tests — require a live Neo4j + Heimdall; run with `--ignored`.
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    // NOTE: these are #[ignore]d because they need the live Asgard stack
    // (Neo4j bolt + Heimdall :8080). Run explicitly with:
    //   cargo test -p mimir-core-ai --test ... -- --ignored
    // The pure decision logic is covered by the inline unit tests in the sibling
    // modules and by tests/resolve_phase1.rs.

    #[tokio::test]
    #[ignore = "requires live Neo4j + Heimdall"]
    async fn flag_only_pipeline_is_idempotent() {
        // Placeholder for the live E2E: ingest two near-duplicate medical entities,
        // assert exactly one pending DUPLICATE_OF edge, and that a second run adds
        // none (MERGE idempotency). Wired once a test Neo4j fixture is available.
    }
}
