CREATE TABLE IF NOT EXISTS pipeline_run_steps (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    step_number TINYINT UNSIGNED NOT NULL,
    step_name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'running',
    item_count BIGINT DEFAULT 0,
    latency_ms BIGINT DEFAULT 0,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES pipeline_runs(id) ON DELETE CASCADE,
    UNIQUE KEY idx_run_step_unique (run_id, step_number)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
