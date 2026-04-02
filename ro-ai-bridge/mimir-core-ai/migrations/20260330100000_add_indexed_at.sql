-- Add indexed_at tracking column for QC results and clusters
ALTER TABLE qa_results ADD COLUMN IF NOT EXISTS indexed_at TIMESTAMP NULL DEFAULT NULL;
ALTER TABLE qa_clusters ADD COLUMN IF NOT EXISTS indexed_at TIMESTAMP NULL DEFAULT NULL;
