-- Add patient_id column to data_sources for patient-scoped document management
ALTER TABLE data_sources
  ADD COLUMN patient_id VARCHAR(255) NULL DEFAULT NULL AFTER tenant_id,
  ADD INDEX idx_patient_sources (tenant_id, patient_id);
