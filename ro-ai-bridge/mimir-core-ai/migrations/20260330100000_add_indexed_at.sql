-- Add indexed_at tracking columns for Vector DB synchronization
-- Fixes Issue: "Unknown column 'qr.indexed_at' in 'WHERE'"

ALTER TABLE qa_results 
ADD COLUMN IF NOT EXISTS indexed_at TIMESTAMP NULL DEFAULT NULL;

ALTER TABLE qa_clusters 
ADD COLUMN IF NOT EXISTS indexed_at TIMESTAMP NULL DEFAULT NULL;
