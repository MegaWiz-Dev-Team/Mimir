-- Migration: 20260501000000_medical_schema
-- Creates the `medical` schema and all Sprint 1 tables on asgard_postgres
-- Run against: mimir database (not zitadel)

-- Create database user if not exists (run as superuser)
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'mimir') THEN
    CREATE ROLE mimir WITH LOGIN PASSWORD 'mimir_pg_password';
  END IF;
END $$;

-- Grant connect
GRANT CONNECT ON DATABASE mimir TO mimir;

-- Medical schema (separate from public used by zitadel)
CREATE SCHEMA IF NOT EXISTS medical AUTHORIZATION mimir;

SET search_path TO medical;

-- PubMed article cache (metadata only; vectors live in Qdrant "pubmed-abstracts")
CREATE TABLE IF NOT EXISTS medical.pubmed_articles (
    pmid         BIGINT PRIMARY KEY,
    title        TEXT NOT NULL,
    abstract     TEXT,
    mesh_terms   JSONB,
    pub_date     DATE,
    fetched_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS pubmed_pub_date_idx
    ON medical.pubmed_articles (pub_date DESC);

CREATE INDEX IF NOT EXISTS pubmed_mesh_terms_idx
    ON medical.pubmed_articles USING GIN (mesh_terms);

-- Clinical guidelines (long-term medical knowledge; vectors live in Qdrant "clinical-wisdom")
CREATE TABLE IF NOT EXISTS medical.clinical_guidelines (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title          TEXT NOT NULL,
    body           TEXT,
    source         TEXT,
    version        TEXT,
    effective_date DATE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS guidelines_source_idx
    ON medical.clinical_guidelines (source);

CREATE INDEX IF NOT EXISTS guidelines_effective_date_idx
    ON medical.clinical_guidelines (effective_date DESC);

-- Agent audit trail (all tool calls from Medical Agents)
CREATE TABLE IF NOT EXISTS medical.agent_audit_log (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  TEXT,
    agent_id   TEXT,
    tool_name  TEXT NOT NULL,
    input      JSONB,
    output     JSONB,
    latency_ms INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS audit_tenant_created_idx
    ON medical.agent_audit_log (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS audit_tool_name_idx
    ON medical.agent_audit_log (tool_name);

-- Grant all on schema to mimir user
GRANT USAGE ON SCHEMA medical TO mimir;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA medical TO mimir;
ALTER DEFAULT PRIVILEGES IN SCHEMA medical
    GRANT ALL PRIVILEGES ON TABLES TO mimir;
