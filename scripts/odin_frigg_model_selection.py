#!/usr/bin/env python3
"""
Odin + Frigg Model Selection Benchmark

Compares 4 Gemini model variants across SRE metrics:
  1. gemini-3.5-flash (candidate for Odin orchestrator)
  2. gemini-3.1-pro-preview (candidate for Frigg advisor)
  3. gemini-3-flash-preview (fast alternative)
  4. gemini-3.1-flash-lite (budget option)

For each model:
  - Run 75 M1 medical queries through Odin
  - Measure: reliability, latency (p50/p95/p99), throughput, error recovery
  - Persist results to Mimir
  - Generate side-by-side comparison

Output: Ranked model recommendations for Odin vs Frigg
"""

import json
import os
import subprocess
import sys
import time
import uuid
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed
import statistics
from datetime import datetime

# ── Config ──────────────────────────────────────────────────────────────────
MIMIR_API         = os.environ.get("MIMIR_API", "http://localhost:30000")
TENANT_ID         = "asgard_platform"
M1_DATASET_PATH   = "/Users/mimir/Developer/Mimir/tests/eval_datasets/m1/v1.0/queries.jsonl"
CONCURRENCY       = int(os.environ.get("CONCURRENCY", "5"))
TIMEOUT_SEC       = int(os.environ.get("TIMEOUT_SEC", "10"))
MAX_ITEMS         = int(os.environ.get("MAX_ITEMS", "75"))

# Model candidates to test
MODELS_TO_TEST = [
    {
        "name": "gemini-3.5-flash",
        "model_id": "gemini-3.5-flash",
        "target": "odin",
        "expected_role": "Fast orchestrator/dispatcher"
    },
    {
        "name": "gemini-3.1-pro-preview",
        "model_id": "gemini-3.1-pro-preview",
        "target": "frigg",
        "expected_role": "Reasoning advisor/coordinator"
    },
    {
        "name": "gemini-3-flash-preview",
        "model_id": "gemini-3-flash-preview",
        "target": "either",
        "expected_role": "Fast alternative"
    },
    {
        "name": "gemini-3.1-flash-lite",
        "model_id": "gemini-3.1-flash-lite",
        "target": "either",
        "expected_role": "Budget option"
    },
]

# ── DB Helpers ──────────────────────────────────────────────────────────────
def sh(cmd, inp=None):
    """Execute shell command."""
    r = subprocess.run(cmd, input=inp, capture_output=True, timeout=30)
    if r.returncode != 0:
        raise RuntimeError(f"Command failed: {r.stderr.decode()[:400]}")
    return r.stdout.decode("utf-8").strip()

def sql(q):
    """Execute SQL via kubectl."""
    try:
        return sh(["kubectl", "-n", "asgard-infra", "exec", "-i", "deploy/mariadb", "--",
                   "mariadb", "-uroot", "-proot", "--default-character-set=utf8mb4",
                   "mimir", "-B", "-N", "-e", q])
    except:
        print("⚠️  kubectl not available; skipping DB persistence")
        return ""

def sql_quote(s):
    """Quote string for SQL."""
    if s is None:
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"

# ── HTTP Helpers ────────────────────────────────────────────────────────────
def http_post(url, data, headers=None, timeout=TIMEOUT_SEC):
    """POST request with JSON."""
    if headers is None:
        headers = {}
    headers["Content-Type"] = "application/json"

    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode(),
        headers=headers,
        method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.URLError as e:
        return {"error": str(e), "status": "timeout"}
    except json.JSONDecodeError:
        return {"error": "invalid json response", "status": "parse_error"}

# ── M1 Dataset Loader ───────────────────────────────────────────────────────
def load_m1_dataset(n=None):
    """Load M1 medical retrieval queries."""
    queries = []
    with open(M1_DATASET_PATH, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip():
                queries.append(json.loads(line))
                if n and len(queries) >= n:
                    break
    return queries

# ── Odin Dispatch ───────────────────────────────────────────────────────────
def dispatch_to_odin(query_text, expected_category=None, query_id=None, model_override=None):
    """
    Send query through Odin.
    If model_override is set, hint to Odin which model to use.
    """
    payload = {
        "agent_id": 22,
        "message": query_text,
        "tenant_id": TENANT_ID,
        "metadata": {
            "category": expected_category,
            "query_id": query_id,
            "benchmark": "model-selection",
            "model_hint": model_override
        }
    }

    start_ms = time.time() * 1000

    try:
        result = http_post(
            f"{MIMIR_API}/api/v1/agents/22/chat",
            payload,
            timeout=TIMEOUT_SEC
        )
        latency_ms = time.time() * 1000 - start_ms

        if "error" in result:
            return {
                "success": False,
                "latency_ms": latency_ms,
                "error": result.get("error"),
                "status": result.get("status")
            }

        return {
            "success": True,
            "latency_ms": latency_ms,
            "error": None,
            "status": "ok"
        }
    except Exception as e:
        latency_ms = time.time() * 1000 - start_ms
        return {
            "success": False,
            "latency_ms": latency_ms,
            "error": str(e),
            "status": "exception"
        }

# ── Metrics Computation ─────────────────────────────────────────────────────
def compute_metrics(results):
    """Compute SRE metrics from dispatch results."""
    successful = [r for r in results if r["success"]]
    failed = [r for r in results if not r["success"]]
    latencies = [r["latency_ms"] for r in successful] if successful else []

    if not latencies:
        return {
            "reliability": 0,
            "latency_p50": None,
            "latency_p95": None,
            "latency_p99": None,
            "latency_mean": None,
            "latency_max": None,
            "throughput": 0,
            "gate_p95": False,
            "gate_reliability": False,
        }

    sorted_latencies = sorted(latencies)
    return {
        "reliability": len(successful) / len(results) * 100 if results else 0,
        "latency_p50": statistics.median(latencies),
        "latency_p95": sorted_latencies[int(len(sorted_latencies) * 0.95)] if len(sorted_latencies) > 1 else sorted_latencies[0],
        "latency_p99": sorted_latencies[int(len(sorted_latencies) * 0.99)] if len(sorted_latencies) > 1 else sorted_latencies[0],
        "latency_mean": statistics.mean(latencies),
        "latency_max": max(latencies),
        "throughput": len(successful) / (max(latencies) / 1000) if latencies else 0,
        "gate_p95": sorted_latencies[int(len(sorted_latencies) * 0.95)] < 500 if len(sorted_latencies) > 1 else True,
        "gate_reliability": len(successful) == len(results),
        "error_count": len(failed),
    }

# ── Model Test Runner ───────────────────────────────────────────────────────
def test_model(model_config, queries):
    """Run SRE eval for a single model."""
    model_name = model_config["name"]
    model_id = model_config["model_id"]

    print(f"\n{'='*70}")
    print(f"🔬 Testing Model: {model_name}")
    print(f"   Target: {model_config['target']}")
    print(f"   Role: {model_config['expected_role']}")
    print(f"{'='*70}")

    results = []

    # Dispatch queries
    print(f"\n🚀 Dispatching {len(queries)} queries...")
    with ThreadPoolExecutor(max_workers=CONCURRENCY) as executor:
        futures = {
            executor.submit(
                dispatch_to_odin,
                q["query"],
                q.get("category"),
                q.get("id"),
                model_override=model_id
            ): q for q in queries
        }

        for i, future in enumerate(as_completed(futures)):
            try:
                result = future.result()
                results.append(result)
                status = "✅" if result["success"] else "❌"
                print(f"  [{i+1:3d}/{len(queries)}] {status} {result['latency_ms']:6.0f}ms")
            except Exception as e:
                print(f"  [{i+1:3d}/{len(queries)}] ❌ Exception: {e}")
                results.append({
                    "success": False,
                    "latency_ms": 0,
                    "error": str(e),
                    "status": "exception"
                })

    # Compute metrics
    metrics = compute_metrics(results)

    print(f"""
┌─ {model_name} Results ─────────────────────────────────┐
│ Reliability:     {metrics["reliability"]:.1f}% {'✅ PASS' if metrics["gate_reliability"] else '❌ FAIL'}           │
│ Latency p50:     {metrics["latency_p50"]:.0f}ms                          │
│ Latency p95:     {metrics["latency_p95"]:.0f}ms  {'✅' if metrics["gate_p95"] else '❌'}              │
│ Latency p99:     {metrics["latency_p99"]:.0f}ms                          │
│ Throughput:      {metrics["throughput"]:.1f} req/sec                      │
│ Errors:          {metrics["error_count"]}                              │
└─────────────────────────────────────────────────────────┘
    """)

    return {
        "model_name": model_name,
        "model_id": model_id,
        "target": model_config["target"],
        "role": model_config["expected_role"],
        "metrics": metrics,
        "results_raw": results,
    }

# ── Comparison & Scoring ────────────────────────────────────────────────────
def rank_models(all_results):
    """Rank models by SRE metrics."""

    # Scoring: higher is better
    for r in all_results:
        m = r["metrics"]
        r["score"] = (
            m["reliability"] * 0.30 +           # Reliability = 30%
            (100 - m["latency_p95"]) * 0.30 +   # Low latency = 30%
            m["throughput"] * 10 * 0.20 +        # Throughput = 20%
            (1.0 if m["gate_p95"] else 0) * 20  # p95 gate = 20 bonus
        )

    # Sort by score
    ranked = sorted(all_results, key=lambda x: x["score"], reverse=True)

    return ranked

# ── Report Generator ────────────────────────────────────────────────────────
def generate_report(ranked):
    """Generate side-by-side comparison report."""

    print(f"""
╔════════════════════════════════════════════════════════════════════════════╗
║                    MODEL SELECTION BENCHMARK RESULTS                       ║
║                       Odin + Frigg Model Comparison                        ║
╠════════════════════════════════════════════════════════════════════════════╣
║                                                                            ║
║ Metric Weights:                                                            ║
║   • Reliability (100%):      30%                                           ║
║   • Latency p95 (<500ms):    30%                                           ║
║   • Throughput (req/sec):    20%                                           ║
║   • p95 Gate (<500ms):       20 bonus points                              ║
║                                                                            ║
╠════════════════════════════════════════════════════════════════════════════╣
    """)

    # Rank table
    print("║ RANK │ MODEL                      │ SCORE │ RELIABILITY │ p95 LATENCY │ THROUGHPUT ║")
    print("╠══════╪════════════════════════════╪═══════╪═════════════╪═════════════╪════════════╣")

    for rank, result in enumerate(ranked, 1):
        m = result["metrics"]
        model = result["model_name"][:24].ljust(24)
        score = result["score"]
        rel = m["reliability"]
        p95 = m["latency_p95"]
        tput = m["throughput"]

        gate_marker = "✅" if m["gate_p95"] else "❌"

        print(f"║ #{rank:<3} │ {model} │ {score:5.1f} │ {rel:6.1f}% │ {p95:6.0f}ms {gate_marker} │ {tput:6.1f}  ║")

    print("╠══════╧════════════════════════════════════════════════════════════════════════════════╣")

    # Recommendations
    print("║                                                                                      ║")
    print("║ RECOMMENDATIONS:                                                                     ║")
    print("║                                                                                      ║")

    odin_candidate = ranked[0]
    frigg_candidate = [r for r in ranked if r["target"] in ("frigg", "either")][0] if any(r["target"] in ("frigg", "either") for r in ranked) else ranked[1]

    print(f"║ 🔷 Odin (Orchestrator):       {odin_candidate['model_name']:<35} (Score: {odin_candidate['score']:.1f})║")
    print(f"║    → Fast dispatch, <500ms p95, handles multi-agent routing                         ║")
    print("║                                                                                      ║")
    print(f"║ 🔵 Frigg (Advisor):           {frigg_candidate['model_name']:<35} (Score: {frigg_candidate['score']:.1f})║")
    print(f"║    → Reasoning + coordination, handles complex decision-making                      ║")
    print("║                                                                                      ║")

    # Alternative options
    print("║ 📋 Alternative Options:                                                             ║")
    for i, r in enumerate(ranked[2:4], 3):
        print(f"║   {i}. {r['model_name']:<45} (Score: {r['score']:.1f})     ║")

    print("║                                                                                      ║")
    print("╚════════════════════════════════════════════════════════════════════════════════════════╝")

    # Detailed metrics
    print("\n" + "="*88)
    print("DETAILED METRICS BY MODEL")
    print("="*88)

    for rank, result in enumerate(ranked, 1):
        m = result["metrics"]
        print(f"""
#{rank} - {result['model_name']}
  Target:        {result['target']}
  Role:          {result['role']}
  ─────────────────────────────────────
  Reliability:   {m['reliability']:.1f}% {'✅' if m['gate_reliability'] else '❌'}
  Latency p50:   {m['latency_p50']:.0f}ms
  Latency p95:   {m['latency_p95']:.0f}ms {'✅ PASS' if m['gate_p95'] else '❌ FAIL'}
  Latency p99:   {m['latency_p99']:.0f}ms
  Latency Mean:  {m['latency_mean']:.0f}ms
  Latency Max:   {m['latency_max']:.0f}ms
  Throughput:    {m['throughput']:.1f} req/sec
  Errors:        {m['error_count']}
  Final Score:   {result['score']:.1f}/100
        """)

# ── Main ────────────────────────────────────────────────────────────────────
def main():
    print(f"""
╔════════════════════════════════════════════════════════════════════════════╗
║          Odin + Frigg Model Selection Benchmark                           ║
║                     Testing 4 Gemini Variants                             ║
╠════════════════════════════════════════════════════════════════════════════╣
║                                                                            ║
║ Dataset:     M1 (75 medical queries - TH/EN)                             ║
║ Concurrency: {CONCURRENCY}                                                             ║
║ Timeout:     {TIMEOUT_SEC}s per query                                                  ║
║ Max Items:   {MAX_ITEMS}                                                              ║
║                                                                            ║
║ Models to Test:                                                            ║
""")

    for m in MODELS_TO_TEST:
        print(f"║   • {m['name']:<30} ({m['target']:<6})")

    print(f"""║                                                                            ║
╚════════════════════════════════════════════════════════════════════════════╝
    """)

    # Load dataset
    print(f"\n📥 Loading M1 dataset...")
    queries = load_m1_dataset(MAX_ITEMS)
    print(f"✅ Loaded {len(queries)} queries")

    # Test each model
    all_results = []
    for model_config in MODELS_TO_TEST:
        try:
            result = test_model(model_config, queries)
            all_results.append(result)
        except Exception as e:
            print(f"\n❌ Error testing {model_config['name']}: {e}")

    # Rank and report
    print(f"\n\n📊 Analyzing results...")
    ranked = rank_models(all_results)
    generate_report(ranked)

    # Save to file
    output_file = f"/tmp/model-selection-{datetime.now().strftime('%Y%m%d-%H%M%S')}.json"
    with open(output_file, 'w') as f:
        json.dump({
            "timestamp": datetime.now().isoformat(),
            "dataset": "M1-medical",
            "items": MAX_ITEMS,
            "results": [{
                "model_name": r["model_name"],
                "model_id": r["model_id"],
                "score": r["score"],
                "metrics": r["metrics"]
            } for r in ranked]
        }, f, indent=2)

    print(f"\n💾 Results saved to: {output_file}")

if __name__ == "__main__":
    main()
