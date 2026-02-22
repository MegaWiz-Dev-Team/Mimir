-- Modify error_message column to MEDIUMTEXT to allow for larger error payloads
ALTER TABLE pipeline_steps MODIFY COLUMN error_message MEDIUMTEXT;
