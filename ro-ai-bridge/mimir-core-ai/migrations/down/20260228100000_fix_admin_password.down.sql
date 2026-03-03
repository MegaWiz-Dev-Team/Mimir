-- Rollback: 20260228100000_fix_admin_password.sql
-- NOTE: Data-only migration. Cannot reverse password hash change.
-- No-op rollback.
SELECT 'No-op: Cannot reverse password hash update' AS info;
