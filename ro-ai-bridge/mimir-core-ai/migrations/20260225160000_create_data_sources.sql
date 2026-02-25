CREATE TABLE data_sources (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    source_type ENUM('web', 'tabular', 'document', 'mcp') NOT NULL,
    config_json JSON NOT NULL, -- Stores URLs, file paths, or MCP connection strings
    schedule VARCHAR(100), -- e.g., 'Manual', 'Daily at 00:00'
    last_sync_status ENUM('PENDING', 'RUNNING', 'COMPLETED', 'FAILED') DEFAULT 'PENDING',
    last_sync_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_tenant (tenant_id)
);
