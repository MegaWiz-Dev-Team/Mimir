-- ============================================================================
-- Unified Evaluation Storage — Core Layer (additive, non-destructive)
-- Date: 2026-06-04
--
-- WHY: eval storage sprawled into 4 separate "runs" tables (eval_runs,
-- rag_eval_runs, ocr_eval_runs, ocr_layout_eval_runs) and 4 "dataset" tables,
-- with metrics stored inconsistently (eval_summary.avg_* columns vs
-- rag_eval_runs.hit_rate/mrr inline vs ocr_eval_results.cer). eval_scores
-- became a god-table of nullable per-type columns. There is no single
-- scoreboard that works across QA / RAG / OCR / NER / coding.
--
-- THIS MIGRATION adds a unifying `evx_*` layer ALONGSIDE the legacy tables.
-- Nothing legacy is dropped or altered — legacy writers keep working during
-- cutover. Backfill is a separate migration (20260604120100). Rollback =
-- ignore evx_*.
--
-- MODEL: experiment (a batch / A-B) ──< run (ONE target on ONE dataset)
--        run ──< metric  (normalized: every score is one row → 1 scoreboard)
--        run ──< item    (per-item evidence; type payload in JSON)
--        run ──< artifact (heavy diagnostic blobs by reference)
--        run ──< span     (satellite for NER / extraction / OCR-layout)
-- ============================================================================

-- ─── evx_target : the thing under test (replaces overloaded agent_name) ──────
-- Deterministic id = SHA2 of the natural key so backfill and live writers
-- converge on the same row without a lookup round-trip.
CREATE TABLE IF NOT EXISTS evx_target (
    id          CHAR(64)     NOT NULL PRIMARY KEY COMMENT 'SHA2-256 of natural key',
    kind        VARCHAR(20)  NOT NULL COMMENT 'model | agent | pipeline | runtime_variant',
    name        VARCHAR(120) NOT NULL COMMENT 'logical name e.g. skuggi-ner, eir-sleep, mlx-q4',
    model_id    VARCHAR(100) NULL     COMMENT 'FK ai_models when applicable',
    runtime     VARCHAR(40)  NULL     COMMENT 'pythainlp | onnx | coreml-ane | mlx | gemini ...',
    quant       VARCHAR(40)  NULL     COMMENT 'q4 | q8 | fp16 ...',
    config_json JSON         NULL     COMMENT 'weights, hops, thresholds — anything that defines the variant',
    created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_kind_name (kind, name),
    INDEX idx_model     (model_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_dataset : unified gold sets (replaces 4 *_datasets tables) ──────────
CREATE TABLE IF NOT EXISTS evx_dataset (
    id              VARCHAR(80)  NOT NULL PRIMARY KEY COMMENT 'family-prefixed or reused legacy uuid',
    family          VARCHAR(30)  NOT NULL COMMENT 'qa | rag | ocr | ocr_layout | ner | coding | stt | rubric ...',
    tenant_id       VARCHAR(50)  NULL,
    name            VARCHAR(160) NOT NULL,
    version         INT          NOT NULL DEFAULT 1,
    item_count      INT          NOT NULL DEFAULT 0,
    pii_sensitivity VARCHAR(20)  NOT NULL DEFAULT 'none' COMMENT 'none | pseudonymized | raw',
    raw_store_ref   TEXT         NULL COMMENT 'pointer to PII-segregated store when sensitivity!=none; never raw PII here',
    spec_json       JSON         NULL COMMENT 'scoring_fn, source, gt_source_path, etc.',
    created_at      TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_family_name_version (family, name, version),
    INDEX idx_family (family)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_experiment : the batch / A-B grouping (= legacy *_runs at batch level)
CREATE TABLE IF NOT EXISTS evx_experiment (
    id                  VARCHAR(36)   NOT NULL PRIMARY KEY,
    tenant_id           VARCHAR(50)   NULL,
    name                VARCHAR(255)  NULL,
    family              VARCHAR(30)   NOT NULL COMMENT 'denormalized for fast scoreboard filtering',
    status              VARCHAR(20)   NOT NULL DEFAULT 'PENDING',
    hypothesis          TEXT          NULL,
    variable_under_test VARCHAR(255)  NULL,
    baseline_experiment_id VARCHAR(36) NULL,
    is_champion         TINYINT(1)    NOT NULL DEFAULT 0,
    total_cost_usd      DECIMAL(12,5) NULL,
    config_json         JSON          NULL,
    legacy_source       VARCHAR(40)   NULL COMMENT 'which legacy table this was backfilled from',
    started_at          TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at         TIMESTAMP     NULL,
    INDEX idx_family_started (family, started_at),
    INDEX idx_champion       (is_champion)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_run : ONE target on ONE dataset (the unit the scoreboard ranks) ─────
CREATE TABLE IF NOT EXISTS evx_run (
    id            VARCHAR(64)  NOT NULL PRIMARY KEY COMMENT 'reused legacy uuid OR SHA2(batch|target) when a legacy batch exploded per-target',
    experiment_id VARCHAR(36)  NULL,
    family        VARCHAR(30)  NOT NULL,
    target_id     CHAR(64)     NOT NULL,
    dataset_id    VARCHAR(80)  NULL,
    dataset_version INT        NULL,
    tenant_id     VARCHAR(50)  NULL,
    status        VARCHAR(20)  NOT NULL DEFAULT 'COMPLETED',
    n_items       INT          NOT NULL DEFAULT 0,
    git_sha       VARCHAR(40)  NULL,
    judge_model   VARCHAR(100) NULL COMMENT 'for LLM-judge families — judge drift = score drift, record it',
    config_json   JSON         NULL,
    started_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at   TIMESTAMP    NULL,
    INDEX idx_experiment (experiment_id),
    INDEX idx_family     (family),
    INDEX idx_target     (target_id),
    INDEX idx_dataset    (dataset_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_metric : THE unifier. Every score, any family, is one row. ─────────
-- Scoreboard = SELECT ... WHERE is_primary GROUP BY target. Per-slice
-- breakdown uses (slice_dim, slice_val) as TWO columns — so "all person slices
-- across runs" is GROUP BY slice_val WHERE slice_dim='entity_type', no string
-- parsing. This is what makes QA/OCR/NER/coding "ดูง่าย" in one view.
-- `primary_one` is a generated column (1 when is_primary, else NULL); the unique
-- key on (run_id, primary_one) ENFORCES at most one primary metric per run —
-- multiple NULLs are allowed, so non-primary rows never conflict.
CREATE TABLE IF NOT EXISTS evx_metric (
    id              BIGINT       AUTO_INCREMENT PRIMARY KEY,
    run_id          VARCHAR(64)  NOT NULL,
    name            VARCHAR(60)  NOT NULL COMMENT 'accuracy | recall | leak_rate | cer | wer | hit_rate | mrr | ndcg | faithfulness | p95_latency_ms | ane_residency_pct ...',
    slice_dim       VARCHAR(40)  NOT NULL DEFAULT '' COMMENT 'entity_type | doc_type | channel | code_system  ("" = overall)',
    slice_val       VARCHAR(120) NOT NULL DEFAULT '' COMMENT 'person | lab | vector | icd10cm  ("" = overall)',
    value           DOUBLE       NULL,
    unit            VARCHAR(20)  NULL COMMENT 'score_1_5 | pct | ms | ratio | usd | count',
    higher_is_better TINYINT(1)  NOT NULL DEFAULT 1,
    is_primary      TINYINT(1)   NOT NULL DEFAULT 0 COMMENT 'the gate metric shown on the scoreboard',
    primary_one     TINYINT      AS (IF(is_primary = 1, 1, NULL)) PERSISTENT,
    ci_low          DOUBLE       NULL,
    ci_high         DOUBLE       NULL,
    n               INT          NULL COMMENT 'sample size behind this number — guards against n=20 variance',
    created_at      TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_run_metric   (run_id, name, slice_dim, slice_val),
    UNIQUE KEY uk_one_primary  (run_id, primary_one),
    INDEX idx_run     (run_id),
    INDEX idx_slice   (slice_dim, slice_val),
    INDEX idx_primary (is_primary, name)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_item : per-item evidence (drill-down + cross-run A/B diff) ──────────
CREATE TABLE IF NOT EXISTS evx_item (
    id           BIGINT       AUTO_INCREMENT PRIMARY KEY,
    run_id       VARCHAR(64)  NOT NULL,
    item_id      VARCHAR(120) NOT NULL COMMENT 'stable across runs → enables A/B diff join on the same gold item',
    score        DOUBLE       NULL,
    correct      TINYINT(1)   NULL,
    payload_json JSON         NULL COMMENT 'type-specific: QA={q,gold,pred,judge}; OCR={pred_text,cer}; NER={pred_spans,gold_spans}',
    created_at   TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_run_item (run_id, item_id),
    INDEX idx_run  (run_id),
    INDEX idx_item (item_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_item_review : human override / HITL review (carries legacy feature) ─
-- Keyed by the NATURAL (run_id, item_id) so backfill needs no surrogate id.
-- Preserves the eval-score-override.tsx workflow: human_scores_json holds the
-- reviewer's per-dimension overrides; the original machine scores stay in
-- evx_item.
CREATE TABLE IF NOT EXISTS evx_item_review (
    id                BIGINT       AUTO_INCREMENT PRIMARY KEY,
    run_id            VARCHAR(64)  NOT NULL,
    item_id           VARCHAR(120) NOT NULL,
    human_scores_json JSON         NULL COMMENT '{accuracy, completeness, relevance, safety} reviewer overrides',
    notes             TEXT         NULL,
    reviewed_by       VARCHAR(100) NULL,
    reviewed_at       TIMESTAMP    NULL,
    created_at        TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_run_item_review (run_id, item_id),
    INDEX idx_reviewed (reviewed_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_artifact : heavy diagnostics by reference (keep blobs out of rows) ──
CREATE TABLE IF NOT EXISTS evx_artifact (
    id          BIGINT       AUTO_INCREMENT PRIMARY KEY,
    run_id      VARCHAR(64)  NOT NULL,
    kind        VARCHAR(40)  NOT NULL COMMENT 'logit_diff_dump | residency_trace | confusion_matrix | latency_histogram',
    storage_uri TEXT         NOT NULL,
    sha256      CHAR(64)     NULL,
    created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_run (run_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── evx_span : satellite for span/region families (NER, extraction, layout) ─
-- Needed because the security-critical NER metric (per-entity regression diff:
-- "span incumbent caught but ANE missed") requires a span-level self-join that
-- JSON payload can't serve. Stores text_hash NEVER raw PII.
CREATE TABLE IF NOT EXISTS evx_span (
    id          BIGINT       AUTO_INCREMENT PRIMARY KEY,
    run_id      VARCHAR(64)  NOT NULL,
    item_id     VARCHAR(120) NOT NULL,
    span_start  INT          NULL COMMENT 'char offset (NER/extraction); NULL for bbox',
    span_end    INT          NULL,
    bbox        JSON         NULL COMMENT '[x,y,w,h] for OCR-layout/IoU families',
    label       VARCHAR(60)  NOT NULL COMMENT 'entity_type / field_name / region_type',
    source      VARCHAR(8)   NOT NULL COMMENT 'gold | pred',
    text_hash   CHAR(64)     NULL COMMENT 'SHA2 of span text — NEVER raw PII',
    confidence  DOUBLE       NULL,
    created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_run_item (run_id, item_id),
    INDEX idx_label    (label),
    INDEX idx_source   (source)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- ─── Convenience view : the single cross-family scoreboard ──────────────────
-- One query, every eval family. Powers the unified UI scoreboard tab.
CREATE OR REPLACE VIEW evx_scoreboard AS
SELECT
    r.family,
    r.id              AS run_id,
    r.experiment_id,
    r.tenant_id,
    t.kind            AS target_kind,
    t.name            AS target_name,
    t.model_id,
    t.runtime,
    r.dataset_id,
    r.n_items,
    m.name            AS primary_metric,
    m.value           AS primary_value,
    m.unit,
    m.higher_is_better,
    m.ci_low,
    m.ci_high,
    r.finished_at
FROM evx_run r
JOIN evx_target t ON t.id = r.target_id
LEFT JOIN evx_metric m ON m.run_id = r.id AND m.is_primary = 1;
