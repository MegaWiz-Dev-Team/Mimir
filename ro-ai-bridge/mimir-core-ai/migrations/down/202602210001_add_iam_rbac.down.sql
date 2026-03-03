-- Rollback: 202602210001_add_iam_rbac.sql
-- Drops IAM tables and seed data

DELETE FROM tenant_users WHERE user_id = '00000000-0000-0000-0000-000000000000';
DELETE FROM users WHERE id = '00000000-0000-0000-0000-000000000000';
DELETE FROM tenants WHERE id = 'default_tenant';

DROP TABLE IF EXISTS tenant_users;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS tenants;
