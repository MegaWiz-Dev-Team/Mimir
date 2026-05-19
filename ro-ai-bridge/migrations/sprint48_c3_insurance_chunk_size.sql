-- Sprint 48 C.3 — Insurance chunk size remediation
--
-- Audit (per `mimir_chunking_audit` memory 2026-05-17): BGE-M3 has 8K
-- token context but Mimir's default `auto_recommend` returns chunk_size
-- = 500 *chars* (~150 tokens) — context underutilized for long-form
-- insurance policy documents.
--
-- Fix: pin `asgard_insurance` tenant to:
--   chunk_strategy = recursive  (preserves markdown hierarchy)
--   chunk_size     = 2400 chars (~600-800 tokens)
--   chunk_overlap  = 200 chars
--
-- Recursive strategy splits on `## ` / `### ` markdown headings first,
-- only falling back to fixed-window inside oversized sections. Insurance
-- documents are heading-rich (Sections, Sub-clauses, Definitions),
-- so heading-aware split keeps semantic units intact.
--
-- Why 2400 / not 4000+: BGE-M3 attention quality degrades beyond ~1024
-- tokens per chunk per upstream evaluations; 2400 chars stays in the
-- 600-800 token sweet spot while ~3-5× the old default size.
--
-- Idempotent — uses INSERT ... ON DUPLICATE KEY UPDATE pattern.

INSERT INTO tenant_configs (tenant_id, pipeline_settings, updated_at)
VALUES (
  'asgard_insurance',
  JSON_OBJECT(
    'chunk_strategy', 'recursive',
    'chunk_size',     2400,
    'chunk_overlap',  200,
    'rationale',      'Sprint 48 C.3 — bump from 500 char default to 2400 (~700 tokens). BGE-M3 8K context utilized; recursive preserves markdown hierarchy in insurance policy text.'
  ),
  NOW()
)
ON DUPLICATE KEY UPDATE
  pipeline_settings = JSON_OBJECT(
    'chunk_strategy', 'recursive',
    'chunk_size',     2400,
    'chunk_overlap',  200,
    'rationale',      'Sprint 48 C.3 — bump from 500 char default to 2400 (~700 tokens). BGE-M3 8K context utilized; recursive preserves markdown hierarchy in insurance policy text.'
  ),
  updated_at = NOW();
