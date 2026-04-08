-- ============================================================================
-- Add RAG Evaluation Datasets & Runs
-- 
-- Creates rag_eval_runs (benchmark run tracking) and rag_eval_datasets
-- (reusable test suites).
-- ============================================================================

-- 1) Create rag_eval_runs if it doesn't exist yet
CREATE TABLE IF NOT EXISTS rag_eval_runs (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(255),
    status VARCHAR(20) DEFAULT 'pending',
    weight_vector DOUBLE DEFAULT 0.5,
    weight_tree DOUBLE DEFAULT 0.3,
    weight_graph DOUBLE DEFAULT 0.2,
    hit_rate DOUBLE,
    mrr DOUBLE,
    ndcg DOUBLE,
    precision_at_k DOUBLE,
    recall_at_k DOUBLE,
    top_k INT DEFAULT 5,
    avg_latency_ms DOUBLE,
    avg_faithfulness DOUBLE,
    avg_answer_relevancy DOUBLE,
    vector_hit_rate DOUBLE,
    tree_hit_rate DOUBLE,
    graph_hit_rate DOUBLE,
    total_queries INT DEFAULT 0,
    vector_alpha DOUBLE,
    vector_threshold DOUBLE,
    graph_hops INT,
    rerank_enabled TINYINT(1) DEFAULT 0,
    rerank_strategy VARCHAR(50),
    rerank_model VARCHAR(100),
    rerank_final_k INT DEFAULT 5,
    source_filter TEXT,
    collections TEXT,
    embed_model VARCHAR(100),
    judge_model VARCHAR(100),
    judge_provider VARCHAR(50),
    dataset_id VARCHAR(36),
    dataset_name VARCHAR(255),
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    INDEX idx_tenant (tenant_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 2) Create rag_eval_datasets
CREATE TABLE IF NOT EXISTS rag_eval_datasets (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    eval_set JSON NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_auth (tenant_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
