# Vardr Grafana S1 Sprint Setup
## Status: ✅ Prometheus + Grafana Live

**Date:** May 16, 2026  
**Status:** Grafana + Prometheus scaled up and running  
**Next:** Configure S1 metrics dashboard

---

## 🎯 Current Infrastructure Status

### Prometheus (Metrics Database)
- ✅ **Service:** `monitoring-kube-prometheus-prometheus` (asgard-monitoring namespace)
- ✅ **Port:** 9090
- ✅ **Health:** Healthy
- ✅ **Data Sources:** AlertManager, Kubernetes metrics, Node Exporter
- ✅ **Default Data Source in Grafana:** YES

### Grafana (Visualization)
- ✅ **Service:** `monitoring-grafana` (asgard-monitoring namespace)
- ✅ **Port:** 30300 (NodePort)
- ✅ **URL:** http://localhost:30300
- ✅ **Credentials:** admin / asgard-grafana
- ✅ **Status:** Running (2/3 ready, web accessible)

### Existing Dashboards (Pre-built)
- ✅ 🏰 **Asgard Platform Overview** (custom)
- ✅ Kubernetes dashboards (API Server, Compute Resources, Networking, etc.)
- ✅ Node Exporter dashboards (Nodes, USE Method)
- ✅ Prometheus Overview
- ✅ Alertmanager Overview

---

## 📊 S1 METRICS REQUIRED (Not yet exported)

For S1 Insurance Sprint daily tracking, we need to export these metrics:

### Custom Metrics (Need to add to extraction scripts)

```prometheus
# Extraction Phase (S1.1)
s1_chunks_extracted_total        COUNTER  # Total chunks successfully extracted
s1_extraction_duration_seconds   GAUGE    # Time spent on extraction (phase)
s1_extraction_errors_total       COUNTER  # Failed extraction attempts

# Chunking Phase (S1.2)
s1_chunks_processed_total        COUNTER  # Chunks after chunking
s1_chunks_deduplicated_total     COUNTER  # Unique chunks after dedup
s1_chunking_duration_seconds     GAUGE    # Time spent on chunking

# Entity Extraction Phase (S1.3)
s1_entities_found_total          COUNTER  # Total entities extracted
s1_avg_confidence_gauge          GAUGE    # Average confidence score (0.0-1.0)
s1_entities_by_type              GAUGE    # Entities per type (product, coverage, etc.)

# Neo4j Ingestion Phase (S1.4)
s1_neo4j_relationships_total     COUNTER  # Total relationships created
s1_neo4j_nodes_total             COUNTER  # Total nodes created

# Decision Gates
s1_hit_rate_3_gauge              GAUGE    # Hit Rate@3 (May 22 measurement: 0-100)
s1_current_phase_gauge           GAUGE    # Current phase (1=extract, 2=chunk, 3=entities, 4=embed)
s1_embedding_tokens_total        COUNTER  # Total tokens embedded into Qdrant
```

### How to Expose These Metrics

**Option A: Prometheus Python Client (RECOMMENDED)**
```python
# Add to extraction scripts
from prometheus_client import Counter, Gauge, start_http_server
import time

# Define metrics
chunks_counter = Counter('s1_chunks_extracted_total', 'Total chunks extracted')
entities_counter = Counter('s1_entities_found_total', 'Total entities found')
confidence_gauge = Gauge('s1_avg_confidence_gauge', 'Average entity confidence')
phase_gauge = Gauge('s1_current_phase_gauge', 'Current S1 phase (1-4)')

# Start Prometheus HTTP server (port 8000)
start_http_server(8000)

# During extraction
chunks_counter.inc()
entities_counter.inc(5)  # Found 5 entities
confidence_gauge.set(0.76)
phase_gauge.set(1)  # Phase 1: Extraction

# Your extraction code...
```

**Option B: Write Metrics to JSON File (Simpler)**
```python
# Write metrics to file, Prometheus scrapes it
import json
from datetime import datetime

metrics = {
    "timestamp": datetime.now().isoformat(),
    "s1_chunks_extracted_total": 450,
    "s1_entities_found_total": 320,
    "s1_avg_confidence_gauge": 0.76,
    "s1_current_phase_gauge": 1,
}

with open("metrics.json", "w") as f:
    json.dump(metrics, f)
```

---

## 🎨 S1 GRAFANA DASHBOARD DESIGN

### Dashboard: "S1 Insurance Sprint Progress"

**6 Panels:**

#### Panel 1: Chunks Extracted (Time Series)
```
Title: Chunks Extracted
Query: s1_chunks_extracted_total
Type: Graph (line chart)
X-axis: Time (May 18-27)
Y-axis: Cumulative count
Target: 950 by May 24
```

#### Panel 2: Entities Found (Time Series)
```
Title: Entities Discovered
Query: s1_entities_found_total
Type: Graph
Target: 400-500 entities by May 24
Threshold: 350-700 (acceptable range)
```

#### Panel 3: Confidence Score (Gauge)
```
Title: Average Entity Confidence
Query: s1_avg_confidence_gauge
Type: Gauge Panel
Min: 0.0, Max: 1.0
Green: ≥0.72 (success)
Red: <0.72 (needs review)
```

#### Panel 4: Hit Rate@3 (Stat Panel - Big Number)
```
Title: Hit Rate@3 (May 22 Decision)
Query: s1_hit_rate_3_gauge
Type: Stat (big number)
Unit: %
Green: ≥75% (GO)
Red: <75% (NO-GO)
```

#### Panel 5: Current Phase (Text Panel)
```
Title: Current S1 Phase
Query: s1_current_phase_gauge
Type: Stat
Values: 1=Extract, 2=Chunk, 3=Entities, 4=Embed
```

#### Panel 6: Neo4j Relationships (Stat Panel)
```
Title: Neo4j Relationships Created
Query: s1_neo4j_relationships_total
Type: Stat
Target: 1000+ by May 25
```

---

## 📋 IMPLEMENTATION PLAN (May 17-18)

### Step 1: Instrument Extraction Scripts (May 17, 3:00-4:00 PM)
- Add Prometheus client library: `pip install prometheus-client`
- Update `scripts/extract_entities.py`:
  - Initialize Counter/Gauge metrics
  - Export metrics at port 8000
  - Increment counters during extraction
  - Update gauges for confidence/phase

### Step 2: Configure Prometheus Scrape Job (May 17, 4:00-4:30 PM)
- Update Prometheus ConfigMap to scrape from extraction pod
- Add scrape job:
  ```yaml
  - job_name: 's1-extraction'
    static_configs:
      - targets: ['localhost:8000']
    scrape_interval: 30s
  ```

### Step 3: Create Grafana Dashboard (May 17, 4:30-5:00 PM)
- Login to Grafana: http://localhost:30300
- Create new dashboard: "S1 Insurance Sprint Progress"
- Add 6 panels (see design above)
- Set Prometheus as data source (default)
- Configure panel queries and thresholds
- Save dashboard

### Step 4: Verify Metrics Flow (May 18, 9:30 AM)
- Start extraction (S1.1)
- Monitor Prometheus: http://localhost:9090
- Verify metrics appearing in Grafana dashboard
- Share dashboard URL with team

---

## 🔗 ACCESS URLS

```
Grafana Dashboard:  http://localhost:30300
Prometheus:         http://localhost:9090 (via port-forward)
Metrics Endpoint:   http://localhost:8000/metrics (from extraction pod)
```

---

## 📝 CHECKLIST (May 17 EOD)

- [ ] Prometheus client added to extraction script
- [ ] Metrics initialized (Counter, Gauge objects)
- [ ] Metrics updated during extraction (increments, set values)
- [ ] Prometheus scrape job configured
- [ ] Grafana dashboard created with 6 panels
- [ ] All queries tested and returning data
- [ ] Threshold colors configured (green/red)
- [ ] Dashboard URL shared with team in Slack
- [ ] All team members can access: http://localhost:30300

---

## 🚀 READY TO IMPLEMENT?

**All prerequisites met:**
- ✅ Prometheus is running
- ✅ Grafana is accessible
- ✅ Dashboards can be created
- ✅ Metrics scraping is configured

**Next:** Instrument extraction scripts with Prometheus metrics

**Time to complete:** 1-1.5 hours (Step 1-4 above)

---

**Owner:** Tech Lead  
**Deadline:** May 17, 5:00 PM  
**Status:** READY TO START
