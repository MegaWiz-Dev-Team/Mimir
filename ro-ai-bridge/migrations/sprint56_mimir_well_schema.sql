-- Sprint 56 — Mimir Well: memory artifact accumulation primitive
--
-- Adds the Tulving 3-tier (episodic/semantic/procedural) memory artifact store
-- + consolidation queue, per ADR-011. Surface labels (short/long/reasoning)
-- mirror the neo4j-labs/agent-memory taxonomy for UX consistency.
--
-- IMPORTANT: This migration is destructive (CREATE TABLE). Before applying:
--   1. Run ./scripts/backup-full-k8s.sh from Asgard/ and verify MANIFEST.md
--   2. Verify dump integrity (gzip -t, neo4j --verify dump)
--   3. Record backup id in docs/sprints/s56-execution-log.md
-- See: Asgard/docs/decisions/ADR-011-mimir-well-memory-artifacts.md §D6
--      Asgard/scripts/backup-neo4j-only.sh (Neo4j-only path)
--
-- Cross-refs:
--   - Asgard/docs/decisions/ADR-011-mimir-well-memory-artifacts.md (this design)
--   - Asgard/docs/sprints/syn-dicom-plan.md §S56 (sprint plan)
--   - mimir-well crate (ro-ai-bridge/mimir-well/) — writer/reader/consolidator

CREATE TABLE memory_artifact (
    id                  CHAR(26)        NOT NULL
        COMMENT 'ULID — sortable, K-safe',
    tenant_id           VARCHAR(64)     NOT NULL
        COMMENT 'asgard_medical | asgard_insurance | asgard_platform | ...',
    agent_id            VARCHAR(64)     NOT NULL
        COMMENT 'producing agent (eir-clinical, underwriter, ...)',
    case_id             VARCHAR(64)     DEFAULT NULL
        COMMENT 'optional case/session anchor',
    kind                ENUM('observation','abstraction','skill','correction','reference')
                        NOT NULL
        COMMENT 'artifact taxonomy (PROV-AGENT)',
    tier                ENUM('episodic','semantic','procedural') NOT NULL
        COMMENT 'Tulving 3-tier — primary storage classification',
    surface             ENUM('short','long','reasoning') NOT NULL
        COMMENT 'UX surface label — neo4j-labs nomenclature (short=episodic, long=semantic, reasoning=procedural)',
    content_hash        CHAR(64)        NOT NULL
        COMMENT 'sha256 of canonical content — auto-merge key',
    content             JSON            NOT NULL
        COMMENT 'artifact payload (structure varies by kind)',
    embedding           BLOB            DEFAULT NULL
        COMMENT 'BGE-M3 embedding for semantic search',
    prov_used           JSON            DEFAULT NULL
        COMMENT 'PROV-O wasInformedBy — artifact ids consumed',
    prov_generated_by   VARCHAR(255)    DEFAULT NULL
        COMMENT 'trace_id:span_id of generating activity',
    confidence          DECIMAL(4,3)    DEFAULT NULL
        COMMENT '0.000-1.000 producer confidence',
    promoted_from       VARCHAR(64)     DEFAULT NULL
        COMMENT 'Bifrost session id if promoted from memvid scratchpad',
    consolidation_state ENUM('fresh','reviewed','superseded','contradicted')
                        NOT NULL DEFAULT 'fresh',
    superseded_by       CHAR(26)        DEFAULT NULL
        COMMENT 'FK soft-ref to the artifact that replaced this one',
    created_at          TIMESTAMP       NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    KEY idx_tenant_tier_surface (tenant_id, tier, surface, created_at),
    KEY idx_consolidation       (tenant_id, consolidation_state),
    KEY idx_content_hash        (tenant_id, content_hash),
    KEY idx_promoted_from       (promoted_from),
    KEY idx_superseded_by       (superseded_by)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='Sprint 56: Mimir Well — Tulving 3-tier memory artifacts (ADR-011)';


CREATE TABLE well_consolidation_queue (
    id              BIGINT          NOT NULL AUTO_INCREMENT,
    tenant_id       VARCHAR(64)     NOT NULL,
    artifact_a      CHAR(26)        NOT NULL
        COMMENT 'lexicographically smaller ULID by convention',
    artifact_b      CHAR(26)        NOT NULL,
    similarity      DECIMAL(4,3)    NOT NULL
        COMMENT 'cosine similarity over embedding',
    kind            ENUM('near_dup','contradiction') NOT NULL,
    ls_task_id      BIGINT          DEFAULT NULL
        COMMENT 'Label Studio task id (well-consolidation project)',
    decided_at      TIMESTAMP       NULL DEFAULT NULL,
    decision        ENUM('merge','keep_both','supersede','flag_conflict')
                    DEFAULT NULL,
    decided_by      VARCHAR(100)    DEFAULT NULL
        COMMENT 'JWT subject of curator who decided',
    note            TEXT            DEFAULT NULL,
    created_at      TIMESTAMP       NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    UNIQUE KEY uq_pair (tenant_id, artifact_a, artifact_b),
    KEY idx_pending (tenant_id, decided_at),
    KEY idx_ls_task (ls_task_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='Sprint 56: consolidation queue feeding mimir-curator (ADR-011 §D3)';