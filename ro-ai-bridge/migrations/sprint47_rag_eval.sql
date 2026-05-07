-- Sprint 47 — Mimir RAG Eval (Rust-native RAGAS-equivalent)
--
-- Adds:
--   1. eval_scores extension — RAGAS metrics + retrieval_chunk_ids per row
--   2. rag_benchmark_items — gold-labelled (question → relevant chunks)
--      curated by clinicians; powers retrieval-side metrics (Recall@k, MRR, …)
--
-- Design notes:
--   - Per-row metrics live alongside HBp dims in eval_scores so a single eval
--     run produces both end-to-end scores and bottleneck-attribution data.
--   - Tenant scope:
--       eval_scores stays as-is (already has tenant_id from Sprint 36).
--       rag_benchmark_items is tenant-scoped (each tenant curates its own
--       gold set; future v0.2 may add tenant_id=NULL shared baselines).
--   - Retrieval metrics (Recall@k etc.) are computed when the tenant has
--     gold rag_benchmark_items for the question_id; otherwise NULL.
--
-- See:
--   Mimir/docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md
--   (Sprint 47 backlog B-47a / B-47b / B-47c / B-47d)

-- ─────────────────────────────────────────────────────────────────────
-- 1. eval_scores extension — RAGAS metrics + retrieved chunk IDs
-- ─────────────────────────────────────────────────────────────────────
-- All cols nullable: legacy rows pre-Sprint-47 won't have these,
-- and even post-Sprint-47 only rows where the eval run opted into RAG
-- metrics will have non-NULL values.
ALTER TABLE eval_scores
    -- LLM-as-judge metrics (0.0-1.0, computed by RAGAS-style prompts)
    ADD COLUMN faithfulness          DECIMAL(4,3)  DEFAULT NULL
        COMMENT 'RAGAS Faithfulness — does the answer cite from retrieved context?',
    ADD COLUMN answer_relevancy      DECIMAL(4,3)  DEFAULT NULL
        COMMENT 'RAGAS Answer Relevancy — does the answer address the question?',
    ADD COLUMN context_precision     DECIMAL(4,3)  DEFAULT NULL
        COMMENT 'RAGAS Context Precision — fraction of retrieved chunks that are relevant',
    ADD COLUMN context_recall        DECIMAL(4,3)  DEFAULT NULL
        COMMENT 'RAGAS Context Recall — fraction of gold relevant info captured by retrieval (needs gold)',

    -- Retrieved chunk IDs (JSON array of chunk identifiers from Qdrant /
    -- knowledge base). Captured per-eval-row so retrieval metrics can be
    -- recomputed if the rag_benchmark_items gold set changes.
    ADD COLUMN retrieved_chunk_ids   LONGTEXT      DEFAULT NULL
        COMMENT 'JSON array of chunk identifiers retrieved during this eval row',

    -- Pure-Rust retrieval metrics (computed when gold rag_benchmark_items
    -- exists for the question_id). NULL when no gold set.
    ADD COLUMN retrieval_recall_at_5   DECIMAL(4,3) DEFAULT NULL,
    ADD COLUMN retrieval_recall_at_16  DECIMAL(4,3) DEFAULT NULL,
    ADD COLUMN retrieval_mrr           DECIMAL(4,3) DEFAULT NULL
        COMMENT 'Mean Reciprocal Rank — 1/rank of first relevant retrieved chunk',
    ADD COLUMN retrieval_ndcg_at_8     DECIMAL(4,3) DEFAULT NULL
        COMMENT 'Normalized Discounted Cumulative Gain @ k=8',

    -- Per-row judge tracking (which model judged the RAGAS metrics — may
    -- differ from HBp judge_model). Audit trail for re-judging.
    ADD COLUMN rag_judge_model         VARCHAR(100) DEFAULT NULL,
    ADD COLUMN rag_judge_reasoning     LONGTEXT     DEFAULT NULL
        COMMENT 'JSON: per-metric reasoning + raw judge response for audit';

-- ─────────────────────────────────────────────────────────────────────
-- 2. rag_benchmark_items — clinician-curated gold (question → relevant chunks)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE rag_benchmark_items (
    id                   VARCHAR(36)  NOT NULL,
    -- Which benchmark this gold belongs to. Joins to eval_benchmark_datasets.id.
    benchmark_id         VARCHAR(50)  NOT NULL,
    -- The question being labelled. Joins to a benchmark's question_id naming.
    question_id          VARCHAR(100) NOT NULL,
    -- Which Qdrant/knowledge collection the gold chunks come from.
    collection_id        VARCHAR(100) NOT NULL,
    -- JSON array of chunk identifiers that SHOULD be in retrieval results.
    relevant_chunk_ids   LONGTEXT     NOT NULL,
    -- JSON array of "must mention" topics — for fuzzier topic-coverage
    -- evaluation in addition to exact chunk match.
    required_topics      LONGTEXT     DEFAULT NULL,
    -- Optional clinician note describing the gold rationale.
    notes                TEXT         DEFAULT NULL,
    -- Curator + tenant scope. Tenant_id = NULL reserved for future shared
    -- baselines but v0 is tenant-scoped (each hospital curates its own).
    tenant_id            VARCHAR(50)  NOT NULL DEFAULT 'asgard_medical',
    curated_by           VARCHAR(100) DEFAULT NULL,
    curated_at           TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uniq_benchmark_question (benchmark_id, question_id, tenant_id),
    KEY idx_collection (collection_id),
    KEY idx_tenant (tenant_id),
    KEY idx_benchmark (benchmark_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
