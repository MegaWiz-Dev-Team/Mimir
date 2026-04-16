-- Update default provider and model for tenant_configs
ALTER TABLE tenant_configs 
MODIFY COLUMN default_provider VARCHAR(50) NOT NULL DEFAULT 'heimdall',
MODIFY COLUMN default_model VARCHAR(50) NOT NULL DEFAULT 'mlx-community/Qwen3.5-35B-A3B-4bit';
