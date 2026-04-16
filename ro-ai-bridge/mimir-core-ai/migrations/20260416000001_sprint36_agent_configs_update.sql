-- Sprint 36: Add newly introduced fields to agent_configs table
ALTER TABLE agent_configs
    ADD COLUMN use_pageindex BOOLEAN DEFAULT FALSE,
    ADD COLUMN rag_params JSON DEFAULT NULL,
    ADD COLUMN rerank_config JSON DEFAULT NULL;
