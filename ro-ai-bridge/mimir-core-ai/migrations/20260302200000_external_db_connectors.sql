-- Sprint 14: External DB Connectors (Issue #152)
-- Stores connection configs for external MySQL/PostgreSQL/SQLite databases

CREATE TABLE IF NOT EXISTS external_db_connections (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    db_type ENUM('mysql','postgres','sqlite') NOT NULL,
    connection_string TEXT NOT NULL,
    last_tested_at TIMESTAMP NULL,
    last_test_status VARCHAR(20) DEFAULT 'untested',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    UNIQUE KEY unique_conn_name (tenant_id, name)
);
