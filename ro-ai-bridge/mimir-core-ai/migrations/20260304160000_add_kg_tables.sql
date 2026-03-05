-- Knowledge Graph tables for entity/relation storage
-- Tracks entities extracted from chunks and their relationships

CREATE TABLE IF NOT EXISTS kg_entities (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    name VARCHAR(512) NOT NULL,
    entity_type VARCHAR(128) NOT NULL DEFAULT 'Concept',
    properties JSON DEFAULT NULL,
    source_id BIGINT DEFAULT NULL,
    chunk_id BIGINT DEFAULT NULL,
    neo4j_node_id VARCHAR(128) DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_kg_entities_tenant (tenant_id),
    INDEX idx_kg_entities_type (tenant_id, entity_type),
    INDEX idx_kg_entities_name (tenant_id, name(255)),
    INDEX idx_kg_entities_source (tenant_id, source_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS kg_relations (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    from_entity_id BIGINT NOT NULL,
    to_entity_id BIGINT NOT NULL,
    relation_type VARCHAR(128) NOT NULL,
    properties JSON DEFAULT NULL,
    source_id BIGINT DEFAULT NULL,
    neo4j_rel_id VARCHAR(128) DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (from_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (to_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    INDEX idx_kg_relations_tenant (tenant_id),
    INDEX idx_kg_relations_type (tenant_id, relation_type),
    INDEX idx_kg_relations_from (from_entity_id),
    INDEX idx_kg_relations_to (to_entity_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS kg_extraction_runs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    source_id BIGINT DEFAULT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    entities_found INT DEFAULT 0,
    relations_found INT DEFAULT 0,
    chunks_processed INT DEFAULT 0,
    error_message TEXT DEFAULT NULL,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL DEFAULT NULL,
    INDEX idx_kg_runs_tenant (tenant_id),
    INDEX idx_kg_runs_status (tenant_id, status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
