-- ============================================================================
-- Unified Evaluation Storage — BACKFILL (one-shot script, NOT an auto-migration)
-- Date: 2026-06-05  ·  Revised per design review
--
-- Run MANUALLY, after the core migration (20260604120000) is applied AND after
-- the eval-table backup (scripts/eval_unify_backup.sh) is verified. It is kept
-- OUT of the sqlx auto-chain on purpose: it does large INSERT...SELECT over
-- eval_scores/rag_eval_queries and must not lock prod during a deploy.
--
--   mariadb mimir < ro-ai-bridge/scripts/eval_unify_backfill.sql
--
-- Idempotent: deterministic ids + INSERT IGNORE on natural keys → safe to
-- re-run. Covers agent-QA/HealthBench, RAG, OCR-text. OCR-layout spans + NER
-- are born native (no legacy rows).
--
-- Review fixes applied:
--   #1 replicate_index folded into QA item_id (was silently dropping replicates)
--   #2 HealthBench rubric_score + safety carried to items + aggregate metric
--   #3 human-override review backfilled into evx_item_review
--   #4 QA dataset linked via eval_runs.config->benchmark_dataset_id
--   #5 RAG target = (embed_model, rerank_model) only; weights moved to run.config
--   #11 OCR dataset pii_sensitivity derived from source (synthetic → none)
--   + RAG per-query items from rag_eval_queries; slice_dim/slice_val split
-- ============================================================================

-- ─── 1. TARGETS ─────────────────────────────────────────────────────────────
INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, config_json)
SELECT DISTINCT
    SHA2(CONCAT_WS('|','agent', s.agent_name, s.model_id), 256),
    'agent', s.agent_name, s.model_id, NULL, NULL
FROM eval_summary s;

-- #5 RAG target identity = models only (NOT weights — those are tuning knobs)
INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, config_json)
SELECT DISTINCT
    SHA2(CONCAT_WS('|','pipeline','rag', COALESCE(embed_model,''), COALESCE(rerank_model,'')), 256),
    'pipeline', COALESCE(embed_model,'rag'), NULL, 'rag',
    JSON_OBJECT('embed_model', embed_model, 'rerank_model', rerank_model)
FROM rag_eval_runs;

INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, quant, config_json)
SELECT DISTINCT
    SHA2(CONCAT_WS('|','model','ocr', engine, COALESCE(engine_version,'')), 256),
    'model', engine, NULL, engine, engine_version, NULL
FROM ocr_eval_results;

-- ─── 2. DATASETS ────────────────────────────────────────────────────────────
INSERT IGNORE INTO evx_dataset (id, family, tenant_id, name, version, item_count, spec_json)
SELECT CONCAT('rag:', id), 'rag', tenant_id, name, 1, 0, JSON_OBJECT('description', description)
FROM rag_eval_datasets;

-- #11 derive PII flag from source instead of hardcoding 'raw'
INSERT IGNORE INTO evx_dataset (id, family, tenant_id, name, version, item_count, pii_sensitivity, spec_json)
SELECT CONCAT('ocr:', id), 'ocr', tenant_id, name, version, image_count,
       IF(source = 'synthetic', 'none', 'raw'),
       JSON_OBJECT('source', source, 'gt_source_path', gt_source_path)
FROM ocr_eval_datasets;

-- #4 QA benchmark datasets
INSERT IGNORE INTO evx_dataset (id, family, tenant_id, name, version, item_count, spec_json)
SELECT CONCAT('qa:', id), 'qa', tenant_id, name, version, total_items, JSON_OBJECT('source', source)
FROM eval_benchmark_datasets;

-- ─── 3. EXPERIMENTS (batch level) ───────────────────────────────────────────
INSERT IGNORE INTO evx_experiment
    (id, tenant_id, name, family, status, hypothesis, variable_under_test,
     baseline_experiment_id, is_champion, total_cost_usd, config_json,
     legacy_source, started_at, finished_at)
SELECT r.id, NULL, r.name, 'qa', r.status, r.hypothesis, r.variable_under_test,
       r.baseline_run_id, COALESCE(r.is_champion,0), r.total_cost_usd,
       IF(JSON_VALID(r.config), r.config, NULL),
       'eval_runs', r.started_at, r.finished_at
FROM eval_runs r;

INSERT IGNORE INTO evx_experiment
    (id, tenant_id, name, family, status, config_json, legacy_source, started_at, finished_at)
SELECT id, tenant_id, name, 'rag', COALESCE(status,'COMPLETED'),
       JSON_OBJECT('dataset_name', dataset_name, 'top_k', top_k, 'rerank_enabled', rerank_enabled),
       'rag_eval_runs', started_at, finished_at
FROM rag_eval_runs;

INSERT IGNORE INTO evx_experiment
    (id, tenant_id, name, family, status, config_json, legacy_source, started_at, finished_at)
SELECT id, tenant_id, name, 'ocr', 'COMPLETED',
       JSON_OBJECT('prompt_label', prompt_label, 'engines', engines),
       'ocr_eval_runs', started_at, finished_at
FROM ocr_eval_runs;

-- ─── 4. RUNS (one target on one dataset) ────────────────────────────────────
-- QA: explode per (agent, model); #4 link dataset from config
INSERT IGNORE INTO evx_run
    (id, experiment_id, family, target_id, dataset_id, dataset_version, tenant_id, status, n_items, started_at, finished_at)
SELECT
    SHA2(CONCAT_WS('|', s.run_id, s.agent_name, s.model_id), 256),
    s.run_id, 'qa',
    SHA2(CONCAT_WS('|','agent', s.agent_name, s.model_id), 256),
    IF(JSON_VALID(r.config) AND JSON_EXTRACT(r.config,'$.benchmark_dataset_id') IS NOT NULL,
       CONCAT('qa:', JSON_UNQUOTE(JSON_EXTRACT(r.config,'$.benchmark_dataset_id'))), NULL),
    NULL, NULL, 'COMPLETED', COALESCE(s.total_questions,0), r.started_at, r.finished_at
FROM eval_summary s
JOIN eval_runs r ON r.id = s.run_id;

-- RAG: 1:1; #5 weights live on the run, not the target identity
INSERT IGNORE INTO evx_run
    (id, experiment_id, family, target_id, dataset_id, tenant_id, status, n_items, judge_model, config_json, started_at, finished_at)
SELECT
    id, id, 'rag',
    SHA2(CONCAT_WS('|','pipeline','rag', COALESCE(embed_model,''), COALESCE(rerank_model,'')), 256),
    CONCAT('rag:', dataset_id), tenant_id, COALESCE(status,'COMPLETED'),
    COALESCE(total_queries,0), judge_model,
    JSON_OBJECT('weight_vector', weight_vector, 'weight_tree', weight_tree,
                'weight_graph', weight_graph, 'top_k', top_k,
                'rerank_strategy', rerank_strategy, 'rerank_final_k', rerank_final_k),
    started_at, finished_at
FROM rag_eval_runs;

-- OCR: explode per engine
INSERT IGNORE INTO evx_run
    (id, experiment_id, family, target_id, dataset_id, tenant_id, status, n_items, started_at, finished_at)
SELECT
    SHA2(CONCAT_WS('|', res.run_id, res.engine, COALESCE(res.engine_version,'')), 256),
    res.run_id, 'ocr',
    SHA2(CONCAT_WS('|','model','ocr', res.engine, COALESCE(res.engine_version,'')), 256),
    CONCAT('ocr:', run.dataset_id), run.tenant_id, 'COMPLETED',
    COUNT(DISTINCT res.case_id), run.started_at, run.finished_at
FROM ocr_eval_results res
JOIN ocr_eval_runs run ON run.id = res.run_id
GROUP BY res.run_id, res.engine, res.engine_version, run.dataset_id, run.tenant_id, run.started_at, run.finished_at;

-- ─── 5. METRICS (normalized) ────────────────────────────────────────────────
-- QA / agent (1-5 rubric). Exactly one is_primary per run (overall_score).
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'overall_score',overall_score,'score_1_5',1,1,total_questions FROM eval_summary WHERE overall_score IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'accuracy',avg_accuracy,'score_1_5',1,0,total_questions FROM eval_summary WHERE avg_accuracy IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'completeness',avg_completeness,'score_1_5',1,0,total_questions FROM eval_summary WHERE avg_completeness IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'relevance',avg_relevance,'score_1_5',1,0,total_questions FROM eval_summary WHERE avg_relevance IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'safety',avg_safety_score,'score',1,0,total_questions FROM eval_summary WHERE avg_safety_score IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'latency_ms',avg_latency_ms,'ms',0,0,total_questions FROM eval_summary WHERE avg_latency_ms IS NOT NULL;
-- #2 rubric_score has no summary column → aggregate from eval_scores
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,agent_name,model_id),256),'rubric_score',AVG(rubric_score),'points',1,0,COUNT(*)
FROM eval_scores WHERE rubric_score IS NOT NULL GROUP BY run_id, agent_name, model_id;

-- RAG (overall + per-channel slices). hit_rate is_primary.
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'hit_rate',hit_rate,'ratio',1,1,total_queries FROM rag_eval_runs WHERE hit_rate IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'mrr',mrr,'ratio',1,0,total_queries FROM rag_eval_runs WHERE mrr IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'ndcg',ndcg,'ratio',1,0,total_queries FROM rag_eval_runs WHERE ndcg IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'precision_at_k',precision_at_k,'ratio',1,0,total_queries FROM rag_eval_runs WHERE precision_at_k IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'recall_at_k',recall_at_k,'ratio',1,0,total_queries FROM rag_eval_runs WHERE recall_at_k IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'faithfulness',avg_faithfulness,'ratio',1,0,total_queries FROM rag_eval_runs WHERE avg_faithfulness IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'answer_relevancy',avg_answer_relevancy,'ratio',1,0,total_queries FROM rag_eval_runs WHERE avg_answer_relevancy IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'latency_ms',avg_latency_ms,'ms',0,0,total_queries FROM rag_eval_runs WHERE avg_latency_ms IS NOT NULL;
-- per-channel slices (slice_dim/slice_val split)
INSERT IGNORE INTO evx_metric (run_id, name, slice_dim, slice_val, value, unit, higher_is_better, n)
SELECT id,'hit_rate','channel','vector',vector_hit_rate,'ratio',1,total_queries FROM rag_eval_runs WHERE vector_hit_rate IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, slice_dim, slice_val, value, unit, higher_is_better, n)
SELECT id,'hit_rate','channel','tree',tree_hit_rate,'ratio',1,total_queries FROM rag_eval_runs WHERE tree_hit_rate IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, slice_dim, slice_val, value, unit, higher_is_better, n)
SELECT id,'hit_rate','channel','graph',graph_hit_rate,'ratio',1,total_queries FROM rag_eval_runs WHERE graph_hit_rate IS NOT NULL;

-- OCR (aggregate per run × engine; CER/WER lower-is-better). cer is_primary.
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,engine,COALESCE(engine_version,'')),256),'cer',AVG(cer),'ratio',0,1,COUNT(*)
FROM ocr_eval_results WHERE cer IS NOT NULL GROUP BY run_id, engine, engine_version;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT SHA2(CONCAT_WS('|',run_id,engine,COALESCE(engine_version,'')),256),'wer',AVG(wer),'ratio',0,0,COUNT(*)
FROM ocr_eval_results WHERE wer IS NOT NULL GROUP BY run_id, engine, engine_version;

-- ─── 6. ITEMS ───────────────────────────────────────────────────────────────
-- QA items; #1 replicate_index in item_id; #2 rubric+safety in payload
INSERT IGNORE INTO evx_item (run_id, item_id, score, payload_json)
SELECT
    SHA2(CONCAT_WS('|', sc.run_id, sc.agent_name, sc.model_id), 256),
    COALESCE(CONCAT(sc.benchmark_item_id,'#',COALESCE(sc.replicate_index,0)), CONCAT('row:', sc.id)),
    sc.accuracy_score / 5.0,
    JSON_OBJECT('question', LEFT(sc.question,2000), 'expected', LEFT(sc.expected_answer,2000),
                'actual', LEFT(sc.actual_answer,2000), 'judge_reasoning', LEFT(sc.judge_reasoning,2000),
                'rubric_score', sc.rubric_score, 'safety_score', sc.safety_score,
                'rubric_items', sc.rubric_items)
FROM eval_scores sc;

-- #3 human-override review → evx_item_review (only reviewed rows)
INSERT IGNORE INTO evx_item_review (run_id, item_id, human_scores_json, notes, reviewed_by, reviewed_at)
SELECT
    SHA2(CONCAT_WS('|', sc.run_id, sc.agent_name, sc.model_id), 256),
    COALESCE(CONCAT(sc.benchmark_item_id,'#',COALESCE(sc.replicate_index,0)), CONCAT('row:', sc.id)),
    JSON_OBJECT('accuracy', sc.human_accuracy_score, 'completeness', sc.human_completeness_score,
                'relevance', sc.human_relevance_score, 'safety', sc.human_safety_score),
    sc.human_notes, sc.reviewed_by, sc.reviewed_at
FROM eval_scores sc
WHERE sc.reviewed_at IS NOT NULL OR sc.reviewed_by IS NOT NULL;

-- RAG per-query items from rag_eval_queries
INSERT IGNORE INTO evx_item (run_id, item_id, score, correct, payload_json)
SELECT
    q.run_id, CONCAT('q:', q.id), q.reciprocal_rank, q.hit,
    JSON_OBJECT('query', LEFT(q.query,2000), 'matched_at_rank', q.matched_at_rank,
                'ndcg', q.ndcg_score, 'faithfulness', q.faithfulness,
                'answer_relevancy', q.answer_relevancy, 'difficulty', q.difficulty,
                'channels', JSON_OBJECT('vector', q.vector_contributed, 'tree', q.tree_contributed, 'graph', q.graph_contributed))
FROM rag_eval_queries q;

-- OCR items (per case × engine)
INSERT IGNORE INTO evx_item (run_id, item_id, score, payload_json)
SELECT
    SHA2(CONCAT_WS('|', res.run_id, res.engine, COALESCE(res.engine_version,'')), 256),
    res.case_id, NULL,
    JSON_OBJECT('cer', res.cer, 'wer', res.wer, 'status', res.status, 'extracted_chars', res.extracted_chars)
FROM ocr_eval_results res;

-- ─── 6b. OCR-LAYOUT family (mAP / parity) → runs, metrics, items, spans ──────
-- target = layout model; run = 1:1; run-level metrics live in summary JSON
-- whose shape depends on eval_kind (mAP vs parity).
INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, config_json)
SELECT DISTINCT
    SHA2(CONCAT_WS('|','model','ocr_layout', model_name, COALESCE(model_sha256,'')), 256),
    'model', model_name, NULL, eval_kind,
    JSON_OBJECT('model_sha256', model_sha256, 'syn_version', syn_version)
FROM ocr_layout_eval_runs;

INSERT IGNORE INTO evx_dataset (id, family, tenant_id, name, version, item_count, pii_sensitivity, spec_json)
SELECT DISTINCT CONCAT('ocrlay:', dataset_name), 'ocr_layout', tenant_id, dataset_name, 1, n_images,
       IF(is_synthetic, 'none', 'raw'), JSON_OBJECT('dataset_hash', dataset_hash)
FROM ocr_layout_eval_runs;

INSERT IGNORE INTO evx_experiment
    (id, tenant_id, name, family, status, config_json, legacy_source, started_at, finished_at)
SELECT id, tenant_id, CONCAT(eval_kind,' · ',model_name), 'ocr_layout', 'COMPLETED',
       JSON_OBJECT('eval_kind', eval_kind, 'iou_threshold', iou_threshold, 'commit_sha', commit_sha),
       'ocr_layout_eval_runs', started_at, finished_at
FROM ocr_layout_eval_runs;

INSERT IGNORE INTO evx_run
    (id, experiment_id, family, target_id, dataset_id, tenant_id, status, n_items, git_sha, config_json, started_at, finished_at)
SELECT id, id, 'ocr_layout',
       SHA2(CONCAT_WS('|','model','ocr_layout', model_name, COALESCE(model_sha256,'')), 256),
       CONCAT('ocrlay:', dataset_name), tenant_id, 'COMPLETED', n_images, commit_sha,
       JSON_OBJECT('eval_kind', eval_kind, 'iou_threshold', iou_threshold), started_at, finished_at
FROM ocr_layout_eval_runs;

-- metrics: mAP kind → ap50 (primary) + precision/recall; parity → max_abs_diff (primary)
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'ap50', JSON_VALUE(summary,'$.class_agnostic.ap50'),'ratio',1,1,n_images
FROM ocr_layout_eval_runs WHERE eval_kind='mAP' AND JSON_VALUE(summary,'$.class_agnostic.ap50') IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'precision', JSON_VALUE(summary,'$.class_agnostic.precision'),'ratio',1,0,n_images
FROM ocr_layout_eval_runs WHERE eval_kind='mAP' AND JSON_VALUE(summary,'$.class_agnostic.precision') IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'recall', JSON_VALUE(summary,'$.class_agnostic.recall'),'ratio',1,0,n_images
FROM ocr_layout_eval_runs WHERE eval_kind='mAP' AND JSON_VALUE(summary,'$.class_agnostic.recall') IS NOT NULL;
INSERT IGNORE INTO evx_metric (run_id, name, value, unit, higher_is_better, is_primary, n)
SELECT id,'max_abs_diff', JSON_VALUE(summary,'$.max_abs_diff'),'ratio',0,1,n_images
FROM ocr_layout_eval_runs WHERE eval_kind='parity' AND JSON_VALUE(summary,'$.max_abs_diff') IS NOT NULL;

-- items (per image)
INSERT IGNORE INTO evx_item (run_id, item_id, score, payload_json)
SELECT run_id, id, NULL,
       JSON_OBJECT('image', COALESCE(image_name, image_hash), 'n_gt', n_gt, 'n_pred', n_pred,
                   'n_matched', n_matched, 'metrics', metrics)
FROM ocr_layout_eval_items;

-- spans: each region_match row → up to 2 spans (gold bbox + pred bbox).
-- dedup_key makes the span insert idempotent (evx_span is otherwise append-only).
INSERT IGNORE INTO evx_span (run_id, item_id, bbox, label, source, confidence, dedup_key)
SELECT run_id, item_id, JSON_ARRAY(bbox_gt_x,bbox_gt_y,bbox_gt_w,bbox_gt_h),
       COALESCE(class_true,'region'), 'gold', NULL,
       SHA2(CONCAT_WS('|', id, 'gold'), 256)
FROM ocr_layout_region_match WHERE bbox_gt_x IS NOT NULL;
INSERT IGNORE INTO evx_span (run_id, item_id, bbox, label, source, confidence, dedup_key)
SELECT run_id, item_id, JSON_ARRAY(bbox_pred_x,bbox_pred_y,bbox_pred_w,bbox_pred_h),
       COALESCE(class_pred,'region'), 'pred', confidence,
       SHA2(CONCAT_WS('|', id, 'pred'), 256)
FROM ocr_layout_region_match WHERE bbox_pred_x IS NOT NULL;

-- ─── 7. n_items refresh ─────────────────────────────────────────────────────
UPDATE evx_run r SET n_items = (SELECT COUNT(*) FROM evx_item i WHERE i.run_id = r.id)
WHERE r.n_items = 0;

-- ============================================================================
-- VERIFY after run (expect equality):
--   SELECT family, COUNT(*) FROM evx_run GROUP BY family;
--     qa  == (SELECT COUNT(*) FROM eval_summary)
--     rag == (SELECT COUNT(*) FROM rag_eval_runs)
--     ocr == (SELECT COUNT(DISTINCT CONCAT(run_id,engine,IFNULL(engine_version,''))) FROM ocr_eval_results)
--   SELECT COUNT(*) FROM evx_item;          -- ~ eval_scores + rag_eval_queries + ocr_eval_results
--   SELECT COUNT(*) FROM evx_item_review;   -- == eval_scores WHERE reviewed_at OR reviewed_by NOT NULL
--   -- one-primary invariant holds (else the unique key would have errored):
--   SELECT run_id, COUNT(*) c FROM evx_metric WHERE is_primary=1 GROUP BY run_id HAVING c>1;  -- expect 0 rows
-- ============================================================================
