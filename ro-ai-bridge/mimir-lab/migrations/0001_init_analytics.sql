-- mimir-lab analytics registry (ADR-024) — MariaDB (Mimir `mimir` DB, asgard ns).
-- Relational metadata for the asgard_analytics tenant. Actual dataset DATA lives
-- in Parquet/DuckDB + MinIO; these tables are the catalog.
-- Tenant-scoped: every row carries tenant_id (default 'asgard_analytics').

-- 1. datasets — registry of every dataset
CREATE TABLE IF NOT EXISTS datasets (
  id            VARCHAR(64)  NOT NULL PRIMARY KEY,
  tenant_id     VARCHAR(50)  NOT NULL DEFAULT 'asgard_analytics',
  name          VARCHAR(255) NOT NULL,
  source_type   ENUM('upload','cross_tenant','external') NOT NULL DEFAULT 'upload',
  schema_json   LONGTEXT     NULL,
  storage_uri   VARCHAR(1024) NULL,
  row_count     BIGINT       NOT NULL DEFAULT 0,
  pii_status    ENUM('pending','clean','flagged') NOT NULL DEFAULT 'pending',
  pii_categories LONGTEXT     NULL,
  created_by    VARCHAR(255) NULL,
  created_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_datasets_tenant (tenant_id),
  INDEX idx_datasets_pii (pii_status)
);

-- 2. dataset_versions — immutable snapshots
CREATE TABLE IF NOT EXISTS dataset_versions (
  id            VARCHAR(64)  NOT NULL PRIMARY KEY,
  dataset_id    VARCHAR(64)  NOT NULL,
  version       INT          NOT NULL,
  storage_uri   VARCHAR(1024) NULL,
  checksum      VARCHAR(128) NULL,
  created_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uq_dataset_version (dataset_id, version),
  INDEX idx_dataset_versions_dataset (dataset_id)
);

-- 3. analyses — saved query / notebook / geo artifacts
CREATE TABLE IF NOT EXISTS analyses (
  id            VARCHAR(64)  NOT NULL PRIMARY KEY,
  tenant_id     VARCHAR(50)  NOT NULL DEFAULT 'asgard_analytics',
  title         VARCHAR(255) NOT NULL,
  kind          ENUM('sql','notebook','geo') NOT NULL DEFAULT 'sql',
  spec_json     LONGTEXT     NULL,
  created_by    VARCHAR(255) NULL,
  created_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_analyses_tenant (tenant_id)
);

-- 4. report_jobs — scheduled report config (driven by bifrost-jobs)
CREATE TABLE IF NOT EXISTS report_jobs (
  id                VARCHAR(64)  NOT NULL PRIMARY KEY,
  tenant_id         VARCHAR(50)  NOT NULL DEFAULT 'asgard_analytics',
  name              VARCHAR(255) NOT NULL,
  cron              VARCHAR(64)  NOT NULL,
  analysis_id       VARCHAR(64)  NULL,
  evidence_template VARCHAR(255) NULL,
  last_run          TIMESTAMP    NULL,
  status            ENUM('active','paused','error') NOT NULL DEFAULT 'active',
  created_at        TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at        TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_report_jobs_tenant (tenant_id)
);

-- 5. geo_layers — registered spatial layers (mimir-geo)
CREATE TABLE IF NOT EXISTS geo_layers (
  id            VARCHAR(64)  NOT NULL PRIMARY KEY,
  dataset_id    VARCHAR(64)  NOT NULL,
  geom_type     VARCHAR(32)  NULL,
  crs           VARCHAR(32)  NULL,
  bbox          VARCHAR(255) NULL,
  feature_count BIGINT       NOT NULL DEFAULT 0,
  created_at    TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_geo_layers_dataset (dataset_id)
);
