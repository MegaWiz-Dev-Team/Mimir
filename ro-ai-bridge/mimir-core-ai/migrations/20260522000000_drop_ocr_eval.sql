-- Drop the Sprint 51 text-level OCR eval schema (ocr_eval_*).
--
-- These four tables (datasets / cases / runs / results) were created by
-- 20260513000000_ocr_eval.sql but never wired to any consumer: no ingest
-- route, no Python script (the referenced scripts/ingest_ocr_eval_to_mimir.py
-- never existed), no frontend. They held 0 rows. CER/WER text-level metrics
-- are tracked in Syn's benchmark reports instead.
--
-- The 20260513000000_ocr_eval.sql forward migration is kept in place (sqlx
-- migrations are append-only + checksummed); this migration nets it out so a
-- fresh DB ends with the tables created-then-dropped. Rollback (recreate)
-- lives in down/20260522000000_drop_ocr_eval.down.sql.
--
-- FK-safe drop order: children before parents (results → cases → runs →
-- datasets). All FKs are internal to this group; no other table references
-- them, and audit_id → ocr_documents.id is a soft VARCHAR column (not a
-- constraint), so ocr_documents is unaffected.

DROP TABLE IF EXISTS ocr_eval_results;
DROP TABLE IF EXISTS ocr_eval_cases;
DROP TABLE IF EXISTS ocr_eval_runs;
DROP TABLE IF EXISTS ocr_eval_datasets;