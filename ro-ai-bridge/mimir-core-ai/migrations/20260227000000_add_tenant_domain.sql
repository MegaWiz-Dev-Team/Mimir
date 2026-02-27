-- Migration: Add domain column to tenants table
-- Issue #76: Domain Connector Architecture (Medical vs Game)
-- Supports: 'game', 'medical', 'general' (default)

SET @column_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'tenants' AND COLUMN_NAME = 'domain');
SET @sql = IF(@column_exists = 0, 'ALTER TABLE tenants ADD COLUMN domain VARCHAR(20) NOT NULL DEFAULT ''general''', 'SELECT 1');
PREPARE stmt FROM @sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;
