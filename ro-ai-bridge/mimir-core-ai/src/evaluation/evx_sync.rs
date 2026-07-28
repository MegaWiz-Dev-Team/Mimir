//! Post-run evx sync — writes a completed QA run into the unified `evx_*`
//! layer so the scoreboard sees it immediately, with no manual backfill.
//!
//! The statements are the QA section of `scripts/eval_unify_backfill.sql`
//! scoped to ONE run (`WHERE run_id = ?`); keep the two in lockstep when the
//! mapping changes. Same idempotency model as the backfill: deterministic ids
//! (SHA2 of the natural key) + INSERT IGNORE, so re-running for an already
//! synced run inserts nothing. IGNORE also means values never refresh in
//! place — fine today (rejudge is a stub, summaries are write-once); if
//! re-scoring ever lands, metrics/items need ON DUPLICATE KEY UPDATE.
//!
//! HITL review rows (evx_item_review) are NOT synced here: human overrides
//! arrive after completion via the score-override UI, which still writes the
//! legacy tables only. They keep flowing in through the manual backfill until
//! that route migrates.
//!
//! Failures must never fail the eval run itself — the caller logs and moves
//! on; the scoreboard then lags until the next manual backfill catches it up.

use crate::services::db::DbPool;
use anyhow::Result;

/// Mirror one COMPLETED QA run (targets, dataset link, experiment, per-target
/// runs, metrics, items) into evx_*. Idempotent; cheap (all statements are
/// index-scoped to the run).
pub async fn sync_qa_run_to_evx(pool: &DbPool, run_id: &str) -> Result<()> {
    // 1. Targets — one per (agent, model) combo in this run's summaries.
    sqlx::query(
        "INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, config_json)
         SELECT DISTINCT SHA2(CONCAT_WS('|','agent', s.agent_name, s.model_id), 256),
                'agent', s.agent_name, s.model_id, NULL, NULL
         FROM eval_summary s WHERE s.run_id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    // 2. Dataset — only the benchmark dataset this run references (if any).
    sqlx::query(
        "INSERT IGNORE INTO evx_dataset (id, family, tenant_id, name, version, item_count, spec_json)
         SELECT CONCAT('qa:', d.id), 'qa', d.tenant_id, d.name, d.version, d.total_items,
                JSON_OBJECT('source', d.source)
         FROM eval_benchmark_datasets d
         JOIN eval_runs r
           ON JSON_VALID(r.config)
          AND JSON_UNQUOTE(JSON_EXTRACT(r.config,'$.benchmark_dataset_id')) = d.id
         WHERE r.id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    // 3. Experiment — the batch row (1:1 with eval_runs).
    sqlx::query(
        "INSERT IGNORE INTO evx_experiment
            (id, tenant_id, name, family, status, hypothesis, variable_under_test,
             baseline_experiment_id, is_champion, total_cost_usd, config_json,
             legacy_source, started_at, finished_at)
         SELECT r.id, NULL, r.name, 'qa', r.status, r.hypothesis, r.variable_under_test,
                r.baseline_run_id, COALESCE(r.is_champion,0), r.total_cost_usd,
                IF(JSON_VALID(r.config), r.config, NULL),
                'eval_runs', r.started_at, r.finished_at
         FROM eval_runs r WHERE r.id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    // 4. Runs — explode per (agent, model); dataset linked via config.
    sqlx::query(
        "INSERT IGNORE INTO evx_run
            (id, experiment_id, family, target_id, dataset_id, dataset_version,
             tenant_id, status, n_items, started_at, finished_at)
         SELECT
            SHA2(CONCAT_WS('|', s.run_id, s.agent_name, s.model_id), 256),
            s.run_id, 'qa',
            SHA2(CONCAT_WS('|','agent', s.agent_name, s.model_id), 256),
            IF(JSON_VALID(r.config) AND JSON_EXTRACT(r.config,'$.benchmark_dataset_id') IS NOT NULL,
               CONCAT('qa:', JSON_UNQUOTE(JSON_EXTRACT(r.config,'$.benchmark_dataset_id'))), NULL),
            NULL, NULL, 'COMPLETED', COALESCE(s.total_questions,0), r.started_at, r.finished_at
         FROM eval_summary s
         JOIN eval_runs r ON r.id = s.run_id
         WHERE s.run_id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    // 5. Metrics — one row each; overall_score is the single primary.
    for (name, col, unit, higher, primary) in [
        ("overall_score", "overall_score", "score", 1, 1),
        ("accuracy", "avg_accuracy", "score", 1, 0),
        ("completeness", "avg_completeness", "score", 1, 0),
        ("relevance", "avg_relevance", "score", 1, 0),
        ("safety", "avg_safety_score", "score", 1, 0),
        ("latency_ms", "avg_latency_ms", "ms", 0, 0),
    ] {
        sqlx::query(&format!(
            "INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
             SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256), '{name}', {col},
                    '{unit}', {higher}, {primary}, total_questions
             FROM eval_summary WHERE {col} IS NOT NULL AND run_id = ?",
        ))
        .bind(run_id)
        .execute(pool)
        .await?;
    }
    // rubric_score has no summary column — aggregate from eval_scores.
    sqlx::query(
        "INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
         SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'rubric_score',
                AVG(rubric_score),'points',1,0,COUNT(*)
         FROM eval_scores WHERE rubric_score IS NOT NULL AND run_id = ?
         GROUP BY run_id, agent_name, model_id",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    // 6. Items — per-question evidence; replicate_index folded into item_id.
    sqlx::query(
        "INSERT IGNORE INTO evx_item (run_id, item_id, score, payload_json)
         SELECT
            SHA2(CONCAT_WS('|', sc.run_id, sc.agent_name, sc.model_id), 256),
            COALESCE(CONCAT(sc.benchmark_item_id,'#',COALESCE(sc.replicate_index,0)), CONCAT('row:', sc.id)),
            sc.accuracy_score / 5.0,
            JSON_OBJECT('question', LEFT(sc.question,2000), 'expected', LEFT(sc.expected_answer,2000),
                        'actual', LEFT(sc.actual_answer,2000), 'judge_reasoning', LEFT(sc.judge_reasoning,2000),
                        'rubric_score', sc.rubric_score, 'safety_score', sc.safety_score,
                        'rubric_items', sc.rubric_items)
         FROM eval_scores sc WHERE sc.run_id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}
