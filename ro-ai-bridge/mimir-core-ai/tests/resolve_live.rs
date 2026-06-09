//! Live Neo4j integration tests for the Phase-2 merge / review / dream-pass paths.
//!
//! These validate graph-mutation correctness that substring tests cannot
//! (edge redirect in both directions, parallel-edge dedup, self-loop dropping,
//! tombstone exclusion, anti-resurrection name namespacing, MERGED_INTO audit).
//!
//! They require a live Neo4j and are SKIPPED (pass as no-ops) unless
//! `NEO4J_TEST_URI` is set. Run with:
//!   NEO4J_TEST_URI=bolt://HOST:7687 NEO4J_TEST_USER=neo4j \
//!   NEO4J_TEST_PASSWORD=*** cargo test -p mimir-core-ai --test resolve_live
//!
//! Each test uses a unique throwaway tenant and deletes it on entry and exit, so
//! it never touches real tenant data.

use mimir_core_ai::services::resolve::store;
use neo4rs::{query, Graph};

fn test_env() -> Option<(String, String, String)> {
    let uri = std::env::var("NEO4J_TEST_URI").ok()?;
    let user = std::env::var("NEO4J_TEST_USER").unwrap_or_else(|_| "neo4j".into());
    let pass = std::env::var("NEO4J_TEST_PASSWORD").unwrap_or_default();
    Some((uri, user, pass))
}

async fn connect() -> Option<Graph> {
    let (uri, user, pass) = test_env()?;
    match Graph::new(&uri, &user, &pass).await {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("resolve_live: cannot connect ({e}); skipping");
            None
        }
    }
}

async fn wipe_tenant(g: &Graph, tenant: &str) {
    let _ = g
        .run(query("MATCH (n:Entity {tenant_id: $t}) DETACH DELETE n").param("t", tenant))
        .await;
}

async fn mk_entity(g: &Graph, tenant: &str, name: &str, ty: &str) {
    g.run(
        query(
            "CREATE (n:Entity {name:$name, entity_type:$ty, tenant_id:$t, created_at: datetime()})",
        )
        .param("name", name)
        .param("ty", ty)
        .param("t", tenant),
    )
    .await
    .unwrap();
}

async fn mk_rel(g: &Graph, tenant: &str, from: &str, to: &str, rel: &str) {
    g.run(
        query(
            "MATCH (a:Entity {name:$from, tenant_id:$t}), (b:Entity {name:$to, tenant_id:$t}) \
             CREATE (a)-[:RELATES_TO {relation_type:$rel, tenant_id:$t}]->(b)",
        )
        .param("from", from)
        .param("to", to)
        .param("rel", rel)
        .param("t", tenant),
    )
    .await
    .unwrap();
}

async fn count(g: &Graph, cypher: &str, tenant: &str) -> i64 {
    let mut r = g.execute(query(cypher).param("t", tenant)).await.unwrap();
    if let Some(row) = r.next().await.unwrap() {
        row.get::<i64>("c").unwrap_or(0)
    } else {
        0
    }
}

#[tokio::test]
async fn merge_redirects_dedups_and_tombstones() {
    let Some(g) = connect().await else { return };
    let t = "resolve_test_merge";
    wipe_tenant(&g, t).await;

    // Fixture: survivor "aspirin", duplicate "asprin" (distinct names — the upsert
    // MERGE key is name+type+tenant, so real duplicates always differ in name),
    // plus X/Y/Z, all type DRUG.
    for n in ["aspirin", "asprin", "x", "y", "z"] {
        mk_entity(&g, t, n, "DRUG").await;
    }
    // A->X (R1) and B->X (R1): parallel edge → must collapse to one.
    mk_rel(&g, t, "aspirin", "x", "R1").await;
    mk_rel(&g, t, "asprin", "x", "R1").await;
    // B->Y (R2): outgoing redirect to A->Y.
    mk_rel(&g, t, "asprin", "y", "R2").await;
    // Z->B (R3): incoming redirect to Z->A.
    mk_rel(&g, t, "z", "asprin", "R3").await;
    // B->B self-loop (R4): must be dropped, never become A->B.
    mk_rel(&g, t, "asprin", "asprin", "R4").await;
    // B->A residual (R5): must be dropped.
    mk_rel(&g, t, "asprin", "aspirin", "R5").await;

    // Merge duplicate "asprin" into survivor "aspirin".
    let survivor_id = store::merge_entities(&g, t, "DRUG", "aspirin", "asprin", "tester", 0.97, true)
        .await
        .expect("merge ok");
    assert!(!survivor_id.is_empty());

    // A->X collapsed to exactly one R1 edge.
    let ax = count(
        &g,
        "MATCH (:Entity {name:'aspirin', tenant_id:$t})-[r:RELATES_TO {relation_type:'R1'}]->(:Entity {name:'x'}) RETURN count(r) AS c",
        t,
    )
    .await;
    assert_eq!(ax, 1, "parallel A->X / B->X must collapse to one");

    // B->Y redirected to A->Y.
    let ay = count(
        &g,
        "MATCH (:Entity {name:'aspirin', tenant_id:$t})-[r:RELATES_TO {relation_type:'R2'}]->(:Entity {name:'y'}) RETURN count(r) AS c",
        t,
    )
    .await;
    assert_eq!(ay, 1, "B->Y must redirect to A->Y");

    // Z->B redirected to Z->A (direction preserved).
    let za = count(
        &g,
        "MATCH (:Entity {name:'z', tenant_id:$t})-[r:RELATES_TO {relation_type:'R3'}]->(:Entity {name:'aspirin'}) RETURN count(r) AS c",
        t,
    )
    .await;
    assert_eq!(za, 1, "Z->B must redirect to Z->A preserving direction");

    // The tombstoned duplicate has NO remaining RELATES_TO edges (self-loop + residual dropped).
    let b_edges = count(
        &g,
        "MATCH (b:Entity {tenant_id:$t})-[r:RELATES_TO]-() WHERE b.name STARTS WITH 'asprin#merged#' RETURN count(r) AS c",
        t,
    )
    .await;
    assert_eq!(b_edges, 0, "tombstoned node must have no RELATES_TO edges left");

    // Duplicate is tombstoned, name namespaced, and has a MERGED_INTO audit edge.
    let tomb = count(
        &g,
        "MATCH (b:Entity:Tombstoned {tenant_id:$t}) WHERE b.name STARTS WITH 'asprin#merged#' AND b.merged_into IS NOT NULL RETURN count(b) AS c",
        t,
    )
    .await;
    assert_eq!(tomb, 1, "duplicate must be tombstoned with namespaced name");

    let audit = count(
        &g,
        "MATCH (:Entity:Tombstoned {tenant_id:$t})-[m:MERGED_INTO]->(:Entity {name:'aspirin'}) WHERE m.merged_by='tester' RETURN count(m) AS c",
        t,
    )
    .await;
    assert_eq!(audit, 1, "MERGED_INTO audit edge must record the merge");

    // find_candidates excludes the tombstoned node.
    let cands = store::find_candidates(&g, t, "DRUG", 100).await.unwrap();
    assert!(
        cands.iter().all(|c| !c.canonical_name.contains("#merged#")),
        "tombstoned node must not appear as a candidate"
    );

    // Survivor's alias set absorbed the duplicate's surface name.
    let alias_hit = count(
        &g,
        "MATCH (a:Entity {name:'aspirin', tenant_id:$t}) WHERE 'asprin' IN a.aliases RETURN count(a) AS c",
        t,
    )
    .await;
    assert_eq!(alias_hit, 1, "duplicate name must become a survivor alias");

    wipe_tenant(&g, t).await;
}

#[tokio::test]
async fn approve_review_triggers_merge() {
    let Some(g) = connect().await else { return };
    let t = "resolve_test_approve";
    wipe_tenant(&g, t).await;

    mk_entity(&g, t, "hypertension", "DISEASE").await;
    mk_entity(&g, t, "htn", "DISEASE").await;
    // Propose a duplicate (htn -> hypertension), as the flag path would.
    store::flag_duplicate(
        &g, t, "DISEASE", "htn", "hypertension", 0.9, 0.0, 0.9, "fuzzy", "stub", 3, false, "system",
    )
    .await
    .unwrap();

    let pending = store::review_queue(&g, t, 10).await.unwrap();
    assert_eq!(pending.len(), 1, "one pending proposal expected");

    // Approve → merges htn into hypertension.
    store::resolve_duplicate(&g, t, "DISEASE", "htn", "hypertension", true, "reviewer", 0.9, false)
        .await
        .unwrap();

    let tomb = count(
        &g,
        "MATCH (b:Entity:Tombstoned {tenant_id:$t}) WHERE b.name STARTS WITH 'htn#merged#' RETURN count(b) AS c",
        t,
    )
    .await;
    assert_eq!(tomb, 1, "approving a review must merge+tombstone the duplicate");

    wipe_tenant(&g, t).await;
}

#[tokio::test]
async fn merge_is_idempotent_flag_via_dream_pass() {
    let Some(g) = connect().await else { return };
    let t = "resolve_test_dream";
    wipe_tenant(&g, t).await;

    // Two same-type nodes with nearly identical stored embeddings — the dream
    // pass should flag them once, and a second run must not add a duplicate edge.
    mk_entity(&g, t, "metformin", "DRUG").await;
    mk_entity(&g, t, "metformine", "DRUG").await;
    for (n, e) in [("metformin", vec![1.0f64, 0.0, 0.0]), ("metformine", vec![0.99, 0.02, 0.0])] {
        g.run(
            query("MATCH (x:Entity {name:$n, tenant_id:$t}) SET x.embedding=$e, x.embed_model='stub', x.embed_dim=3")
                .param("n", n)
                .param("t", t)
                .param("e", e),
        )
        .await
        .unwrap();
    }

    let since = "2000-01-01T00:00:00Z";
    let n1 = store::dream_pass(&g, t, since, &store::ResolveParams { fuzzy_threshold: 0.8, semantic_threshold: 0.9, ..Default::default() }).await.unwrap();
    let n2 = store::dream_pass(&g, t, since, &store::ResolveParams { fuzzy_threshold: 0.8, semantic_threshold: 0.9, ..Default::default() }).await.unwrap();
    assert!(n1 >= 1, "dream pass should flag at least one pair, got {n1}");

    let edges = count(
        &g,
        "MATCH (:Entity {tenant_id:$t})-[d:DUPLICATE_OF]->(:Entity) RETURN count(d) AS c",
        t,
    )
    .await;
    assert!(edges >= 1, "expected a DUPLICATE_OF edge");
    // Idempotency: the second pass must not multiply edges beyond the first.
    assert!(edges <= n1.max(1) as i64, "dream pass must be idempotent (n2={n2}, edges={edges})");

    wipe_tenant(&g, t).await;
}
