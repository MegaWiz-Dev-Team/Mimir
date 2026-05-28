-- ============================================================================
-- Seed Agent Groups for All Tenants
--
-- Date: 2026-05-28
-- Purpose: Create standard agent group categories for all tenants
--
-- Groups:
--   1. Boundary Agents (5) — Trust & policy enforcement (asgard_medical/insurance only)
--   2. Specialty Agents (14) — Clinical expertise (asgard_medical only)
--   3. Router Agent (1) — Orchestration (all tenants)
--   4. Platform Agents — System/admin agents (asgard_platform)
-- ============================================================================

-- Get distinct tenants
SET @tenants = (SELECT GROUP_CONCAT(DISTINCT tenant_id SEPARATOR ',') FROM agent_configs);

-- Macro: Create groups for each tenant
-- (Note: MariaDB doesn't support true macros, so we use procedural approach)

-- ═══════════════════════════════════════════════════════════════════════════
-- STANDARD GROUPS (Applied to all tenants)
-- ═══════════════════════════════════════════════════════════════════════════

-- INSERT IGNORE for idempotency

-- Group 1: Boundary Agents (Enforce trust & policy)
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active)
VALUES
('asgard_medical', 'boundary-agents', '🏛️ Boundary Agents', 'Trust & policy enforcement (clinical, pharmacy, pediatrics, psychiatry, emergency)', 1, '🏛️', '#7C3AED', 1),
('asgard_insurance', 'boundary-agents', '🏛️ Boundary Agents', 'Trust & policy enforcement (underwriter consensus agents)', 1, '🏛️', '#7C3AED', 1),
('asgard_platform', 'boundary-agents', '🏛️ Boundary Agents', 'Platform-level boundary agents', 1, '🏛️', '#7C3AED', 1),
('asgard_wellness', 'boundary-agents', '🏛️ Boundary Agents', 'Trust & policy enforcement', 1, '🏛️', '#7C3AED', 1);

-- Group 2: Specialty Agents (Clinical expertise / skills)
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active)
VALUES
('asgard_medical', 'specialty-agents', '🩺 Specialty Agents', 'Clinical expertise modules (internal-medicine, surgery, pediatrics, etc) — future conversion to skills', 2, '🩺', '#10B981', 1),
('asgard_insurance', 'specialty-agents', '🩺 Specialty Agents', 'Underwriting specialist agents (medical-analyzer, fraud-detector, etc)', 2, '🩺', '#10B981', 1),
('asgard_wellness', 'specialty-agents', '🩺 Specialty Agents', 'Wellness domain specialists', 2, '🩺', '#10B981', 1);

-- Group 3: Router & Orchestration Agents
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active)
VALUES
('asgard_medical', 'router-agents', '🎯 Router & Orchestration', 'Request routing, specialty classification (deprecated per ADR-010)', 3, '🎯', '#F59E0B', 1),
('asgard_insurance', 'router-agents', '🎯 Router & Orchestration', 'Request routing and orchestration', 3, '🎯', '#F59E0B', 1),
('asgard_platform', 'router-agents', '🎯 Router & Orchestration', 'Platform-level routing agents', 3, '🎯', '#F59E0B', 1),
('asgard_wellness', 'router-agents', '🎯 Router & Orchestration', 'Domain routing and orchestration', 3, '🎯', '#F59E0B', 1);

-- Group 4: Platform/System Agents (asgard_platform only)
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active)
VALUES
('asgard_platform', 'platform-agents', '⚙️ Platform Agents', 'System-level agents for benchmarking, evaluation, monitoring', 4, '⚙️', '#6366F1', 1);

-- Group 5: Test/Development Agents
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active)
VALUES
('asgard_medical', 'test-agents', '🧪 Test Agents', 'QA, development, and temporary test agents', 99, '🧪', '#EF4444', 1),
('asgard_insurance', 'test-agents', '🧪 Test Agents', 'QA and development agents', 99, '🧪', '#EF4444', 1),
('asgard_platform', 'test-agents', '🧪 Test Agents', 'QA and development agents', 99, '🧪', '#EF4444', 1),
('asgard_wellness', 'test-agents', '🧪 Test Agents', 'QA and development agents', 99, '🧪', '#EF4444', 1);

-- ═══════════════════════════════════════════════════════════════════════════
-- ASSIGN AGENTS TO GROUPS (asgard_medical)
-- ═══════════════════════════════════════════════════════════════════════════

-- Boundary Agents (5)
UPDATE agent_configs SET agent_group_id = (
  SELECT id FROM agent_groups WHERE tenant_id='asgard_medical' AND name='boundary-agents'
) WHERE tenant_id='asgard_medical' AND name IN (
  'eir-clinical', 'eir-pharmacy', 'eir-pediatrics', 'eir-psychiatry', 'eir-emergency'
);

-- Specialty Agents (14)
UPDATE agent_configs SET agent_group_id = (
  SELECT id FROM agent_groups WHERE tenant_id='asgard_medical' AND name='specialty-agents'
) WHERE tenant_id='asgard_medical' AND name IN (
  'eir-internal-medicine', 'eir-surgery', 'eir-ophthalmology', 'eir-orthopedics',
  'eir-ob-gyn', 'eir-radiology', 'eir-medtech', 'eir-nursing',
  'eir-pt', 'eir-dietitian', 'eir-social-work', 'eir-anesthesia',
  'eir-ent', 'eir-urology'
);

-- Router Agent (1)
UPDATE agent_configs SET agent_group_id = (
  SELECT id FROM agent_groups WHERE tenant_id='asgard_medical' AND name='router-agents'
) WHERE tenant_id='asgard_medical' AND name='eir-router';

-- Test Agent (1)
UPDATE agent_configs SET agent_group_id = (
  SELECT id FROM agent_groups WHERE tenant_id='asgard_medical' AND name='test-agents'
) WHERE tenant_id='asgard_medical' AND name='eir-test';

-- ═══════════════════════════════════════════════════════════════════════════
-- ASSIGN AGENTS TO GROUPS (asgard_platform)
-- ═══════════════════════════════════════════════════════════════════════════

UPDATE agent_configs SET agent_group_id = (
  SELECT id FROM agent_groups WHERE tenant_id='asgard_platform' AND name='platform-agents'
) WHERE tenant_id='asgard_platform';

-- ═══════════════════════════════════════════════════════════════════════════
-- VERIFICATION
-- ═══════════════════════════════════════════════════════════════════════════

SELECT 'Agent Groups Seeded Successfully' as status;

-- Summary by tenant
SELECT
  t.tenant_id,
  g.display_name as group_name,
  COUNT(a.id) as agent_count
FROM agent_groups g
LEFT JOIN agent_configs a ON g.id=a.agent_group_id AND g.tenant_id=a.tenant_id
CROSS JOIN (SELECT DISTINCT tenant_id FROM agent_groups) t
WHERE g.tenant_id=t.tenant_id
GROUP BY g.tenant_id, g.display_name
ORDER BY g.tenant_id, g.sort_order;

-- Agents with groups (asgard_medical detail)
SELECT
  g.display_name as 'Group',
  a.name as 'Agent Slug',
  a.display_name as 'Display Name',
  a.model_id as 'Model',
  a.agent_version as 'Version'
FROM agent_configs a
LEFT JOIN agent_groups g ON a.agent_group_id=g.id
WHERE a.tenant_id='asgard_medical'
ORDER BY
  COALESCE(g.sort_order, 99),
  a.name;
