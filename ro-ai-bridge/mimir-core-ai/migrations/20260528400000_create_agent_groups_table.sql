-- ============================================================================
-- Feature: Agent Groups Management System
--
-- Date: 2026-05-28
-- Purpose: Hierarchical grouping of agents by category
--          - Editable group names (no hard-coded strings)
--          - Multi-tenant support (each tenant can customize)
--          - Extensible (add new groups anytime)
--
-- Tables:
--   agent_groups: Group definitions (boundary, specialty, router, etc)
--   agent_configs: Add agent_group_id foreign key
-- ============================================================================

-- Create agent_groups table
CREATE TABLE IF NOT EXISTS agent_groups (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  tenant_id VARCHAR(50) NOT NULL,
  name VARCHAR(100) NOT NULL COMMENT 'Group name (e.g., "Boundary Agents", "Specialty Agents")',
  display_name VARCHAR(200) COMMENT 'User-friendly display name (e.g., "🏛️ Boundary & Policy")',
  description TEXT COMMENT 'Group description and purpose',
  sort_order INT DEFAULT 0 COMMENT 'Display order in UI',
  icon_emoji VARCHAR(10) COMMENT 'Icon emoji for UI display',
  color_hex VARCHAR(7) COMMENT 'Hex color for UI theme (e.g., #7C3AED)',
  is_active TINYINT(1) DEFAULT 1 COMMENT 'Soft-delete flag',
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  UNIQUE KEY uk_tenant_group_name (tenant_id, name),
  KEY idx_tenant_active (tenant_id, is_active),
  KEY idx_sort_order (tenant_id, sort_order)
) COMMENT='Agent grouping system - allows flexible categorization per tenant';

-- Add agent_group_id to agent_configs (idempotent: column + FK guarded so a
-- re-run after a partial apply doesn't error 121 on the duplicate FK).
ALTER TABLE agent_configs
ADD COLUMN IF NOT EXISTS agent_group_id BIGINT DEFAULT NULL AFTER agent_version;
ALTER TABLE agent_configs DROP FOREIGN KEY IF EXISTS fk_agent_group;
ALTER TABLE agent_configs
ADD CONSTRAINT fk_agent_group FOREIGN KEY (agent_group_id)
REFERENCES agent_groups(id) ON DELETE SET NULL;

-- Create index for agent group queries
CREATE INDEX IF NOT EXISTS idx_agent_group_id
ON agent_configs(tenant_id, agent_group_id, is_published);
