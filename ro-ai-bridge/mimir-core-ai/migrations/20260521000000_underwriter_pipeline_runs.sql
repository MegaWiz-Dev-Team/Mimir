-- Asgard Underwriter pipeline-run registry (G5 telemetry sink).
-- Iris registers one row per underwriting run, keyed by batch_id, so a run can
-- be reconstructed across the monitoring sinks (Vardr/Tyr/Laminar).
-- Distinct from the generic `pipeline_runs` table (LLM eval/extraction runs).

CREATE TABLE IF NOT EXISTS underwriter_pipeline_runs (
  batch_id           CHAR(36)    NOT NULL PRIMARY KEY,
  tenant_id          VARCHAR(64) NOT NULL,
  dataset_id         VARCHAR(64) NOT NULL,
  record_id          VARCHAR(64) NOT NULL,
  pipeline_id        VARCHAR(64) NOT NULL,
  model_step3        VARCHAR(128),
  model_step4        VARCHAR(128),
  started_at         DATETIME    NOT NULL,
  completed_at       DATETIME    NULL,
  total_elapsed_s    DECIMAL(10,3),
  status             ENUM('running','completed','failed') NOT NULL DEFAULT 'running',
  risk_band          VARCHAR(32),
  risk_score         INT,
  hitl_required      BOOLEAN,
  diagnoses_count    INT,
  summary_json       LONGTEXT,
  perf_json          LONGTEXT,
  telemetry_json     LONGTEXT,
  INDEX idx_tenant_dataset  (tenant_id, dataset_id),
  INDEX idx_record          (record_id),
  INDEX idx_started         (started_at),
  INDEX idx_status          (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
