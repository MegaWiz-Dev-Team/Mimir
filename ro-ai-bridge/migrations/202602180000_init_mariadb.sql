-- MariaDB Migration for Monitoring System

CREATE TABLE IF NOT EXISTS pipeline_runs (
    id VARCHAR(36) PRIMARY KEY, -- UUID
    status VARCHAR(20) NOT NULL, -- RUNNING, COMPLETED, FAILED
    provider VARCHAR(50) NOT NULL, -- ollama, gemini
    model VARCHAR(50) NOT NULL,
    test_run BOOLEAN DEFAULT FALSE,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    INDEX idx_status (status),
    INDEX idx_started_at (started_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS pipeline_steps (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    chunk_index INT DEFAULT 0,
    status VARCHAR(20) NOT NULL, -- PENDING, IN_PROGRESS, COMPLETED, FAILED
    step_type VARCHAR(20) NOT NULL, -- EXTRACT, GENERATE, VERIFY
    error_message TEXT,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    FOREIGN KEY (run_id) REFERENCES pipeline_runs(id) ON DELETE CASCADE,
    INDEX idx_run_id (run_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS qa_results (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    step_id BIGINT NOT NULL,
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    context TEXT, -- The chunk used
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (step_id) REFERENCES pipeline_steps(id) ON DELETE CASCADE,
    INDEX idx_step_id (step_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS evaluation_reports (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    step_id BIGINT NOT NULL,
    coverage_score FLOAT NOT NULL,
    atomic_facts JSON, -- MariaDB supports JSON column
    missing_facts JSON, -- JSON column for missing facts
    reasoning TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (step_id) REFERENCES pipeline_steps(id) ON DELETE CASCADE,
    INDEX idx_step_id (step_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
