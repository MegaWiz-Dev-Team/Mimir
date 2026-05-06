-- Sprint 39 — Mimir Curator + LoRA MLOps Tracking (B-30a, B-32a)
--
-- Adds tables for:
--   1. training_corpus_datasets / training_corpus_items — Mimir Curator backing
--      store (annotation workflow, see ADR-001).
--   2. lora_training_runs — MLOps tracking for LoRA fine-tune experiments
--      (hyperparams, loss curves, adapter artifacts; see ADR-002).
--   3. ai_models lineage extension — parent_model_id + lineage_metadata + tenant_id
--      so adapter genealogy is queryable.
--
-- Design decisions confirmed 2026-05-06:
--   - Multi-tenant: tenant_id NULLABLE (NULL = shared, non-NULL = tenant-scoped)
--   - Promotion gate: "no worse than incumbent" (≤1 unsafe rule, not 0 absolute)
--   - Adapter storage: RustFS in-cluster S3 (path stored as URI string)
--
-- See:
--   - Asgard/docs/architecture/ADR-001-Training-Data-Curator-Build-vs-Buy.md
--   - Asgard/docs/architecture/ADR-002-MLOps-Tracking-Build-vs-Buy.md
--   - Mimir/docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md (Sprint 39)

-- ─────────────────────────────────────────────────────────────────────
-- Curator: corpus datasets + items
-- ─────────────────────────────────────────────────────────────────────

CREATE TABLE training_corpus_datasets (
    id              VARCHAR(36)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT         DEFAULT NULL,
    -- NULL = shared corpus, non-NULL = tenant-scoped (visibility filter at API layer).
    tenant_id       VARCHAR(50)  DEFAULT NULL,
    -- Source label: 'gemini-2.5-pro', 'mlx-community/gemma-4-26b-...', 'manual', etc.
    -- Used for provenance + reproducibility (which generator produced this set).
    source          VARCHAR(100) DEFAULT NULL,
    -- Free-form: hyperparams of the synthesis run, prompt template id, etc.
    metadata        LONGTEXT     DEFAULT NULL,
    status          VARCHAR(20)  NOT NULL DEFAULT 'OPEN',
    -- Rough sizing tracking (filled by triggers / eventually_consistent updates).
    total_items     INT          NOT NULL DEFAULT 0,
    approved_items  INT          NOT NULL DEFAULT 0,
    rejected_items  INT          NOT NULL DEFAULT 0,
    created_at      TIMESTAMP    NULL DEFAULT current_timestamp(),
    updated_at      TIMESTAMP    NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),
    created_by      VARCHAR(100) DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_corpus_tenant (tenant_id),
    KEY idx_corpus_status (status),
    KEY idx_corpus_created (created_at)
);

CREATE TABLE training_corpus_items (
    id                    BIGINT       NOT NULL AUTO_INCREMENT,
    dataset_id            VARCHAR(36)  NOT NULL,
    -- Original generated content (immutable after import).
    question              TEXT         NOT NULL,
    ai_answer             TEXT         NOT NULL,
    expected_answer       TEXT         DEFAULT NULL,
    citations             LONGTEXT     DEFAULT NULL,
    -- Reviewer scoring (1-5 Likert + binary safety).
    accuracy_score        TINYINT      DEFAULT NULL,
    completeness_score    TINYINT      DEFAULT NULL,
    relevance_score       TINYINT      DEFAULT NULL,
    -- Safety: 1 = safe, 0 = unsafe; NULL = unscored.
    safety_score          TINYINT      DEFAULT NULL,
    -- Reviewer's rewrite of the AI answer (the "gold" target for LoRA training).
    -- If NULL, ai_answer is used. If non-NULL, this replaces ai_answer in JSONL export.
    improved_answer       TEXT         DEFAULT NULL,
    -- Specialty tag for stratified review + per-specialty fine-tune slicing.
    specialty             VARCHAR(50)  DEFAULT NULL,
    -- Workflow status.
    status                VARCHAR(20)  NOT NULL DEFAULT 'PENDING',
    reviewer_id           VARCHAR(100) DEFAULT NULL,
    reviewer_notes        TEXT         DEFAULT NULL,
    reviewed_at           TIMESTAMP    NULL DEFAULT NULL,
    -- NULL = item belongs to shared dataset; non-NULL = tenant-scoped.
    -- Inherited from parent dataset on import; can override per item.
    tenant_id             VARCHAR(50)  DEFAULT NULL,
    created_at            TIMESTAMP    NULL DEFAULT current_timestamp(),
    PRIMARY KEY (id),
    KEY idx_items_dataset (dataset_id),
    KEY idx_items_status (status),
    KEY idx_items_dataset_status (dataset_id, status),
    KEY idx_items_reviewer (reviewer_id),
    KEY idx_items_specialty (specialty),
    KEY idx_items_tenant (tenant_id),
    CONSTRAINT fk_corpus_item_dataset
        FOREIGN KEY (dataset_id) REFERENCES training_corpus_datasets(id) ON DELETE CASCADE,
    CONSTRAINT chk_status
        CHECK (status IN ('PENDING','APPROVED','REJECTED','FLAGGED'))
);

-- ─────────────────────────────────────────────────────────────────────
-- MLOps: LoRA training run tracking
-- ─────────────────────────────────────────────────────────────────────

CREATE TABLE lora_training_runs (
    id                    VARCHAR(36)  NOT NULL,
    name                  VARCHAR(255) DEFAULT NULL,
    -- Snapshot of corpus dataset used for training (export hash or dataset_id+version).
    -- Pointer to training_corpus_datasets.id (for active datasets) or external ref.
    dataset_id            VARCHAR(36)  DEFAULT NULL,
    -- Stable hash of approved-items JSONL export, for reproducibility even if
    -- the dataset later receives more items.
    dataset_snapshot_hash VARCHAR(64)  DEFAULT NULL,
    -- The base model we're fine-tuning (FK by string match to ai_models.model_id).
    base_model_id         VARCHAR(100) NOT NULL,
    -- Hyperparameters as submitted (rank, alpha, dropout, LR, iterations, target_modules, ...).
    hyperparams           LONGTEXT     DEFAULT NULL,
    -- Time series of training metrics; structure: [{step, loss, val_loss, lr, ...}, ...].
    loss_curve            LONGTEXT     DEFAULT NULL,
    -- Pointer to adapter artifact in RustFS (s3://rustfs/lora-adapters/<run_id>/...).
    adapter_path          VARCHAR(500) DEFAULT NULL,
    -- After mlx_lm.fuse, the merged model gets registered as a new ai_models row.
    -- Track the resulting model_id for promotion lineage.
    merged_model_id       VARCHAR(100) DEFAULT NULL,
    status                VARCHAR(20)  NOT NULL DEFAULT 'PENDING',
    -- Reason field for failures, manual cancellations, etc.
    status_message        TEXT         DEFAULT NULL,
    started_at            TIMESTAMP    NULL DEFAULT current_timestamp(),
    finished_at           TIMESTAMP    NULL DEFAULT NULL,
    -- NULL = shared adapter (e.g. eir-base-lora); non-NULL = tenant-specific.
    tenant_id             VARCHAR(50)  DEFAULT NULL,
    created_by            VARCHAR(100) DEFAULT NULL,
    -- Free-form notes for hypothesis, prior-art reference, etc.
    notes                 TEXT         DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_lora_status (status),
    KEY idx_lora_base_model (base_model_id),
    KEY idx_lora_tenant (tenant_id),
    KEY idx_lora_started (started_at),
    KEY idx_lora_dataset (dataset_id),
    CONSTRAINT chk_lora_status
        CHECK (status IN ('PENDING','RUNNING','COMPLETED','FAILED','CANCELLED'))
);

-- ─────────────────────────────────────────────────────────────────────
-- ai_models lineage extension
-- ─────────────────────────────────────────────────────────────────────

-- The base ai_models table already has model_id (PK) + provider + is_active +
-- capabilities + metadata. Add lineage cols:

ALTER TABLE ai_models
    ADD COLUMN parent_model_id     VARCHAR(100) DEFAULT NULL AFTER model_type,
    ADD COLUMN lineage_metadata    LONGTEXT     DEFAULT NULL AFTER metadata,
    -- NULL = global model (Heimdall serves to all tenants); non-NULL = tenant-scoped.
    ADD COLUMN tenant_id           VARCHAR(50)  DEFAULT NULL AFTER lineage_metadata;

CREATE INDEX idx_ai_models_parent ON ai_models (parent_model_id);
CREATE INDEX idx_ai_models_tenant ON ai_models (tenant_id);

-- Lineage_metadata schema (JSON):
--   {
--     "training_run_id": "uuid",          // FK → lora_training_runs.id
--     "training_dataset_id": "uuid",       // FK → training_corpus_datasets.id
--     "training_dataset_snapshot_hash": "sha256",
--     "promoted_from_run_id": "uuid",      // FK → eval_runs.id (the A/B that promoted)
--     "promotion_lift_pp": 5.2,            // HBp% delta vs incumbent at promotion
--     "promotion_at": "2026-XX-XX"
--   }
