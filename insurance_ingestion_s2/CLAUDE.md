# Sprint 1 Insurance Ingestion — Team Setup & Execution Guide

**Sprint:** S1 (May 18-27, 2026)  
**Objective:** Ingest 950+ document chunks + 500+ entities into Mimir RAG  
**DRIs:** Data Engineer (Phase 1), Backend (2-4), QA (5), DevOps (infra)

---

## 🚀 Day 1 Setup (May 18, 9:00 AM)

### Step 1: Clone & Activate (2 min)

```bash
cd /Users/mimir/Developer/Mimir
git checkout feature/insurance-s1-ingestion
cd insurance_ingestion

# Create virtual environment
python3 -m venv venv
source venv/bin/activate

# Verify Python 3.10+
python --version
```

### Step 2: Install Dependencies (3 min)

```bash
pip install -r requirements.txt

# Verify key packages
python -c "import pytest; import pydantic; import requests; print('✅ All deps installed')"
```

### Step 3: Verify Imports (1 min)

```bash
python -c "from core import Phase, PipelineLogger, Chunk; print('✅ Core imports OK')"
```

### Step 4: Run Unit Tests (2 min)

```bash
# Fast tests (no K8s required)
python main.py --test

# Should see: 20+ tests, 0 failures
# If any fail: check Python version, pydantic installed
```

---

## 🔧 K8s Preflight (DevOps, 5 min)

### Pre-requisite: K8s Services Running

```bash
# Check pods are running
kubectl get pods -n asgard | grep -E "mimir|bifrost|neo4j|qdrant"

# Expected output:
# bifrost-xxxxx          1/1     Running
# mimir-xxxxx            1/1     Running
# neo4j-xxxxx            1/1     Running
# qdrant-xxxxx           1/1     Running
```

### Port Forwarding (if not already done)

```bash
# Terminal 1: Mimir
kubectl port-forward -n asgard svc/mimir 8000:8000 &

# Terminal 2: Neo4j
kubectl port-forward -n asgard svc/neo4j 7687:7687 &

# Terminal 3: Qdrant
kubectl port-forward -n asgard svc/qdrant 6333:6333 &

# Terminal 4: Heimdall (embeddings)
kubectl port-forward -n asgard svc/heimdall 8001:8001 &

# Verify connectivity
curl -s http://localhost:8000/health && echo "✅ Mimir ready"
curl -s http://localhost:6333/health && echo "✅ Qdrant ready"
```

### Configure Environment

```bash
# Copy and edit
cp .env.example .env

# Update with actual endpoints (if not localhost:8000)
vim .env
```

---

## 📋 Daily Execution (9:00 AM Standup)

### Update Sprint Log

```bash
# Edit today's section in docs/SPRINT_1_LOG.md
vim docs/SPRINT_1_LOG.md

# Add:
# - What phase you're on
# - Blockers (if any)
# - Metrics (chunks extracted, entities, latency)
# - Decisions made
```

### Run Current Phase

**Phase 1: Extraction** (May 18-19, Data Eng)
```bash
python main.py --phase 1

# Output: phase1_chunks.jsonl (960 chunks, ~300 tokens each)
# Time: ~5 min
# Success: 960 chunks extracted
```

**Phase 2: Schema** (May 20, Backend)
```bash
python main.py --phase 2

# Reads: phase1_chunks.jsonl
# Output: phase2_normalized.jsonl (all 21 metadata keys, PII abstracted)
# Time: ~2 min
```

**Phase 3: Entities** (May 21, Backend)
```bash
python main.py --phase 3

# Output: phase3_entities.jsonl (500+ entities)
#         phase3_edges.jsonl (1000+ relationships)
# Time: ~3 min
```

**Phase 4: Ingestion** (May 22-24, Backend + DevOps)
```bash
python main.py --phase 4

# Posts to Mimir, generates embeddings, indexes Qdrant
# Time: ~10-15 min
# Monitor: curl http://localhost:8000/metrics
```

**Phase 5: Validation** (May 22 check-in, May 27 final, QA)
```bash
# Standalone validation (no re-ingestion)
python main.py --phase 5 --skip-ingest

# Or after Phase 4:
python main.py --phase 4-5
```

---

## 🧪 Testing During Sprint

### Unit Tests (Fast, before standup)

```bash
# Run all unit tests
pytest insurance_ingestion/tests/unit -v

# Run specific phase
pytest insurance_ingestion/tests/unit/test_phase1_extraction.py -v

# Single test
pytest insurance_ingestion/tests/unit/test_phase1_extraction.py::TestPhase1Extraction::test_chunk_document_splits_on_paragraph_boundary -v
```

### Skip Integration Tests (K8s not needed for unit tests)

```bash
# Unit only (recommended for local dev)
pytest -m "not integration"

# Integration only (requires K8s)
pytest -m integration
```

### Quick Smoke Test

```bash
# Verify pipeline can start
python main.py --phase 1 --quiet

# Check for import errors
python -c "from phases.phase1_extraction import run_phase1; print('✅ Phase 1 imports OK')"
```

---

## 📊 Monitoring & Metrics

### Real-Time Progress

Each phase outputs:
```
[1_extraction] INFO | ▓▓▓▓▓▓▓▓▓░░░░░░░░░░ 45% (9/20)
[1_extraction] ✅ SUCCESS | Extracted 960 chunks (285,600 tokens)
```

### Daily Dashboard (update in SPRINT_1_LOG.md)

```markdown
| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Chunks | 950 | 960 | ✅ |
| Entities | 500 | 487 | 🟡 |
| Hit Rate@3 | ≥75% | 72% | 🟡 |
```

### Check Mimir Health

```bash
# API health
curl -s http://localhost:8000/health | jq .

# Database stats
curl -s http://localhost:8000/api/stats | jq .

# Vector DB stats
curl -s http://localhost:6333/collections/insurance_products | jq '.result.points_count'
```

---

## 🚨 Decision Gates

### May 22 (End of Phase 4) — Hit Rate Check

```bash
python main.py --phase 5

# Check Hit Rate@3 in output
# If ≥75%: ✅ Proceed to Phase 5 full validation
# If 50-74%: 🟡 Retry with query optimization
# If <50%: 🔴 Activate Plan B (switch embedding model)
```

**Fallback Activation (Plan B):**

```bash
# If Hit Rate < 50%, edit config:
vim .env
# Change: EMBEDDINGS_MODEL=typhoon

# Rebuild embeddings with Typhoon
python main.py --phase 4 --skip-mimir

# Re-validate
python main.py --phase 5
```

### May 27 (End of Sprint) — Final GO/NO-GO

Checklist:
```
✅ 950/950 chunks ingested?
✅ 500/500 entities indexed?
✅ Hit Rate@3 ≥ 75%?
✅ Latency < 500ms per query?
✅ Zero PII in results?
✅ Zero data quality errors?
✅ All unit tests passing?

Decision: GO to production / NO-GO, needs fixes
```

---

## 🐛 Troubleshooting

### Mimir 503 (Service Unavailable)

```bash
# Check pod is running
kubectl get pod -n asgard -l app=mimir

# Check logs
kubectl logs -n asgard -l app=mimir --tail=50 | grep ERROR

# Restart if needed
kubectl rollout restart -n asgard deployment/mimir

# Wait for rollout
kubectl rollout status -n asgard deployment/mimir
```

### Port Forward Already in Use

```bash
# Kill existing port-forward
lsof -i :8000
kill -9 <PID>

# Restart
kubectl port-forward -n asgard svc/mimir 8000:8000
```

### Chunk Extraction Timeout

```bash
# Reduce batch size (default 100)
python main.py --phase 1
# Edit: config.batch_size = 50

# Or increase timeout
export REQUEST_TIMEOUT=60
python main.py --phase 1
```

### Hit Rate < 50% (Fallback Activation)

```bash
# 1. Check test queries are representative
cat tests/fixtures/sample_data.py | grep SAMPLE_TEST_QUERIES

# 2. Verify chunk quality
head -5 data/output/phase1_chunks.jsonl | jq .

# 3. Check embedding dimension
curl -s http://localhost:8001/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["test"], "model": "bge-m3"}' | jq '.embeddings[0] | length'
# Should be 1024

# 4. If still <50%, activate Plan B
# See: Decision Gates → Fallback Activation
```

### Neo4j Write Timeout

```bash
# Check active transactions
curl -u neo4j:password bolt://localhost:7687 \
  "SHOW TRANSACTIONS;"

# Increase batch size in config
vim .env
# NEO4J_BATCH_SIZE=50  (reduce from 100)

# Restart ingestion
python main.py --phase 4
```

---

## 📁 Key Files

| File | Purpose | Who | When |
|------|---------|-----|------|
| `main.py` | Pipeline orchestrator | Everyone | Run daily |
| `core.py` | Design system (config, logger) | Developers | Reference |
| `phases/phase*.py` | Each phase implementation | Phase owner | Implement |
| `tests/` | Unit tests + fixtures | QA | Write tests first |
| `docs/SPRINT_1_LOG.md` | Daily standup log | Scrum master | Update 9:00 AM |
| `.env` | Local config (K8s endpoints) | DevOps | Setup once |
| `README.md` | Full documentation | Everyone | Reference |

---

## 🎯 Phase Ownership & Timeline

| Phase | DRI | Dates | Acceptance Criteria |
|-------|-----|-------|---------------------|
| 1: Extract | Data Eng | 5/18-5/19 | 960 chunks, sequential indices |
| 2: Schema | Backend | 5/20 | All 21 metadata keys, vendor abstraction |
| 3: Entities | Backend | 5/21 | 500+ entities, 1000+ Neo4j edges |
| 4: Ingest | Backend + DevOps | 5/22-5/24 | Data in Mimir, embeddings, indexed in Qdrant |
| 5: Validate | QA | 5/22 (gate), 5/27 (final) | Hit Rate@3 ≥ 75%, latency < 500ms |

---

## 📞 Quick Reference

### Common Commands

```bash
# Run phase 1 only
python main.py --phase 1

# Run all phases (1-5)
python main.py --phase 1-5

# Validate existing data (skip ingestion)
python main.py --phase 5 --skip-ingest

# Run tests
python main.py --test

# Run tests + show coverage
pytest --cov=insurance_ingestion

# Quiet mode (minimal output)
python main.py --phase 1-5 --quiet

# Custom config
python main.py --config config/production.json
```

### Logs & Output

```bash
# All output
ls -la data/output/
# phase1_chunks.jsonl
# phase2_normalized.jsonl
# phase3_entities.jsonl
# phase3_edges.jsonl

# Pipeline logs
tail -f logs/pipeline.log

# Check progress
cat docs/SPRINT_1_LOG.md | grep "Day 1"
```

### Service Health

```bash
# All services
for svc in mimir neo4j qdrant heimdall; do
  echo -n "$svc: "
  curl -s http://localhost:$([ "$svc" = "mimir" ] && echo 8000 || echo 6333)/health && echo "✅" || echo "❌"
done

# Detailed health
curl http://localhost:8000/api/health | jq .
```

---

## ✅ Pre-Kickoff Checklist (May 17)

- [ ] Git branch checked out: `feature/insurance-s1-ingestion`
- [ ] venv activated: `source venv/bin/activate`
- [ ] Dependencies installed: `pip install -r requirements.txt`
- [ ] Unit tests pass: `python main.py --test`
- [ ] Imports work: `python -c "from core import *"`
- [ ] K8s pods running: `kubectl get pods -n asgard | grep mimir`
- [ ] Port forwarding active: `curl http://localhost:8000/health`
- [ ] .env configured with K8s endpoints
- [ ] Team assigned to phases (see Phase Ownership table)
- [ ] Daily standup time set (9:00 AM)

---

## 📚 References

- **Sprint Plan:** [SPRINT_PLAN_asgard_insurance_km.md](../SPRINT_PLAN_asgard_insurance_km.md)
- **Data Schema:** [PRUDENTIAL_DATA_INGESTION_SUMMARY.md](../PRUDENTIAL_DATA_INGESTION_SUMMARY.md)
- **Design System:** [insurance_ingestion/core.py](core.py) — config, logger, types
- **README:** [insurance_ingestion/README.md](README.md) — full documentation
- **Test Fixtures:** [insurance_ingestion/tests/fixtures/sample_data.py](tests/fixtures/sample_data.py)

---

**Last Updated:** 2026-05-16  
**Contact:** paripol@megawiz.co
