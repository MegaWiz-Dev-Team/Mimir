-- Drop pageindex_tree column from data_sources

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
  'ALTER TABLE data_sources DROP COLUMN pageindex_tree;',
  'SELECT 1'
));
PREPARE alterIfExists FROM @preparedStatement;
EXECUTE alterIfExists;
DEALLOCATE PREPARE alterIfExists;
