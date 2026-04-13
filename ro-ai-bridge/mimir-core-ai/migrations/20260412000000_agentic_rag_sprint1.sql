-- T1.1: Dataset Version Schema
ALTER TABLE rag_eval_datasets ADD COLUMN IF NOT EXISTS version INT DEFAULT 1;
ALTER TABLE rag_eval_datasets ADD COLUMN IF NOT EXISTS difficulty VARCHAR(10);
ALTER TABLE rag_eval_datasets ADD COLUMN IF NOT EXISTS question_type VARCHAR(20);

-- T1.4: Store Token in Results (Run Level)
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS total_prompt_tokens INT DEFAULT 0;
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS total_completion_tokens INT DEFAULT 0;
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS total_thinking_tokens INT DEFAULT 0;

-- CREATE MISSING TABLE: rag_eval_queries (Needed for Evaluation Matrix Tasks)
CREATE TABLE IF NOT EXISTS rag_eval_queries (
    id INT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    tenant_id VARCHAR(50) NOT NULL,
    query TEXT NOT NULL,
    expected_titles JSON,
    expected_content TEXT,
    hit BOOLEAN DEFAULT FALSE,
    reciprocal_rank DOUBLE DEFAULT 0,
    ndcg_score DOUBLE DEFAULT 0,
    precision_score DOUBLE DEFAULT 0,
    recall_score DOUBLE DEFAULT 0,
    matched_at_rank INT,
    vector_contributed BOOLEAN DEFAULT FALSE,
    tree_contributed BOOLEAN DEFAULT FALSE,
    graph_contributed BOOLEAN DEFAULT FALSE,
    top_results JSON,
    generated_answer TEXT,
    faithfulness DOUBLE DEFAULT 0,
    answer_relevancy DOUBLE DEFAULT 0,
    context_precision DOUBLE DEFAULT 0,
    judge_reasoning TEXT,
    retrieval_latency_ms INT DEFAULT 0,
    generation_latency_ms INT DEFAULT 0,
    total_latency_ms INT DEFAULT 0,
    prompt_tokens INT DEFAULT 0,
    completion_tokens INT DEFAULT 0,
    thinking_tokens INT DEFAULT 0,
    ttft_ms INT DEFAULT 0,
    difficulty VARCHAR(20),
    question_type VARCHAR(20),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_rag_eval_queries_run_id (run_id),
    INDEX idx_rag_eval_queries_tenant_id (tenant_id)
);
