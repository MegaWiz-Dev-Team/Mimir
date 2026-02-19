-- Migration: Agent Evaluation System Tables
-- Author: Antigravity
-- Date: 2026-02-20

-- 1. Evaluation Run (one per "evaluate all" batch)
CREATE TABLE IF NOT EXISTS eval_runs (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',  -- PENDING, RUNNING, COMPLETED, FAILED
    total_combinations INT DEFAULT 0,
    completed_combinations INT DEFAULT 0,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    config JSON COMMENT 'rubric config, dataset version, etc.',
    INDEX idx_status (status),
    INDEX idx_started (started_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 2. Individual eval result: one row per (agent, model, question)
CREATE TABLE IF NOT EXISTS eval_scores (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    agent_name VARCHAR(50) NOT NULL,
    model_id VARCHAR(100) NOT NULL,
    question TEXT NOT NULL,
    expected_answer TEXT NOT NULL,
    actual_answer TEXT,
    -- Rubric Scores (1-5 scale, assigned by LLM-as-Judge)
    accuracy_score TINYINT COMMENT '1-5: factual correctness vs expected',
    completeness_score TINYINT COMMENT '1-5: covers all key points',
    relevance_score TINYINT COMMENT '1-5: stays on topic, no hallucination',
    latency_ms INT COMMENT 'Time to first response in milliseconds',
    -- LLM-as-Judge metadata
    judge_model VARCHAR(100) COMMENT 'Model used for judging, e.g. gemini-2.5-flash',
    judge_reasoning TEXT COMMENT 'Full reasoning from judge LLM',
    -- Human Override (dashboard review)
    human_accuracy_score TINYINT,
    human_completeness_score TINYINT,
    human_relevance_score TINYINT,
    human_notes TEXT,
    reviewed_by VARCHAR(100),
    reviewed_at TIMESTAMP NULL,
    -- Meta
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (model_id) REFERENCES ai_models(model_id),
    INDEX idx_run (run_id),
    INDEX idx_agent_model (agent_name, model_id),
    INDEX idx_reviewed (reviewed_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 3. Aggregated summary per (agent, model) combination per run
CREATE TABLE IF NOT EXISTS eval_summary (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    agent_name VARCHAR(50) NOT NULL,
    model_id VARCHAR(100) NOT NULL,
    total_questions INT DEFAULT 0,
    avg_accuracy FLOAT,
    avg_completeness FLOAT,
    avg_relevance FLOAT,
    avg_latency_ms FLOAT,
    overall_score FLOAT COMMENT 'Weighted composite: (acc*0.4 + comp*0.3 + rel*0.3)',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE,
    UNIQUE KEY uk_run_agent_model (run_id, agent_name, model_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
