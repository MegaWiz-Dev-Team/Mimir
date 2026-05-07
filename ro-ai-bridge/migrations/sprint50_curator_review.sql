-- Sprint 50 B-50f — Curator review queue
--
-- Adds a review-status track on ocr_documents so Curators can flag and
-- adjudicate low-confidence calls. NULL means "no review needed";
-- "pending" is auto-set when engine confidence drops below the per-tenant
-- threshold (or when a Curator manually flags it).
--
-- Decisions land on the same row — no separate review table — because
-- there's at most one review per OCR call. Cross-references via audit_id
-- keep the audit invariant intact.
--
-- See: Asgard/docs/architecture/ADR-006-Syn-OCR-Stack.md (curator gate)
--      Mimir/docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md (Sprint 50 B-50f)

ALTER TABLE ocr_documents
    ADD COLUMN review_status VARCHAR(20) DEFAULT NULL
        COMMENT 'NULL=no review needed | pending | approved | rejected — Curator decision (B-50f)',
    ADD COLUMN review_note TEXT DEFAULT NULL
        COMMENT 'Curator free-text note on the decision (PHI-safe — short answers expected)',
    ADD COLUMN reviewed_by VARCHAR(100) DEFAULT NULL
        COMMENT 'JWT subject of the Curator who made the decision',
    ADD COLUMN reviewed_at TIMESTAMP NULL DEFAULT NULL,
    ADD KEY idx_review_status (review_status, tenant_id, created_at);
