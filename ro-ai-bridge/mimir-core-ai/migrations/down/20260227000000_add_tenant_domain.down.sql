-- Rollback: 20260227000000_add_tenant_domain.sql
ALTER TABLE tenants DROP COLUMN domain;
