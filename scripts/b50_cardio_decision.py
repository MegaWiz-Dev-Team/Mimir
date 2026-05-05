#!/usr/bin/env python3
"""B-50 — Cardio model decision: gemma-4-26b vs flash-lite head-to-head.

Sprint 38 PoC put cardio specialist on gemma-4-26b based on cross-benchmark
finding (gemma stronger on RAG-heavy synthesis). But gemma local is +27s
slower than flash-lite cloud. Test on cardio-specific items:
  - if gemma wins ≥3pp HBp on cardio subset → keep gemma (latency justified)
  - if neutral or worse → switch eir-cardio to flash-lite (uniform speed)

This script:
  1. Updates eir-cardio agent_config.model_id sequentially to each candidate
  2. Triggers eval on hb-pro-asgard-001 with locked items
  3. Compares HBp%, latency, cost
  4. Recommends decision

Note: This isn't a true cardio-only test — hb-pro-asgard-001 has mixed
specialties. Better test would be cardio-tagged subset, but we don't have
specialty tags on items yet. As proxy: any specialty improvement on broad
hb-pro should signal benefit on cardio-only too.

Usage:
  python3 scripts/b50_cardio_decision.py
"""
import json
import sys
import time
import urllib.request

API = "http://localhost:30000"
TENANT = "asgard_medical"
EIR_CARDIO_ID = 29  # from sprint38 migration
LOCKED_RUN = "4682363c-af88-42a8-b170-d1cf9131a3e5"  # n=20 reference

CANDIDATES = [
    ("gemma-4-26b (current)", "mlx-community/gemma-4-26b-a4b-it-4bit", "heimdall"),
    ("flash-lite (alternative)", "gemini-3.1-flash-lite-preview", "google"),
]


def http(method, path, body=None):
    headers = {"X-Tenant-Id": TENANT}
    data = json.dumps(body).encode() if body else None
    if data: headers["Content-Type"] = "application/json"
    req = urllib.request.Request(f"{API}{path}", data=data, headers=headers, method=method)
    return json.loads(urllib.request.urlopen(req, timeout=30).read())


def n5(x): return max(0, (x - 1) / 4) if x and x > 0 else 0
def hbp(s):
    return ((n5(s.get('avg_accuracy') or 0) + n5(s.get('avg_completeness') or 0)
            + n5(s.get('avg_relevance') or 0) + max(0, s.get('avg_safety_score') or 0)) / 4) * 100


def main():
    # Pull locked items from reference run
    ref = http("GET", f"/api/v1/eval/runs/{LOCKED_RUN}")
    cfg = json.loads(ref["run"]["config"])
    locked = cfg["item_ids"]
    print(f"Using {len(locked)} locked items from {cfg['benchmark_dataset_id']}\n")

    results = []
    for label, model_id, provider in CANDIDATES:
        print(f"[{label}] switching eir-cardio → {model_id}")
        http("PUT", f"/api/v1/agents/{EIR_CARDIO_ID}",
             {"model_id": model_id, "provider": provider})

        payload = {
            "tenant_id": TENANT, "agent_names": ["eir-cardio"], "model_ids": [model_id],
            "question_limit": 20, "benchmark_dataset_id": "hb-pro-asgard-001",
            "run_name": f"b50-cardio-decision__{label.split()[0]}",
            "hypothesis": f"B-50 cardio model: {label}",
            "variable_under_test": "cardio_model_id",
            "expected_change": "gemma cardio justifies +27s latency only if ≥3pp HBp",
            "replicates": 1, "item_ids": locked,
        }
        run = http("POST", "/api/v1/eval/runs", payload)
        rid = run["run_id"]
        print(f"  triggered run_id={rid[:8]} — polling...")
        for _ in range(180):
            time.sleep(5)
            d = http("GET", f"/api/v1/eval/runs/{rid}")
            if d["run"]["status"] in ("COMPLETED", "FAILED", "CANCELLED"):
                break
        sm = (d.get("summaries") or [{}])[0]
        results.append({
            "label": label, "rid": rid[:8],
            "hbp": hbp(sm), "acc": sm.get('avg_accuracy', 0),
            "lat_ms": sm.get('avg_latency_ms', 0),
            "cost": d["run"].get('total_cost_usd', 0) or 0,
        })
        print(f"  → HBp%={results[-1]['hbp']:.1f}%  lat={results[-1]['lat_ms']/1000:.1f}s  cost=${results[-1]['cost']:.4f}\n")

    # Compare
    g, f = results[0], results[1]
    delta_hbp = g["hbp"] - f["hbp"]
    delta_lat = (g["lat_ms"] - f["lat_ms"]) / 1000
    print("═" * 64)
    print(" B-50 Cardio Model Decision")
    print("═" * 64)
    print(f"  gemma cardio:      HBp%={g['hbp']:.1f}%  lat={g['lat_ms']/1000:.1f}s  cost=${g['cost']:.4f}")
    print(f"  flash-lite cardio: HBp%={f['hbp']:.1f}%  lat={f['lat_ms']/1000:.1f}s  cost=${f['cost']:.4f}")
    print(f"  Δ HBp:    {delta_hbp:+.1f}pp (gemma vs flash-lite)")
    print(f"  Δ Latency: +{delta_lat:.1f}s (gemma slower)")
    if delta_hbp >= 3:
        rec = "✅ KEEP gemma cardio — quality lift justifies latency cost"
    elif delta_hbp >= -1:
        rec = "⚠️  Switch to flash-lite — uniform speed, no quality penalty"
    else:
        rec = "❌ Switch to flash-lite IMMEDIATELY — gemma worse AND slower"
    print(f"\n  Recommendation: {rec}")

    # Restore agent to flash-lite by default if recommended
    if delta_hbp < 3:
        print("  Reverting eir-cardio → flash-lite (no model_id change vs starting state if already flash-lite)")
        http("PUT", f"/api/v1/agents/{EIR_CARDIO_ID}",
             {"model_id": "gemini-3.1-flash-lite-preview", "provider": "google"})
    else:
        print("  Keeping eir-cardio on gemma-4-26b")


if __name__ == "__main__":
    sys.exit(main() or 0)
