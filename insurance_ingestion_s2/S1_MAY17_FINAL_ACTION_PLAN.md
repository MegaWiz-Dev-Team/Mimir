# S1 Sprint: May 17 Final Action Plan
## All 3 Critical Decisions LOCKED IN ✅

**Date:** May 16, 2026, Evening  
**Execution Start:** May 17, 2026, 9:00 AM  
**Deadline:** May 17, 2026, 5:00 PM  
**Status:** READY TO EXECUTE

---

## 🎯 3 DECISIONS CONFIRMED

### ✅ DECISION 1: Search UI = Option C (Hybrid)
- **Actual validation:** CLI method (100% reliable, no UI dependency)
- **Demo approach:** UI screenshots (visual polish for stakeholders)
- **Implementation:** 2-3 hours non-critical path (May 20-21, can slip to May 22 morning)
- **Owner:** UX/UI
- **May 22 Impact:** None - CLI validation is ready now
- **Status:** LOCKED ✅

### ✅ DECISION 2: Metrics Dashboard = Option B (Grafana)
- **Found:** Vardr has Prometheus + Grafana already running! 🎉
- **Grafana URL:** http://localhost:30300 (admin / asgard-grafana)
- **Prometheus:** Default data source (monitoring-kube-prometheus-prometheus:9090)
- **Implementation:** 1.5 hours (instrument scripts + create dashboard)
- **Owner:** Tech Lead (Data Engineer assists with metrics instrumentation)
- **Status:** LOCKED ✅

### ✅ DECISION 3: Deduplication = Option A (Ready NOW)
- **Timeline:** May 17 EOD (complete script + test)
- **Use in production:** May 20-21 (S1.2 phase)
- **Owner:** Data Engineer
- **Status:** LOCKED ✅

---

## 📋 DETAILED ACTION ITEMS (May 17, 9:00 AM - 5:00 PM)

### 🔴 MORNING SESSION (9:00 AM - 12:30 PM)

#### Data Engineer Tasks (3 items, ~2.5 hours)

**TASK 1: Entity Extraction Config** ⏱️ 1.5 hours
```
Timeline: 9:00 AM - 10:30 AM

Checklist:
  ☐ Read CONFIG_ENTITY_EXTRACTION.md (created May 16)
  ☐ Copy config into scripts/extract_entities.py
  ☐ Install: pip install spacy pythainlp tiktoken
  ☐ Download: python -m spacy download en_core_web_sm
  ☐ Test on smoke data (1 URL → extract → verify output)
  ☐ Verify: entities 350-700, avg confidence ≥0.72
  
Output: extract_entities.py updated + tested
```

**TASK 2: Rate Limiting Config** ⏱️ 30 min
```
Timeline: 10:30 AM - 11:00 AM

Checklist:
  ☐ Add to extraction script: 2-sec delays between URLs
  ☐ Add user-agent rotation (vary User-Agent header)
  ☐ Add robots.txt check (respect scraping guidelines)
  ☐ Test: Extract 5 URLs, verify no 429 rate-limit errors
  
Output: Extraction script handles rate limiting correctly
```

**TASK 3: Deduplication Script** ⏱️ 1.5 hours
```
Timeline: 11:00 AM - 12:30 PM

Checklist:
  ☐ Write scripts/deduplicate_chunks.py
  ☐ Implement Jaccard similarity (threshold 0.95)
  ☐ Test on 100-chunk sample
  ☐ Verify output: Merged chunks with preserved sources
  ☐ Test reduction ratio: Expect ~10% reduction
  
Output: deduplicate_chunks.py ready for May 20-21 use
```

#### Tech Lead Tasks (2 items, ~1 hour)

**TASK 4: Verify Grafana Setup** ⏱️ 15 min
```
Timeline: 9:00 AM - 9:15 AM

Checklist:
  ☐ Confirm Grafana running: http://localhost:30300
  ☐ Login with admin / asgard-grafana
  ☐ Verify Prometheus data source exists (default)
  ☐ Confirm AlertManager data source configured
  
Output: Grafana confirmed accessible + ready for dashboard creation
```

**TASK 5: Prometheus Metrics Design** ⏱️ 45 min
```
Timeline: 9:15 AM - 10:00 AM

Review VARDR_GRAFANA_S1_SETUP.md (created May 16)

Design Prometheus metrics needed:
  ☐ s1_chunks_extracted_total (Counter)
  ☐ s1_entities_found_total (Counter)
  ☐ s1_avg_confidence_gauge (Gauge)
  ☐ s1_current_phase_gauge (Gauge)
  ☐ s1_neo4j_relationships_total (Counter)
  ☐ s1_hit_rate_3_gauge (Gauge, populated May 22)

Design 6 Grafana panels (specs in setup doc):
  1. Chunks Extracted (time series)
  2. Entities Found (time series)
  3. Confidence Score (gauge)
  4. Hit Rate@3 (stat panel)
  5. Current Phase (text panel)
  6. Neo4j Relationships (stat panel)

Output: Dashboard design + metric list ready for creation
```

#### UX/UI Tasks (1 item, ~30 min)

**TASK 6: Search UI + Result Format Decision** ⏱️ 30 min
```
Timeline: 10:00 AM - 10:30 AM

Checklist:
  ☐ Confirm Option C (Hybrid) approach understood
  ☐ Review result display JSON format spec
  ☐ Plan minimal UI polish (domain selector, test buttons) — 2-3 hours
  ☐ Identify what can be done May 20-21 vs must slip to May 22
  
Output: UX/UI ready to start UI work, decision documented
```

---

### 🟢 AFTERNOON SESSION (1:00 PM - 5:00 PM)

#### Data Engineer Tasks (~1 hour total)

**TASK 7: Git Commit** ⏱️ 15 min
```
Timeline: 1:00 PM - 1:15 PM

Checklist:
  ☐ Stage files: git add scripts/extract_entities.py scripts/deduplicate_chunks.py
  ☐ Commit: git commit -m "Add entity extraction config + deduplication script"
  ☐ Verify: git log shows new commits
  
Output: Scripts committed to repository
```

**TASK 8: QA Briefing** ⏱️ 15 min
```
Timeline: 1:15 PM - 1:30 PM

Checklist:
  ☐ Brief QA on new configs (entity thresholds, rate limiting)
  ☐ Explain deduplication step (what to expect in S1.2)
  ☐ Review token validation rules (400-600 range)
  ☐ Answer questions on May 18 smoke test
  
Output: QA understands all new configurations
```

**TASK 9: Final Review** ⏱️ 30 min
```
Timeline: 1:30 PM - 2:00 PM

Checklist:
  ☐ Review all scripts one more time
  ☐ Verify no typos / syntax errors
  ☐ Confirm all config values are production-ready
  ☐ Test on smoke data one final time (optional but recommended)
  
Output: Data Engineer confidence: ✅ READY
```

#### Tech Lead Tasks (~2.5 hours total)

**TASK 10: Instrument Extraction Scripts** ⏱️ 1 hour
```
Timeline: 1:00 PM - 2:00 PM

Checklist:
  ☐ Add prometheus-client: pip install prometheus-client
  ☐ Update scripts/extract_entities.py:
    - Initialize metrics (Counter, Gauge objects)
    - Import prometheus_client
    - Start metrics HTTP server on port 8000
    - Increment counters during extraction
    - Update gauges for confidence/phase
  ☐ Test metrics endpoint: http://localhost:8000/metrics
  
Code example:
```
from prometheus_client import Counter, Gauge, start_http_server

chunks_counter = Counter('s1_chunks_extracted_total', 'Total chunks extracted')
entities_counter = Counter('s1_entities_found_total', 'Total entities found')
confidence_gauge = Gauge('s1_avg_confidence_gauge', 'Average confidence')
phase_gauge = Gauge('s1_current_phase_gauge', 'Current S1 phase')

start_http_server(8000)  # Expose metrics at :8000/metrics

# During extraction
chunks_counter.inc()
entities_counter.inc(5)
confidence_gauge.set(0.76)
phase_gauge.set(1)
```
  
Output: Extraction scripts export Prometheus metrics
```

**TASK 11: Create Grafana Dashboard** ⏱️ 1 hour
```
Timeline: 2:00 PM - 3:00 PM

Checklist:
  ☐ Login to Grafana: http://localhost:30300 (admin/asgard-grafana)
  ☐ Create new dashboard: "S1 Insurance Sprint Progress"
  ☐ Add 6 panels (see TASK 5 above for design):
    - Panel 1: Chunks Extracted (line graph, Query: s1_chunks_extracted_total)
    - Panel 2: Entities Found (line graph, Query: s1_entities_found_total)
    - Panel 3: Confidence Score (gauge, Query: s1_avg_confidence_gauge, min=0, max=1)
    - Panel 4: Hit Rate@3 (stat, Query: s1_hit_rate_3_gauge, unit=%)
    - Panel 5: Current Phase (stat, Query: s1_current_phase_gauge)
    - Panel 6: Neo4j Relationships (stat, Query: s1_neo4j_relationships_total)
  ☐ Set thresholds:
    - Confidence: Green ≥0.72, Red <0.72
    - Hit Rate: Green ≥75%, Red <75%
  ☐ Set Prometheus as data source (default)
  ☐ Save dashboard (auto-generates URL)
  
Output: Grafana dashboard created + all panels configured
```

**TASK 12: Verify + Share** ⏱️ 30 min
```
Timeline: 3:00 PM - 3:30 PM

Checklist:
  ☐ Verify all 6 panels are visible
  ☐ Test one query manually (run S1.1 on sample, watch metrics update)
  ☐ Get dashboard URL from Grafana
  ☐ Share URL in Slack: #insurance-s1-sprint
  ☐ Post access instructions: admin/asgard-grafana (if needed)
  
Output: Dashboard accessible to team + URL shared
```

#### UX/UI Tasks (~1.5 hours total)

**TASK 13: Define Result Format** ⏱️ 30 min
```
Timeline: 1:00 PM - 1:30 PM

Checklist:
  ☐ Review JSON schema from INTEGRATED_FEEDBACK_ACTION_PLAN.md
  ☐ Add JSON fields to test_results:
    - rank, title, snippet
    - relevance_score, relevance_stars
    - source_type, source_url
    - pii_clearance (score + status)
    - consolidation_confidence
    - chunk_id, reasoning
  ☐ Document: What makes a "good result" (relevance ≥0.75, PII=0, source valid)
  ☐ Share spec with QA
  
Output: Result format JSON defined + QA briefed
```

**TASK 14: UI Polish Planning** ⏱️ 1 hour
```
Timeline: 1:30 PM - 2:30 PM

Checklist:
  ☐ Plan minimal UI changes for Option C (hybrid):
    - Add domain selector dropdown: [Insurance ▼]
    - Add test query buttons (10 pre-populated)
    - Add result formatting (relevance score, PII badge visible)
  ☐ Estimate effort: 2-3 hours
  ☐ Identify critical vs nice-to-have
  ☐ Schedule: May 20-21 (non-blocking path, can slip to May 22 morning)
  
Output: UI work plan ready (defer actual implementation to May 20-21)
```

---

### 🏁 FINAL SIGN-OFF (3:30 PM - 5:00 PM)

#### All Team Members (30 min checkout)

**TASK 15: End-of-Day Verification** ⏱️ 30 min
```
Timeline: 3:30 PM - 4:00 PM

Checklist:

DATA ENGINEER:
  ☐ All configs tested on smoke data
  ☐ Scripts committed to git
  ☐ Ready for S1.1 extraction (May 18)
  Status: ✅ READY / ⚠️ NEEDS MORE TIME

TECH LEAD:
  ☐ Metrics instrumentation complete
  ☐ Grafana dashboard created (6 panels)
  ☐ Dashboard URL shared with team
  ☐ Metrics flowing (can test on sample data)
  Status: ✅ READY / ⚠️ NEEDS MORE TIME

UX/UI:
  ☐ Result format defined + JSON schema ready
  ☐ UI polish work plan prepared (May 20-21)
  ☐ Option C (hybrid) approach understood
  Status: ✅ READY / ⚠️ NEEDS MORE TIME

QA:
  ☐ Briefed on all new configs
  ☐ Test queries prepared (10 queries)
  ☐ Understands Hit Rate@3 success criteria (≥75%)
  Status: ✅ READY / ⚠️ NEEDS MORE TIME
```

**TASK 16: Team Standup (4:00-4:30 PM)** ⏱️ 30 min
```
Timeline: 4:00 PM - 4:30 PM

Agenda:
  1. Tech Lead reviews all completed items (10 min)
  2. Each owner confirms their section (5 min each)
  3. Any blockers / concerns raised (5 min)
  4. Final GO/NO-GO decision (5 min)
  
Output: Team consensus on readiness for May 18 kickoff
```

**TASK 17: Final Checklist Sign-Off (4:30-5:00 PM)** ⏱️ 30 min
```
Timeline: 4:30 PM - 5:00 PM

CONFIGS & SCRIPTS:
  ☐ Entity extraction config in scripts/extract_entities.py ✅
  ☐ Rate limiting added to extraction script ✅
  ☐ Token validation rules documented ✅
  ☐ Deduplication script written + tested ✅
  ☐ Prometheus metrics instrumented ✅
  ☐ All scripts committed to git ✅

DASHBOARDS & TOOLS:
  ☐ Grafana dashboard created (6 panels) ✅
  ☐ Prometheus data source configured ✅
  ☐ Metrics endpoint available (port 8000) ✅
  ☐ Dashboard URL shared with team ✅

DECISIONS DOCUMENTED:
  ☐ Search UI: Option C (Hybrid) documented ✅
  ☐ Metrics Dashboard: Grafana confirmed ✅
  ☐ Deduplication: Script ready May 17 EOD ✅
  ☐ All decisions locked in + no changes ✅

TEAM SIGN-OFF:
  ☐ Data Engineer: ✅ READY
  ☐ Tech Lead: ✅ READY
  ☐ UX/UI: ✅ READY
  ☐ QA: ✅ READY

═══════════════════════════════════════════════════════════════
              GO FOR MAY 18 KICKOFF ✅ 9:00 AM
═══════════════════════════════════════════════════════════════
```

---

## 📊 TIME BREAKDOWN (May 17)

```
DATA ENGINEER:        ~3.5 hours
  - Entity config:     1.5h
  - Rate limiting:     0.5h
  - Dedup script:      1.5h
  - Git + brief:       0.5h

TECH LEAD:            ~2.5 hours
  - Grafana verify:    0.25h
  - Metrics design:    0.75h
  - Instrumentation:   1h
  - Dashboard create:  0.5h

UX/UI:                ~1.5 hours
  - Format definition: 0.5h
  - UI planning:       1h

TEAM STANDUP:         ~1.5 hours
  - Verification:      0.5h
  - Standup:           0.5h
  - Sign-off:          0.5h

TOTAL PER PERSON:     3.5-4 hours
```

---

## ✅ SUCCESS CRITERIA (May 17, 5:00 PM)

| Item | Status | Owner |
|------|--------|-------|
| Entity extraction config added | ✅ | Data Engineer |
| Rate limiting configured | ✅ | Data Engineer |
| Token validation step ready | ✅ | Data Engineer |
| Deduplication script written + tested | ✅ | Data Engineer |
| Grafana dashboard created (6 panels) | ✅ | Tech Lead |
| Prometheus metrics instrumented | ✅ | Tech Lead |
| Result format JSON defined | ✅ | UX/UI |
| UI polish work plan ready | ✅ | UX/UI |
| All decisions locked in + documented | ✅ | Tech Lead |
| Team sign-offs complete | ✅ | All |

**If ALL above = ✅ → GO for May 18, 9:00 AM 🚀**

---

## 🎯 NEXT IMMEDIATE STEPS (Tonight/Tomorrow Morning)

1. **Share this document** with Data Engineer, Tech Lead, UX/UI, QA
2. **Review action items** - everyone knows their tasks
3. **9:00 AM May 17:** Tech Lead starts with Task 4 (Grafana verify)
4. **Data Engineer:** Starts with Task 1 (Entity config)
5. **UX/UI:** Starts with Task 6 (Search UI decision)
6. **Throughout day:** Coordinate via Slack #insurance-s1-sprint
7. **3:30-5:00 PM:** Final verification + sign-offs

---

**Owner:** Tech Lead  
**Approval:** Ready for Team Execution  
**Status:** ✅ ALL SYSTEMS GO
