-- Asgard Security (SOC) tenant — operational security domain.
-- Holds security operational data + SOC agent configs, separate from clinical/
-- insurance domains. The reference security KB (OWASP/AI-security guidelines) is a
-- SHARED KB (tenant_id IS NULL), NOT this tenant — see bootstrap-shared-kbs.
--
-- Idempotent. Rollback: DELETE FROM tenant_configs WHERE tenant_id='asgard_security';
--                       DELETE FROM tenants        WHERE id='asgard_security';
-- Apply: mariadb -u mimir -p<pass> mimir < seed-asgard-security-tenant.sql

INSERT INTO tenants (id, name, domain, service_type, description)
VALUES (
  'asgard_security',
  'Asgard Security',
  'security.asgard.local',
  'security',
  'AI SOC — security operations (scan findings, audit/incident, SOC agents): Odin / Huginn / Muninn / Loki / Tyr.'
)
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  domain = VALUES(domain),
  service_type = VALUES(service_type),
  description = VALUES(description);

INSERT INTO tenant_configs (tenant_id, default_provider, default_model, max_daily_tokens, is_dedicated_vector_db)
VALUES (
  'asgard_security',
  'heimdall',
  'gemma-4-26b',
  1000000,
  0
)
ON DUPLICATE KEY UPDATE
  default_provider = VALUES(default_provider),
  default_model = VALUES(default_model),
  max_daily_tokens = VALUES(max_daily_tokens);
