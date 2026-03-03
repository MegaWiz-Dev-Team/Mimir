-- Issue #164: Configurable Max Crawl Pages
-- Add max_crawl_pages to tenant_configs for admin-configurable crawl limits
SET @dbname = DATABASE();
SET @tablename = 'tenant_configs';
SET @columnname = 'max_crawl_pages';
SET @preparedStatement = (SELECT IF(
  (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA = @dbname AND TABLE_NAME = @tablename AND COLUMN_NAME = @columnname) > 0,
  'SELECT 1',
  'ALTER TABLE tenant_configs ADD COLUMN max_crawl_pages INT NOT NULL DEFAULT 100'
));
PREPARE alterIfNotExists FROM @preparedStatement;
EXECUTE alterIfNotExists;
DEALLOCATE PREPARE alterIfNotExists;
