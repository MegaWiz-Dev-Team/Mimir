-- Asgard Analytics tenant — data-analysis / visualization / research / spatial domain.
-- Holds analytics operational data (datasets, analyses, report jobs) + analyst-* agent
-- configs, separate from clinical / insurance / platform domains. See ADR-024.
--
-- Engines (mimir-lab / mimir-geo) are Tier B code; the analyst-* agent configs seeded by
-- seed-asgard-analytics-agents.sql are tuned data → effectively Tier C, per-box.
--
-- ⚠️ The `tenants` / `tenant_configs` tables live in the **asgard-infra** MariaDB
--    (Bifrost DB), NOT the `asgard` MariaDB (which holds agent_configs). Apply here.
--
-- Idempotent. Rollback: DELETE FROM tenant_configs WHERE tenant_id='asgard_analytics';
--                       DELETE FROM tenants        WHERE id='asgard_analytics';
-- Apply (K8s/OrbStack):
--   POD=$(kubectl get po -n asgard-infra --no-headers | awk '/^mariadb/&&$3=="Running"{print $1;exit}')
--   kubectl exec -i -n asgard-infra "$POD" -- sh -c 'mariadb -uroot \
--     -p"${MYSQL_ROOT_PASSWORD:-${MARIADB_ROOT_PASSWORD:-root}}" mimir' < seed-asgard-analytics-tenant.sql
-- Applied: 2026-06-09

INSERT INTO tenants (id, name, domain, service_type, description)
VALUES (
  'asgard_analytics',
  'Asgard Analytics',
  'analytics.asgard.local',
  'analytics',
  'Data analysis platform — dataset analysis, visualization, research, and spatial (GIS + statistical) analysis. Engines: mimir-lab (DuckDB) + mimir-geo (GeoRust). Agents: analyst-router/-sql/-geo/-stats/-research.'
)
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  domain = VALUES(domain),
  service_type = VALUES(service_type),
  description = VALUES(description);

INSERT INTO tenant_configs (tenant_id, default_provider, default_model, max_daily_tokens, is_dedicated_vector_db)
VALUES (
  'asgard_analytics',
  'heimdall',
  'gemma-4-26b',
  1000000,
  0
)
ON DUPLICATE KEY UPDATE
  default_provider = VALUES(default_provider),
  default_model = VALUES(default_model),
  max_daily_tokens = VALUES(max_daily_tokens);
