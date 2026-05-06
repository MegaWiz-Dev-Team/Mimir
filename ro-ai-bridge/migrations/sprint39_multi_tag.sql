-- Sprint 39 — Multi-tag support for training corpus items
--
-- Adds `tags` column to training_corpus_items as a JSON array of strings.
-- Allows cross-cutting / secondary classification beyond `specialty` (which
-- remains the single primary classifier used for stratified LoRA sampling).
--
-- Use cases:
--   - "AF + CKD anticoagulation" → specialty=cardiology, tags=[pharmacy, nephrology, geriatric]
--   - "Newborn bilious vomiting" → specialty=pediatrics, tags=[surgery, emergency]
--   - "Subclinical hypothyroidism in pregnancy" → specialty=endocrinology, tags=[obgyn, pregnancy]
--
-- Confirmed 2026-05-06 (after Sprint 43 follow-up validated baseline + before
-- Sprint 39 Phase 1 corpus build). See ADR-001 for build-vs-buy reasoning.

ALTER TABLE training_corpus_items
    ADD COLUMN tags LONGTEXT DEFAULT NULL
    AFTER specialty;

-- For tag-based filtering (`?tag=pharmacy`) we need a functional index.
-- MariaDB 10.6+ supports JSON_VALUE-based indexes; fall back to plain text-search
-- if older. The functional index helps GET /queue?tag=X and analytics queries.
CREATE INDEX idx_items_tags_search
    ON training_corpus_items ((CAST(tags AS CHAR(255))));

-- Tag schema (JSON): array of strings, lowercase, kebab-case
-- Example: ["pharmacy", "geriatric", "anticoagulation"]
-- Empty/no tags: NULL (not "[]")
