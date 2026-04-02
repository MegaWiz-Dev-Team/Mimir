-- Sprint 32: RAG Ensemble Playground — Benchmark Database Schema
-- ISO 29110 — Task 2.3: Batch Benchmark Tables

-- Evaluation test sets (ground truth for benchmarking)
CREATE TABLE IF NOT EXISTS eval_sets (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    items JSON NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_eval_sets_tenant (tenant_id)
);

-- Benchmark run results (historical comparison)
CREATE TABLE IF NOT EXISTS search_benchmarks (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(100) NOT NULL,
    eval_set_id VARCHAR(36),
    label VARCHAR(255),
    hit_rate DOUBLE NOT NULL,
    mrr DOUBLE NOT NULL,
    total_queries INT NOT NULL,
    avg_latency_ms DOUBLE,
    weights_json JSON,
    per_query_json JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_benchmarks_tenant (tenant_id),
    INDEX idx_benchmarks_label (label)
);
