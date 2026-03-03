-- Rollback: 20260223132000_add_tenant_configs.sql
DELETE FROM tenant_configs WHERE tenant_id = 'default_tenant';
DROP TABLE IF EXISTS tenant_configs;
