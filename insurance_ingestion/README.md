# Sprint 1: Insurance Product Knowledge Ingestion

**Timeline:** May 18-27, 2026 (8 days, 4 FTE)  
**Objective:** Ingest 950+ document chunks and 500+ entity records from insurance product sources into Mimir RAG engine.

## Quick Start

### Setup (Day 1, 10 min)

```bash
# Create virtual environment
python3 -m venv venv && source venv/bin/activate

# Install dependencies
pip install -r requirements.txt

# Run unit tests (should all pass)
python main.py --test

# Configure local environment (see .env.example)
cp .env.example .env
```

### Run Pipeline

```bash
# Phase 1 only (extraction)
python main.py --phase 1

# All phases end-to-end (1-5)
python main.py --phase 1-5

# Phase 5 only (validation, no re-ingestion)
python main.py --phase 5 --skip-ingest

# Quiet mode (minimal output)
python main.py --phase 1-5 --quiet

# Custom config
python main.py --config config/production.json
```

## Design System

### Core Patterns

**Configuration** (immutable):
```python
config = PipelineConfig(
    tenant_id="asgard_insurance",
    mimir_base_url="http://mimir.asgard.svc:8000",
    # ... 15 more settings
)
```

**Logging** (context-aware):
```python
logger = PipelineLogger(Phase.EXTRACTION)
logger.success("✅ Extracted 10 documents")
logger.warning("⚠️ 1 URL timeout (retried)")
logger.error("❌ BGE-M3 endpoint unreachable")
```

**Data Classes** (strongly-typed):
```python
chunk = Chunk(
    source_id="health_plan_a_001",
    content="...",
    metadata={...},  # 21 required keys
    chunk_index=0,
    tokens=285,
)

entity = Entity(
    entity_id="PROD_001",
    name="Health Plan A",
    entity_type="Product",  # enum
    properties={"coverage_type": "health", ...},
)
```

**Error Handling** (domain-specific):
```python
try:
    result = ingest_chunks_to_mimir(chunks, config)
except IngestionError as e:
    logger.error(f"Mimir API failed: {e}")
except EmbeddingError as e:
    # Fallback: switch to Typhoon model
```

## Testing Strategy

### Unit Tests (Fast, No Services)

```bash
# All unit tests
pytest insurance_ingestion/tests/unit -v

# Single test
pytest insurance_ingestion/tests/unit/test_phase1_extraction.py::TestPhase1Extraction::test_chunk_document_splits_on_paragraph_boundary -v
```

### Integration Tests (K8s Services Required)

```bash
# All integration tests (slow)
pytest -m integration

# Skip integration tests
pytest -m "not integration"
```

### Test Fixtures

Sample data lives in `tests/fixtures/sample_data.py`:
- `SAMPLE_CHUNKS`: 3 realistic insurance document chunks
- `SAMPLE_ENTITIES`: Product, Benefit, Exclusion entities
- `SAMPLE_TEST_QUERIES`: 10 test queries across 4 tiers (lookup, reasoning, exclusion, robustness)

## Phases Overview

### Phase 1: Extraction (May 18-19)
**Input:** 8 public web URLs (health plans, coverage specs)  
**Output:** `phase1_chunks.jsonl` (950+ chunks, ~300 tokens each)  
**Success:** 960 chunks extracted with sequential indices

### Phase 2: Schema Normalization (May 20)
**Input:** `phase1_chunks.jsonl`  
**Output:** `phase2_normalized.jsonl` (with all 21 metadata keys)  
**Success:** Vendor names abstracted, zero null fields

### Phase 3: Entity Extraction (May 21)
**Input:** `phase2_normalized.jsonl`  
**Output:** `phase3_entities.jsonl`, `phase3_edges.jsonl`  
**Success:** 500+ entities (Product/Coverage/Benefit), 1000 relations

### Phase 4: Ingestion (May 22-24)
**Input:** Normalized chunks + entities  
**Output:** Data in Mimir, Neo4j, Qdrant (BGE-M3 embeddings)  
**Success:** 950 chunks ingested, vectors indexed

### Phase 5: Validation (May 22, 27)
**Input:** Test queries against live Mimir  
**Output:** Hit Rate@3, latency metrics, PII scan report  
**Success Criteria:**
- [ ] Hit Rate@3 ≥ 75%
- [ ] Search latency < 500ms
- [ ] Zero PII in results

## Decision Gates

### May 22 (Post-Phase 4)
```
IF Hit Rate@3 < 50%:
  → Activate Plan B: Switch BGE-M3 → Typhoon embedding model
  → Re-run Phase 4 with Typhoon
ELSE:
  → Proceed to Phase 5 full validation
```

### May 27 (End of Sprint)
```
GO / NO-GO decision:
- 950/950 chunks ingested? ✓
- 500/500 entities indexed? ✓
- Hit Rate@3 ≥ 75%? ✓
- Zero data quality errors? ✓
→ Deploy to production
```

## File Structure

```
insurance_ingestion/
├── __init__.py
├── main.py                      # CLI entry point
├── core.py                      # Design system (config, logger, types)
├── requirements.txt
├── pytest.ini
│
├── phases/
│   ├── __init__.py
│   ├── phase1_extraction.py     # Extract from URLs → chunks
│   ├── phase2_schema.py         # Normalize schema, abstract PII
│   ├── phase3_entities.py       # Extract entities → knowledge graph
│   ├── phase4_ingestion.py      # Ingest to Mimir, embed, index
│   └── phase5_validation.py     # Search quality validation
│
├── tests/
│   ├── conftest.py              # Pytest fixtures + mocks
│   ├── __init__.py
│   ├── fixtures/
│   │   ├── __init__.py
│   │   └── sample_data.py       # Test data (chunks, entities, queries)
│   ├── unit/
│   │   ├── test_phase1_extraction.py
│   │   ├── test_phase2_schema.py
│   │   ├── test_phase3_entities.py
│   │   ├── test_phase4_ingestion.py
│   │   └── test_phase5_validation.py
│   └── integration/
│       ├── test_mimir_ingestion.py
│       ├── test_neo4j_graph.py
│       └── test_qdrant_search.py
│
└── docs/
    ├── SPRINT_1_LOG.md          # Daily standup log + metrics
    └── FALLBACK_STRATEGY.md     # Plan B (Typhoon) decision tree
```

## Execution Checklist

**Pre-Kickoff (May 17):**
- [ ] Git branch created
- [ ] TDD scaffolding complete
- [ ] K8s pods verified (Bifrost, Mimir, Neo4j, Qdrant)
- [ ] Heimdall embeddings endpoint responding
- [ ] Mimir tenant `asgard_insurance` ready
- [ ] Pre-ingestion Skuggi scanning configured

**Day 1 (May 18):**
- [ ] Team kickoff + role assignments
- [ ] Phase 1 extraction started
- [ ] Daily standup template in use
- [ ] First test queries drafted

**Ongoing:**
- [ ] Run `pytest -m "not integration"` (10 min, should pass)
- [ ] Update `docs/SPRINT_1_LOG.md` after each standup
- [ ] Check Hit Rate @May 22 for fallback decision

**End of Sprint (May 27):**
- [ ] All 5 phases complete
- [ ] Acceptance criteria verified
- [ ] Pull request ready for merge

## Troubleshooting

### Mimir 503: Service Unavailable
```bash
# Check if local image is loaded
docker image ls | grep mimir

# If missing, rebuild locally
docker build -t mimir:local . && \
  docker tag mimir:local ghcr.io/asgard-ai/mimir:s1-insurance
```

### Hit Rate < 75%
1. Check test queries are representative
2. Verify chunk quality (no truncation, good boundaries)
3. Check BGE-M3 embedding dimension (must be 1024)
4. If still <50%, activate Plan B (see `FALLBACK_STRATEGY.md`)

### Timeout in Phase 4
- Reduce `batch_size` in config (default 100 → 50)
- Check Neo4j DB write locks: `SHOW TRANSACTIONS;`
- Verify Qdrant disk space: `curl http://qdrant:6333/health`

---

**Docs:** See [SPRINT_PLAN_asgard_insurance_km.md](../docs/SPRINT_PLAN_asgard_insurance_km.md) for full scope  
**DRIs:** Data Engineer (Phase 1), Backend (Phases 2-4), QA (Phase 5)  
**Contact:** paripol@megawiz.co
