-- Sprint 57 / ADR-002 Stage 2 — durable region-level audit for OCR replay.
--
-- Stage 1 (Syn + Mimir) surfaced `regions[]` per OCR call so the live
-- response carries per-region bbox + text + confidence. This migration
-- writes that same payload to the audit row so a clinician dispute, a
-- compliance request, or a Sprint-3 prompt regression can replay exactly
-- what the OCR engine saw — bbox-by-bbox — without having to re-run the
-- engine against the original image.
--
-- Why a JSON column (not a normalised regions table):
--   1. The audit row is the unit of replay; one query returns the whole
--      context, no join cascade.
--   2. The structure of OcrRegion is engine-internal; flattening it into
--      relational rows would couple this table to every engine update.
--   3. Read volume is "look up one case" frequency (HITL replay,
--      regulatory request). MariaDB JSON_EXTRACT is fast enough for that.
--
-- Storage cost: PaddleOCR routinely returns 50–150 regions per page; each
-- row ~300 bytes JSON ⇒ ~15–45 KB per audit row. Existing extracted_text
-- LONGTEXT already holds similar magnitudes; we're keeping the per-call
-- audit "small enough to materialise in one row" invariant.

ALTER TABLE ocr_documents
    ADD COLUMN regions_json LONGTEXT DEFAULT NULL
        COMMENT 'ADR-002 Stage 2: serialized Vec<OcrRegion> from Syn (region_id, page, bbox, text, confidence, semantic_tag). NULL for engines without geometry (Apple Vision, Gemini text-only) and for pre-Stage-1 historical rows.';

-- No index — we look up by audit `id`, not by anything inside regions_json.
-- If/when the dashboard adds "find audit rows containing this region_id"
-- we can add a generated column + index, but that's not on the roadmap.
