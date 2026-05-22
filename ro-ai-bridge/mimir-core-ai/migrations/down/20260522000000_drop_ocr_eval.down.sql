-- Rollback for 20260522000000_drop_ocr_eval.sql — recreates the Sprint 51
-- text-level OCR eval schema (ocr_eval_*) exactly as 20260513000000_ocr_eval.sql
-- defined it. Parent tables first so FKs resolve.

CREATE TABLE IF NOT EXISTS ocr_eval_datasets (
    id              VARCHAR(36)   NOT NULL PRIMARY KEY,
    tenant_id       VARCHAR(50)   NOT NULL,
    name            VARCHAR(120)  NOT NULL COMMENT 'Stable handle, e.g. medical_certs_v1, synthetic_handwriting_v1',
    version         INT           NOT NULL DEFAULT 1,
    source          VARCHAR(80)   NOT NULL COMMENT 'real_partner | synthetic | scraped | mixed',
    description     TEXT          NULL,
    image_count     INT           NOT NULL DEFAULT 0,
    gt_source_path  TEXT          NULL  COMMENT 'Source path for ground truth (CSV/JSON) — for reproducibility',
    is_active       TINYINT(1)    NOT NULL DEFAULT 1,
    created_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_tenant_name_version  (tenant_id, name, version),
    INDEX idx_tenant_active            (tenant_id, is_active)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ocr_eval_cases (
    id              VARCHAR(36)   NOT NULL PRIMARY KEY,
    dataset_id      VARCHAR(36)   NOT NULL,
    case_id         VARCHAR(80)   NOT NULL COMMENT 'External id e.g. T001, hw-rx-01',
    image_path      TEXT          NOT NULL COMMENT 'Repo-relative path, e.g. Syn/data/images/T001.jpg',
    image_sha256    CHAR(64)      NULL     COMMENT 'Helps detect dataset drift if files mutate',
    image_format    VARCHAR(10)   NULL     COMMENT 'jpg | png | gif | pdf',
    ground_truth    LONGTEXT      NOT NULL,
    gt_chars        INT           NOT NULL DEFAULT 0,
    pii_types       JSON          NULL     COMMENT 'Array of PII categories present (PATIENT_NAME, HN, ...)',
    doc_type        VARCHAR(40)   NULL     COMMENT 'medical_cert | handwriting | printed_thai | mixed',
    notes           TEXT          NULL,
    created_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_dataset_case      (dataset_id, case_id),
    INDEX idx_dataset               (dataset_id),
    INDEX idx_image_sha256          (image_sha256),
    CONSTRAINT fk_eval_case_dataset
        FOREIGN KEY (dataset_id) REFERENCES ocr_eval_datasets(id)
        ON DELETE CASCADE
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ocr_eval_runs (
    id              VARCHAR(36)   NOT NULL PRIMARY KEY,
    dataset_id      VARCHAR(36)   NOT NULL,
    tenant_id       VARCHAR(50)   NOT NULL,
    name            VARCHAR(120)  NULL COMMENT 'Human-readable run label, e.g. "field-prompt run B"',
    prompt_label    VARCHAR(80)   NULL COMMENT 'Tag like generic-all-text | field-targeted',
    system_prompt   TEXT          NULL,
    user_prompt     TEXT          NULL,
    engines         JSON          NOT NULL COMMENT 'Array of engine ids tested, e.g. ["mlx-q4","flash"]',
    metadata        JSON          NULL COMMENT 'temperature, max_tokens, gateway URL, etc.',
    started_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at     TIMESTAMP     NULL,
    notes           TEXT          NULL,
    INDEX idx_dataset_started       (dataset_id, started_at DESC),
    INDEX idx_tenant_started        (tenant_id, started_at DESC),
    CONSTRAINT fk_eval_run_dataset
        FOREIGN KEY (dataset_id) REFERENCES ocr_eval_datasets(id)
        ON DELETE CASCADE
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ocr_eval_results (
    id                  VARCHAR(36)   NOT NULL PRIMARY KEY,
    run_id              VARCHAR(36)   NOT NULL,
    case_id             VARCHAR(36)   NOT NULL  COMMENT 'FK to ocr_eval_cases.id',
    engine              VARCHAR(80)   NOT NULL  COMMENT 'mlx-q4 | mlx-q8 | flash | pro | paddleocr | typhoon-ollama ...',
    engine_version      VARCHAR(60)   NULL      COMMENT 'Model commit SHA / quant tag for reproducibility',
    status              VARCHAR(20)   NOT NULL  COMMENT 'ok | error | timeout | truncated',
    cer                 DECIMAL(8,4)  NULL      COMMENT 'May exceed 1.0 when extraction is multiple times reference length',
    wer                 DECIMAL(8,4)  NULL,
    wall_ms             INT           NULL      COMMENT 'End-to-end wall clock incl network + auth',
    prompt_tokens       INT           NULL,
    completion_tokens   INT           NULL,
    extracted_text      LONGTEXT      NULL,
    extracted_chars     INT           NULL,
    error               TEXT          NULL,
    audit_id            VARCHAR(36)   NULL      COMMENT 'Optional FK to ocr_documents.id if this result came from a real syn-api call',
    created_at          TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_run                   (run_id),
    INDEX idx_run_engine            (run_id, engine),
    INDEX idx_case                  (case_id),
    INDEX idx_run_case_engine       (run_id, case_id, engine),
    INDEX idx_engine                (engine),
    INDEX idx_audit                 (audit_id),
    CONSTRAINT fk_eval_result_run
        FOREIGN KEY (run_id)  REFERENCES ocr_eval_runs(id)
        ON DELETE CASCADE,
    CONSTRAINT fk_eval_result_case
        FOREIGN KEY (case_id) REFERENCES ocr_eval_cases(id)
        ON DELETE CASCADE
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;