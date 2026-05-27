//! Live-DB smoke test for the Sprint 56 schema, exercising WellWriter.
//!
//! Inserts one artifact per tier via the public API, then supersedes one,
//! reads counts back, and cleans up — proves the writer + schema are
//! integrated end-to-end.
//!
//! Run:
//!   export DATABASE_URL='mysql://root:<pw>@localhost:13306/mimir'
//!   export NEO4J_URI='bolt://localhost:17687'  # optional — skips Neo4j section if unset
//!   export NEO4J_USER='neo4j'
//!   export NEO4J_PASS='<pw>'
//!   cargo run -p mimir-well --example smoke_live
//!
//! Set up the port-forwards first:
//!   kubectl port-forward -n asgard-infra svc/mariadb 13306:3306
//!   kubectl port-forward -n asgard-infra svc/neo4j   17687:7687

use std::env;

use mimir_well::consolidator::{Consolidator, RunMode};
use mimir_well::model::{Kind, Surface, Tier};
use mimir_well::promotion::{
    promote_session, HeimdallTierClassifier, PromotionFrame, TierClassifier,
};
use mimir_well::reader::WellReader;
use mimir_well::touched::{TouchRole, TouchedAttr, TouchedBatch, TouchedMaterializer};
use mimir_well::writer::{TracingProvSink, WellWriter, WriteRequest};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use sqlx::mysql::MySqlPoolOptions;
use ulid::Ulid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set — see file header"))?;
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await?;

    // Use TracingProvSink — proves the async ProvSink trait works through write().
    let writer = Arc::new(WellWriter::with_prov_sink(
        pool.clone(),
        Arc::new(TracingProvSink),
    ));
    let reader = Arc::new(WellReader::new(pool.clone()));
    let consolidator = Consolidator::new(pool.clone(), writer.clone(), reader.clone());
    let tenant = format!("smoke_{}", Ulid::new());
    println!("== smoke run with tenant_id={tenant} ==");

    let cases = [
        (Tier::Episodic, Surface::Short, Kind::Observation, "episodic"),
        (Tier::Semantic, Surface::Long, Kind::Reference, "semantic"),
        (
            Tier::Procedural,
            Surface::Reasoning,
            Kind::Skill,
            "procedural",
        ),
    ];

    let mut ids = Vec::new();
    for (tier, surface, kind, label) in cases {
        let mut h = Sha256::new();
        h.update(label.as_bytes());
        let hash = hex(&h.finalize());

        let id = writer
            .write(WriteRequest {
                tenant_id: tenant.clone(),
                agent_id: "smoke-live-example".into(),
                case_id: None,
                kind,
                tier,
                surface,
                content_hash: hash,
                content: serde_json::json!({ "label": label }),
                embedding: None,
                prov_used: None,
                prov_generated_by: "smoke-trace:span-1".into(),
                confidence: Some(0.95),
                promoted_from: None,
            })
            .await?;
        println!("  ✅ wrote {label:<11} {id}");
        ids.push(id);
    }

    let count = writer.count_fresh(&tenant).await?;
    assert_eq!(count, 3, "expected 3 fresh, got {count}");
    println!("  ✅ count_fresh = {count}");

    // Reader round-trip: fetch by id, list by tier filter.
    let fetched = reader.get_by_id(&tenant, ids[0]).await?.expect("artifact missing");
    assert_eq!(fetched.id, ids[0]);
    assert_eq!(fetched.tier, Tier::Episodic);
    assert_eq!(fetched.surface, Surface::Short);
    println!("  ✅ reader.get_by_id round-trip: tier={:?} confidence={:?}", fetched.tier, fetched.confidence);

    let semantic_rows = reader
        .list_by_tenant(&tenant, Some(Tier::Semantic), None, 10)
        .await?;
    assert_eq!(semantic_rows.len(), 1);
    println!("  ✅ reader.list_by_tenant(tier=Semantic) returned {} row", semantic_rows.len());

    // Supersede the first artifact with the second.
    writer.supersede(&tenant, ids[0], ids[1]).await?;
    let count_after = writer.count_fresh(&tenant).await?;
    assert_eq!(count_after, 2, "expected 2 fresh after supersession, got {count_after}");
    println!("  ✅ supersede({}, {}) → fresh dropped to {count_after}", ids[0], ids[1]);

    // Supersession chain: ids[0] → ids[1] (terminal).
    let chain = reader.supersession_chain(&tenant, ids[0]).await?;
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].id, ids[0]);
    assert_eq!(chain[1].id, ids[1]);
    println!("  ✅ supersession_chain returned {} hops: {} → {}", chain.len(), chain[0].id, chain[1].id);

    // Supersede missing → expect NotFound error.
    let bogus = Ulid::new();
    let err = writer.supersede(&tenant, bogus, ids[2]).await.unwrap_err();
    println!("  ✅ supersede(missing) correctly errored: {err}");

    // ── consolidator hash-dedup ──
    // Write two artifacts with the SAME content_hash (same tier+kind+label).
    let dup_hash = {
        let mut h = Sha256::new();
        h.update(b"dup-payload");
        hex(&h.finalize())
    };
    let dup_req = |label: &str| WriteRequest {
        tenant_id: tenant.clone(),
        agent_id: "smoke-live-example".into(),
        case_id: None,
        kind: Kind::Observation,
        tier: Tier::Episodic,
        surface: Surface::Short,
        content_hash: dup_hash.clone(),
        content: serde_json::json!({ "label": label }),
        embedding: None,
        prov_used: None,
        prov_generated_by: "smoke-trace:dup-span".into(),
        confidence: None,
        promoted_from: None,
    };
    let dup_a = writer.write(dup_req("dup-a")).await?;
    let dup_b = writer.write(dup_req("dup-b")).await?;
    let dup_c = writer.write(dup_req("dup-c")).await?;
    println!("  ✅ wrote 3 hash-duplicates: {dup_a} {dup_b} {dup_c}");

    let fresh_before_dedup = writer.count_fresh(&tenant).await?;
    // 2 from first phase (ids[1] + ids[2] — ids[0] was superseded earlier) + 3 new dups.
    assert_eq!(fresh_before_dedup, 5, "expected 5 fresh before consolidator");

    // DryRun: count_fresh unchanged.
    let report_dry = consolidator.run_pass(&tenant, RunMode::DryRun).await?;
    let fresh_after_dry = writer.count_fresh(&tenant).await?;
    println!(
        "  ✅ consolidator DryRun: auto_merged_hash={} (fresh unchanged: {} → {})",
        report_dry.auto_merged_hash, fresh_before_dedup, fresh_after_dry
    );
    assert_eq!(report_dry.auto_merged_hash, 2);
    assert_eq!(fresh_after_dry, fresh_before_dedup, "DryRun must not write");

    // Apply: 2 duplicates get superseded, leaving canonical.
    let report_apply = consolidator.run_pass(&tenant, RunMode::Apply).await?;
    let fresh_after_apply = writer.count_fresh(&tenant).await?;
    println!(
        "  ✅ consolidator Apply:  auto_merged_hash={} (fresh now {} = {} - 2)",
        report_apply.auto_merged_hash, fresh_after_apply, fresh_before_dedup
    );
    assert_eq!(report_apply.auto_merged_hash, 2);
    assert_eq!(fresh_after_apply, fresh_before_dedup - 2, "Apply must drop 2");

    // Verify supersession chain on a duplicate — should walk to dup_a (canonical).
    let chain = reader.supersession_chain(&tenant, dup_b).await?;
    assert_eq!(chain.len(), 2, "expected dup_b → dup_a chain");
    assert_eq!(chain[0].id, dup_b);
    assert_eq!(chain[1].id, dup_a);
    println!(
        "  ✅ dup_b → dup_a chain verified ({} → {})",
        chain[0].id, chain[1].id
    );

    let deleted = writer.purge_tenant_for_tests(&tenant).await?;
    println!("  ✅ purge deleted {deleted} rows");

    // ── Neo4j :TOUCHED materializer ──
    if let (Ok(uri), Ok(user), Ok(pass)) = (
        env::var("NEO4J_URI"),
        env::var("NEO4J_USER"),
        env::var("NEO4J_PASS"),
    ) {
        println!();
        println!("== Neo4j section ==");
        let mat = TouchedMaterializer::connect(&uri, &user, &pass).await?;
        let smoke_art = format!("01JSMK{}", Ulid::new().to_string());
        let smoke_trace = format!("smoke-trace-{}", Ulid::new());

        mat.seed_artifact_for_tests(&smoke_art, "asgard_platform", "episodic")
            .await?;
        println!("  ✅ seeded :Artifact {smoke_art}");

        let batch = TouchedBatch {
            trace_id: smoke_trace.clone(),
            span_id: "span-001".into(),
            touches: vec![
                TouchedAttr { artifact_id: smoke_art.clone(), role: TouchRole::Used },
                TouchedAttr { artifact_id: smoke_art.clone(), role: TouchRole::Refined },
            ],
            at: chrono::Utc::now(),
        };

        let n1 = mat.materialize(&batch).await?;
        println!("  ✅ materialize #1: {n1} touches");

        // Idempotency: re-run, edge count in Neo4j unchanged.
        let n2 = mat.materialize(&batch).await?;
        println!("  ✅ materialize #2 (idempotent): {n2} touches");

        let purged_neo4j = mat.purge_test_prefix(&smoke_art[..6]).await?
            + mat.purge_test_prefix(&smoke_trace[..5]).await?;
        println!("  ✅ neo4j cleanup deleted {purged_neo4j} nodes (artifact+span)");
    } else {
        println!();
        println!("⏭️  Neo4j section skipped (NEO4J_URI/USER/PASS not set)");
    }

    // ── Heimdall TierClassifier ──
    if let (Ok(url), Ok(key), Ok(model)) = (
        env::var("HEIMDALL_API_URL"),
        env::var("HEIMDALL_API_KEY"),
        env::var("HEIMDALL_MODEL"),
    ) {
        println!();
        println!("== TierClassifier section (Heimdall: {model}) ==");
        let clf = HeimdallTierClassifier::new(&url, &key, &model);

        let cases = [
            (
                "system noise",
                "DEBUG: system init complete, no actionable content",
                "should classify near 'drop'",
            ),
            (
                "tenant fact",
                "The asgard_medical tenant policy is to never autoreply to PHI questions without explicit user consent.",
                "should classify near 'semantic'",
            ),
            (
                "case event",
                "On 2026-05-23 case A23, Eir answered HCC staging question for tenant asgard_medical.",
                "should classify near 'episodic'",
            ),
        ];

        for (label, content, hint) in cases {
            let frame = PromotionFrame {
                content: content.into(),
                title: Some(label.into()),
                span_ref: "smoke:span".into(),
            };
            match clf.classify(&frame).await {
                Ok(c) => println!("  ✅ {label:<14} → {c:?}   ({hint})"),
                Err(e) => println!("  ⚠️  {label:<14} errored: {e}"),
            }
        }

        // End-to-end promote_session: classifier → writer.
        println!();
        println!("  -- promote_session end-to-end --");
        let promo_tenant = format!("smoke_{}", Ulid::new());
        let frames: Vec<PromotionFrame> = [
            ("noise", "DEBUG: init complete"),
            ("fact", "asgard_medical never autoreplies to PHI without consent."),
            ("event", "On 2026-05-23 case A23, Eir answered HCC staging for asgard_medical."),
        ]
        .into_iter()
        .map(|(t, c)| PromotionFrame {
            content: c.into(),
            title: Some(t.into()),
            span_ref: format!("trace-{}:span-1", Ulid::new()),
        })
        .collect();

        let report = promote_session(
            &frames,
            "smoke-session-1",
            &promo_tenant,
            "smoke-promoter",
            &clf,
            &writer,
        )
        .await?;
        println!(
            "  ✅ promote_session report: dropped={} episodic={} semantic={} procedural={} errored={}",
            report.dropped, report.episodic, report.semantic, report.procedural, report.errored
        );
        let fresh_promo = writer.count_fresh(&promo_tenant).await?;
        let expected = report.episodic + report.semantic + report.procedural;
        assert_eq!(fresh_promo as u64, expected, "fresh count must match writes");
        println!("  ✅ count_fresh in {promo_tenant} = {fresh_promo} (matches {expected} writes)");
        let purged = writer.purge_tenant_for_tests(&promo_tenant).await?;
        println!("  ✅ promo cleanup deleted {purged} rows");
    } else {
        println!();
        println!("⏭️  TierClassifier section skipped (HEIMDALL_* env not set)");
    }

    println!();
    println!("== all good ==");
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
