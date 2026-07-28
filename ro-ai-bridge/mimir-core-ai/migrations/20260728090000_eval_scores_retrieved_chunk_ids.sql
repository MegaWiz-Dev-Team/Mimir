-- Sprint 47 B-47c added the retrieved_chunk_ids write to the eval runner but
-- shipped no migration, so every eval INSERT has failed with 1054 since.
-- Bare chunk_id JSON list, extracted from retrieval_chunks at write time so
-- RAGAS / Recall@k never re-parse the full trace JSON.
ALTER TABLE eval_scores
    ADD COLUMN IF NOT EXISTS retrieved_chunk_ids TEXT NULL
    COMMENT 'JSON array of chunk_ids retrieved for this answer (B-47c fast path)';
