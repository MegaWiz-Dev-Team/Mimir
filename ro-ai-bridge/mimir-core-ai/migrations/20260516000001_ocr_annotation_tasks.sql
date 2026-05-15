-- Sprint 51 — OCR Annotation Tasks
-- Multi-user annotation workflow for creating ground truth data
-- Tracks annotator identity, status, and confidence per image

CREATE TABLE IF NOT EXISTS ocr_annotation_tasks (
    id              VARCHAR(36)   NOT NULL PRIMARY KEY COMMENT 'unique annotation task id',
    case_id         VARCHAR(36)   NOT NULL COMMENT 'FK to ocr_eval_cases.id',
    dataset_id      VARCHAR(36)   NOT NULL COMMENT 'FK to ocr_eval_datasets.id (denormalized)',
    tenant_id       VARCHAR(50)   NOT NULL COMMENT 'tenant scope',
    status          ENUM('pending','in_progress','completed','skipped') NOT NULL DEFAULT 'pending',
    ground_truth    LONGTEXT      NULL COMMENT 'annotated text — copies to ocr_eval_cases when completed',
    annotator_id    VARCHAR(100)  NULL COMMENT 'user_id from JWT sub claim',
    confidence      ENUM('high','medium','low') NULL COMMENT 'annotator confidence in the transcription',
    issues          JSON          NULL COMMENT 'array of detected issues: ["handwritten","blurry","partial","damaged"]',
    notes           TEXT          NULL COMMENT 'free-form notes from annotator',
    started_at      TIMESTAMP     NULL COMMENT 'when annotator first opened the task',
    completed_at    TIMESTAMP     NULL COMMENT 'when status changed to completed',
    created_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

    INDEX idx_dataset_status  (dataset_id, status),
    INDEX idx_tenant_annotator (tenant_id, annotator_id),
    INDEX idx_case_id (case_id),
    INDEX idx_status_created (status, created_at DESC),

    CONSTRAINT fk_anno_case
        FOREIGN KEY (case_id) REFERENCES ocr_eval_cases(id) ON DELETE CASCADE,
    CONSTRAINT fk_anno_dataset
        FOREIGN KEY (dataset_id) REFERENCES ocr_eval_datasets(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE utf8mb4_unicode_ci;
