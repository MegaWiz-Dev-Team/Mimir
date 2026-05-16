# Prometheus Metrics Integration Guide
## For S1 Extraction Scripts

**Purpose:** Add Prometheus metrics export to track S1 progress  
**Status:** Ready to integrate  
**Time to complete:** 30-45 minutes

---

## 📋 CHECKLIST (Follow in Order)

- [ ] Step 1: Install prometheus-client library
- [ ] Step 2: Review s1_prometheus_metrics.py module
- [ ] Step 3: Add imports to extract_entities.py
- [ ] Step 4: Initialize metrics in main()
- [ ] Step 5: Update extraction code to track metrics
- [ ] Step 6: Test metrics endpoint
- [ ] Step 7: Deploy dashboard
- [ ] Step 8: Verify metrics flow

---

## 📦 STEP 1: Install prometheus-client

```bash
pip install prometheus-client
```

Verify installation:
```bash
python -c "import prometheus_client; print(prometheus_client.__version__)"
```

Expected output: Version number (e.g., 0.15.0)

---

## 📖 STEP 2: Review Prometheus Metrics Module

Read `/Mimir/scripts/s1_prometheus_metrics.py`

Key components:
- `chunks_counter`: Count chunks extracted
- `entities_counter`: Count entities found (by type)
- `confidence_gauge`: Track average confidence
- `phase_gauge`: Track current S1 phase (1-4)
- `relationships_gauge`: Track Neo4j relationships
- `hit_rate_gauge`: Track Hit Rate@3 (May 22)

Helper functions:
- `init_metrics(port=8000)` — Start metrics server
- `increment_chunks(count)` — Add to chunks counter
- `increment_entities(count, entity_type)` — Add to entities counter
- `update_confidence(float)` — Update confidence gauge
- `set_extraction_phase(phase)` — Update phase gauge

---

## 🔗 STEP 3: Add Imports to extract_entities.py

At the top of `scripts/extract_entities.py`, add:

```python
# Prometheus metrics for S1 Sprint tracking
from s1_prometheus_metrics import (
    init_metrics,
    chunks_counter,
    entities_counter,
    confidence_gauge,
    phase_gauge,
    set_extraction_phase,
    increment_chunks,
    increment_entities,
    update_confidence,
)
```

---

## 🚀 STEP 4: Initialize Metrics in main()

In the `main()` function of `extract_entities.py`, add near the start:

```python
def main():
    # ... existing setup code ...
    
    # Initialize Prometheus metrics (port 8000)
    init_metrics(port=8000)
    print("📊 Metrics server started on http://localhost:8000/metrics")
    
    # ... rest of main() ...
```

This starts the HTTP server that Prometheus scrapes.

---

## 📊 STEP 5: Update Extraction Code to Track Metrics

### During chunk extraction:

```python
# Each time you extract a chunk
def extract_chunk(url, content):
    # ... your extraction logic ...
    
    chunk = {
        "chunk_id": chunk_id,
        "content": text,
        "token_count": token_count,
        "sources": [url],
    }
    
    # Track metrics
    increment_chunks(1)  # Increment counter by 1
    set_extraction_phase(1)  # Phase 1: Extraction
    
    return chunk
```

### During entity extraction:

```python
# Each time you extract entities
def extract_entities(chunk_text):
    # ... your entity extraction logic ...
    
    entities = []
    for entity in doc.ents:
        entity_obj = {
            "entity": entity.text,
            "type": entity.label_,
            "confidence": confidence_score,
        }
        entities.append(entity_obj)
    
    # Track metrics
    increment_entities(len(entities), entity_type="product")
    update_confidence(0.76)  # Update after each batch
    set_extraction_phase(3)  # Phase 3: Entities
    
    return entities
```

### Periodically update confidence:

```python
# After processing a batch of chunks
def process_batch(chunks):
    all_confidences = []
    
    for chunk in chunks:
        # ... processing ...
        confidences = [e['confidence'] for e in chunk_entities]
        all_confidences.extend(confidences)
    
    # Calculate and update average confidence
    if all_confidences:
        avg_confidence = sum(all_confidences) / len(all_confidences)
        update_confidence(avg_confidence)  # Updates gauge
```

---

## 🧪 STEP 6: Test Metrics Endpoint

Run the extraction script:

```bash
python scripts/extract_entities.py --test
```

In another terminal, check metrics:

```bash
curl http://localhost:8000/metrics | grep s1_
```

Expected output:
```
# HELP s1_chunks_extracted_total Total chunks successfully extracted
# TYPE s1_chunks_extracted_total counter
s1_chunks_extracted_total{phase="extraction"} 10.0

# HELP s1_entities_found_total Total entities extracted
# TYPE s1_entities_found_total counter
s1_entities_found_total{type="product"} 5.0

# HELP s1_avg_confidence_gauge Average confidence score (0.0-1.0)
# TYPE s1_avg_confidence_gauge gauge
s1_avg_confidence_gauge 0.76
```

---

## 📊 STEP 7: Deploy Dashboard

```bash
bash scripts/deploy_s1_grafana_dashboard.sh
```

Expected output:
```
🚀 Deploying S1 Dashboard to Grafana...
✅ Grafana is accessible
📤 Uploading dashboard...
✅ Dashboard deployed successfully!

Dashboard URL: http://localhost:30300/d/s1-insurance-progress/s1-insurance-sprint-progress
```

---

## ✅ STEP 8: Verify Metrics Flow

1. **Start extraction:**
   ```bash
   python scripts/extract_entities.py --input data/raw_urls.txt
   ```

2. **Open Grafana dashboard:**
   - URL: http://localhost:30300
   - Login: admin / asgard-grafana
   - Dashboard: "S1 Insurance Sprint Progress"

3. **Watch metrics update:**
   - Chunks Extracted: Should increase as extraction runs
   - Entities Found: Should increase as entities are extracted
   - Avg Confidence: Should stabilize around 0.72-0.80
   - Current Phase: Should show 1 (Extraction)

4. **Refresh interval:** Set to 30 seconds (automatic)

---

## 🔍 TROUBLESHOOTING

### Metrics not appearing in Grafana?

**Check 1:** Prometheus is scraping the metrics endpoint

```bash
kubectl port-forward -n asgard-monitoring svc/monitoring-kube-prometheus-prometheus 9090:9090 &

# Check if metrics are available
curl http://localhost:9090/api/v1/query?query=s1_chunks_extracted_total
```

**Check 2:** Extraction script is running and exporting metrics

```bash
# Check if metrics endpoint is responding
curl http://localhost:8000/metrics | head -20

# Should show s1_* metrics with values
```

**Check 3:** Prometheus scrape job is configured

In `prometheus.yml`:
```yaml
- job_name: 's1-extraction'
  static_configs:
    - targets: ['localhost:8000']
  scrape_interval: 30s
```

### Dashboard panels show "No data"?

**Solution:** 
1. Wait 2-3 minutes for Prometheus to scrape metrics
2. Refresh dashboard (press F5)
3. Check that extraction script is actively running (metrics should be increasing)

### Metrics endpoint not responding?

**Check:**
```bash
# Is the extraction script running?
ps aux | grep extract_entities.py

# Is port 8000 in use?
lsof -i :8000

# Is prometheus-client installed?
python -c "from prometheus_client import Counter; print('OK')"
```

---

## 📝 EXAMPLE: Complete Integration

Here's a minimal example of how to integrate metrics:

```python
#!/usr/bin/env python3
"""Extract entities with Prometheus metrics."""

from s1_prometheus_metrics import (
    init_metrics,
    increment_chunks,
    increment_entities,
    update_confidence,
)

def main():
    # Initialize metrics server
    init_metrics(port=8000)
    
    # Your extraction code
    for url in urls:
        chunk = extract_chunk(url)
        increment_chunks(1)  # Track
        
        entities = extract_entities(chunk["content"])
        increment_entities(len(entities), entity_type="product")
        update_confidence(0.75)
    
    print("✅ Extraction complete. Metrics available at :8000/metrics")

if __name__ == "__main__":
    main()
```

---

## 🎯 SUCCESS CRITERIA

After integration, verify:

```
✅ Metrics server starts without error
✅ curl http://localhost:8000/metrics returns data
✅ Prometheus scrapes metrics (check /api/v1/query)
✅ Grafana dashboard shows 6 panels
✅ Dashboard panels update every 30 seconds
✅ Metrics reflect actual extraction progress
```

---

## 📞 SUPPORT

If you encounter issues:

1. Check this guide (Section: Troubleshooting)
2. Review `s1_prometheus_metrics.py` comments
3. Check Grafana logs: `kubectl logs -n asgard-monitoring -f monitoring-grafana-*`
4. Check Prometheus: http://localhost:9090 (via port-forward)

---

**Owner:** Tech Lead  
**Timeline:** May 17, 1:00 PM - 2:00 PM  
**Status:** Ready to implement
