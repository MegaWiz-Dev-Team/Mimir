-- Add pageindex_tree JSON column to data_sources table
-- Allows storing hierarchical document trees (VectifyAI PageIndex format)

SET @dbname = DATABASE();
SET @tablename = 'data_sources';
SET @columnname = 'pageindex_tree';
SET @preparedStatement = (SELECT IF(
  (
    SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
    WHERE
      (table_name = @tablename)
      AND (table_schema = @dbname)
      AND (column_name = @columnname)
  ) > 0,
  'SELECT 1',
  'ALTER TABLE data_sources ADD COLUMN pageindex_tree JSON NULL;'
));
PREPARE alterIfNotExists FROM @preparedStatement;
EXECUTE alterIfNotExists;
DEALLOCATE PREPARE alterIfNotExists;
