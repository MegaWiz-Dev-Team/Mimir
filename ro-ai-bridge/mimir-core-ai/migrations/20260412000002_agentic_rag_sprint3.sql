-- T3.3: QC Status in Dataset
ALTER TABLE rag_eval_datasets ADD COLUMN IF NOT EXISTS qc_status VARCHAR(20) DEFAULT 'Draft';
