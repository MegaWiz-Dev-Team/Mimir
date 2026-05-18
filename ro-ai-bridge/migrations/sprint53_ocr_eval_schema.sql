-- Sprint 53 — OCR Eval Schema (Syn v0.3.0+ region-aware OCR evaluation)
--
-- Stores OCR evaluation results (mAP, parity, CER/WER, GriTS) separately
-- from agent eval (eval_scores / rag_benchmark_items) because OCR data
-- shape is geometric (bboxes, IoU per region) not scalar-per-question.
--
-- Lives in the asgard_platform tenant — a new cross-cutting tenant for
-- engineering quality metrics. Parallel to asgard_medical /
-- asgard_insurance / asgard_wellness which hold clinical/insurance data.
-- No special "tenant create" step needed; Mimir treats tenant_id as a
-- per-row column.
--
-- PII safety:
--   * is_synthetic = TRUE rows MAY store image_name + bbox coords + class
--     labels (no PHI; data is synthetic / publishable).
--   * is_synthetic = FALSE rows MUST set image_hash only and use class
--     labels (no text/coords leak — refer to asgard_medical.ocr_documents
--     for the raw content cross-linked by predicted_hash).
-- The application layer (syn-eval-ingest) enforces this; the schema
-- supports both shapes via NULLable columns.
--
-- See memory files:
--   - asgard_platform_tenant.md     (tenant rationale)
--   - syn_data_internal_only.md     (PHI handling)
--   - syn_v030_phase1_finding.md    (first set of results to land here)

-- ─────────────────────────────────────────────────────────────────────
-- 1. ocr_eval_runs — one row per benchmark execution
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE ocr_eval_runs (
    id                  VARCHAR(36)  NOT NULL,
    tenant_id           VARCHAR(50)  NOT NULL DEFAULT 'asgard_platform'
        COMMENT 'Tenant scope. asgard_platform for cross-cutting eng metrics.',

    -- Discriminator: what kind of OCR eval produced this row.
    -- mAP      → region detection (bboxes + IoU per match)
    -- parity   → Rust↔Python tensor diff (single max-abs-diff scalar / fixture)
    -- cer_wer  → handwriting recognition error rates (per image)
    -- grits    → table structure similarity (Phase 3)
    eval_kind           VARCHAR(16)  NOT NULL,

    -- Provenance — every run must be reproducible from commit + model.
    syn_version         VARCHAR(32)  NOT NULL
        COMMENT 'e.g. v0.3.0-alpha.2',
    commit_sha          VARCHAR(40)  DEFAULT NULL,
    model_name          VARCHAR(128) NOT NULL,
    model_sha256        VARCHAR(64)  DEFAULT NULL,

    -- Dataset identity.
    dataset_name        VARCHAR(64)  NOT NULL
        COMMENT 'Stable identifier e.g. synthetic-handwriting-5, medcerts-30',
    dataset_hash        VARCHAR(64)  DEFAULT NULL
        COMMENT 'SHA-256 over the dataset manifest; detects silent corpus changes.',
    is_synthetic        BOOLEAN      NOT NULL DEFAULT FALSE
        COMMENT 'TRUE = publishable per-item details OK; FALSE = hash-only mode.',

    -- mAP-only param. NULL for other eval_kinds.
    iou_threshold       DECIMAL(4,3) DEFAULT NULL,

    -- Counts (denormalized from items for fast list view).
    n_images            INT          NOT NULL DEFAULT 0,
    n_gt_regions        INT          DEFAULT NULL,
    n_predictions       INT          DEFAULT NULL,

    -- Top-level metrics summary. Schema varies by eval_kind:
    --   mAP:     { class_agnostic: {ap50, tp, fp, fn, precision, recall},
    --             per_class: [{class, ap50, tp, fp, fn, gt_count}, ...] }
    --   parity:  { max_abs_diff, mean_abs_diff, tolerance, all_within_tol }
    --   cer_wer: { mean_cer, mean_wer, exact_match_pct }
    --   grits:   { grits_top, grits_con, grits_loc }
    summary             JSON         NOT NULL
        COMMENT 'Top-level metrics. Shape depends on eval_kind.',

    started_at          TIMESTAMP(3) NOT NULL,
    finished_at         TIMESTAMP(3) NOT NULL,
    duration_ms         INT GENERATED ALWAYS AS
        (TIMESTAMPDIFF(MICROSECOND, started_at, finished_at) DIV 1000) VIRTUAL,

    created_at          TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    KEY idx_runs_tenant_kind         (tenant_id, eval_kind, finished_at DESC),
    KEY idx_runs_started_at          (started_at DESC),
    KEY idx_runs_syn_version         (syn_version),
    KEY idx_runs_dataset             (dataset_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────
-- 2. ocr_eval_items — per-image rollup within a run
-- ─────────────────────────────────────────────────────────────────────
-- One row per (run, image). Holds counts + per-image metrics blob.
-- Region-level detail lives in ocr_region_eval (3rd table) which FKs here.
CREATE TABLE ocr_eval_items (
    id                  VARCHAR(36)  NOT NULL,
    run_id              VARCHAR(36)  NOT NULL,

    -- Synthetic data: image_name visible (e.g. "hw-rx-01.png").
    -- Real data:      image_name NULL, image_hash populated.
    -- The application layer decides which to write based on
    -- `ocr_eval_runs.is_synthetic`.
    image_name          VARCHAR(128) DEFAULT NULL,
    image_hash          VARCHAR(64)  DEFAULT NULL,

    image_width         INT          DEFAULT NULL,
    image_height        INT          DEFAULT NULL,

    n_gt                INT          NOT NULL DEFAULT 0,
    n_pred              INT          NOT NULL DEFAULT 0,
    n_matched           INT          NOT NULL DEFAULT 0,

    -- Per-image metrics, shape depends on eval_kind. Examples:
    --   mAP:     { precision, recall, ap50_image_local }
    --   parity:  { max_abs_diff, n_exceeded_tol }
    --   cer_wer: { cer, wer, exact_match }
    metrics             JSON         DEFAULT NULL,

    latency_ms          INT          DEFAULT NULL,

    created_at          TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    KEY idx_items_run                (run_id),
    KEY idx_items_image_hash         (image_hash),
    CONSTRAINT fk_items_run
        FOREIGN KEY (run_id) REFERENCES ocr_eval_runs(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────
-- 3. ocr_region_eval — per-region GT↔prediction match record
-- ─────────────────────────────────────────────────────────────────────
-- Used by eval_kind = 'mAP' (and any future bbox-geometric metric).
-- For non-bbox evals (parity, cer_wer) this table stays empty.
--
-- Each row = one (gt_bbox, pred_bbox) pairing decision:
--   * Both filled → matched at IoU threshold
--   * Only GT filled → false negative (no prediction matched)
--   * Only pred filled → false positive (no GT in range)
--
-- Confidence comes from the prediction; class_true comes from GT;
-- is_match folds the IoU threshold check; is_class_match is separate so
-- we can compute class-aware AND class-agnostic AP from the same rows.
CREATE TABLE ocr_region_eval (
    id                  BIGINT       NOT NULL AUTO_INCREMENT,
    run_id              VARCHAR(36)  NOT NULL,
    item_id             VARCHAR(36)  NOT NULL,

    -- GT bbox (NULL = false positive; prediction with no GT match).
    bbox_gt_x           INT          DEFAULT NULL,
    bbox_gt_y           INT          DEFAULT NULL,
    bbox_gt_w           INT          DEFAULT NULL,
    bbox_gt_h           INT          DEFAULT NULL,
    class_true          VARCHAR(32)  DEFAULT NULL,

    -- Predicted bbox (NULL = false negative; GT with no prediction in range).
    bbox_pred_x         INT          DEFAULT NULL,
    bbox_pred_y         INT          DEFAULT NULL,
    bbox_pred_w         INT          DEFAULT NULL,
    bbox_pred_h         INT          DEFAULT NULL,
    class_pred          VARCHAR(32)  DEFAULT NULL,
    confidence          DECIMAL(5,4) DEFAULT NULL,

    iou                 DECIMAL(5,4) DEFAULT NULL,
    is_match            BOOLEAN      NOT NULL DEFAULT FALSE
        COMMENT 'TRUE iff iou >= run.iou_threshold (independent of class).',
    is_class_match      BOOLEAN      NOT NULL DEFAULT FALSE
        COMMENT 'TRUE iff is_match AND class_true = class_pred.',

    created_at          TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    KEY idx_region_run               (run_id),
    KEY idx_region_item              (item_id),
    KEY idx_region_class_true        (class_true),
    KEY idx_region_class_pred        (class_pred),
    KEY idx_region_match             (is_match, is_class_match),
    CONSTRAINT fk_region_run
        FOREIGN KEY (run_id) REFERENCES ocr_eval_runs(id) ON DELETE CASCADE,
    CONSTRAINT fk_region_item
        FOREIGN KEY (item_id) REFERENCES ocr_eval_items(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
