-- Rollback: 202602180000_init_mariadb.sql
-- Drops initial monitoring tables

DROP TABLE IF EXISTS evaluation_reports;
DROP TABLE IF EXISTS qa_results;
DROP TABLE IF EXISTS pipeline_steps;
DROP TABLE IF EXISTS pipeline_runs;
