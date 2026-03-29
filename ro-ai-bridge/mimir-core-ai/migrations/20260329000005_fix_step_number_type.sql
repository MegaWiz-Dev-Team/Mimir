-- Description: Fix step_number type to TINYINT UNSIGNED to match sqlx u8 mapping.

ALTER TABLE pipeline_run_steps MODIFY step_number TINYINT UNSIGNED NOT NULL;
