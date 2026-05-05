"""Build cross-benchmark scoreboard from current eval_runs.
Gathers all post-2026-05-05 06:43Z runs (Sprint 36+ post-fix) keyed by (model, benchmark).
Computes per-benchmark metric using the dataset's scoring_fn."""
import json, urllib.request, subprocess

API = "http://localhost:30000"; TENANT = "asgard_medical"

# Get all runs from the last cross-bench (started 06:43+)
resp = json.loads(urllib.request.urlopen(
    urllib.request.Request(f"{API}/api/v1/eval/runs?limit=50", headers={"X-Tenant-Id": TENANT}),
    timeout=10).read())

# Get scoring_fn lookup
ds_resp = json.loads(urllib.request.urlopen(
    urllib.request.Request(f"{API}/api/v1/eval/benchmark-datasets", headers={"X-Tenant-Id": TENANT}),
    timeout=10).read())
ds_lookup = {d["id"]: d for d in ds_resp}

# Filter to current cross-bench (post-fix runs starting 2026-05-05T06:43)
crossbench = []
for r in resp:
    if not r['name'].startswith('local-bench__'): continue
    if (r.get('started_at','') or '') < '2026-05-05T06:43': continue
    cfg = json.loads(r.get('config') or '{}')
    bid = cfg.get('benchmark_dataset_id')
    if not bid: continue
    crossbench.append((r, bid))

print(f"Found {len(crossbench)} cross-benchmark runs (started ≥06:43Z)\n")

def n5(x): return max(0, (x-1)/4) if x and x > 0 else 0
def compute_score(r, ds):
    """Return (metric_pct, label) per scoring_fn."""
    sf = ds.get("scoring_fn", "healthbench_likert") if ds else "healthbench_likert"
    detail = json.loads(urllib.request.urlopen(
        urllib.request.Request(f"{API}/api/v1/eval/runs/{r['id']}", headers={"X-Tenant-Id": TENANT}),
        timeout=10).read())
    s = (detail.get("summaries") or [{}])[0]
    if sf == "healthbench_likert":
        acc = s.get('avg_accuracy') or 0
        comp = s.get('avg_completeness') or 0
        rel = s.get('avg_relevance') or 0
        safe = max(0, s.get('avg_safety_score') or 0)
        return ((n5(acc)+n5(comp)+n5(rel)+safe)/4)*100, "HBp%"
    elif sf in ("mcq_accuracy", "binary_yes_no"):
        # Use avg_accuracy as 0-5 → convert: top of 5 = 100%
        # But our judge gives Likert too. Until proper MCQ scoring runner exists,
        # treat avg_accuracy as a proxy: > 4 = correct, < 2 = wrong
        a = s.get('avg_accuracy') or 0
        # Likert→pct rough: (a-1)/4 * 100. For MCQ-style: judge scored "5" if right, "1" if wrong
        return n5(a) * 100, "≈Acc%"
    elif sf == "paper_rubric_pct":
        # Until rubric scoring is wired, use avg_accuracy/5*100
        a = s.get('avg_accuracy') or 0
        return n5(a) * 100, "≈Score%"
    return None, "?"

# Group by benchmark
by_bench = {}
for r, bid in crossbench:
    if bid not in by_bench: by_bench[bid] = []
    by_bench[bid].append(r)

# Order benchmarks
order = ["hb-pro-asgard-001","med-medqa-v1","med-medmcqa-v1","med-pubmedqa-v1",
         "med-healthbench-v1","med-medxpertqa-v1"]

print(f"{'Benchmark':24} {'scoring_fn':22} {'Model':30} {'Score':>7} {'Status':10}")
print("─" * 100)
for bid in order:
    runs = by_bench.get(bid, [])
    ds = ds_lookup.get(bid, {})
    sf = ds.get("scoring_fn", "?")
    if not runs:
        print(f"{bid:24} {sf:22} {'(no runs yet)':30} {'—':>7} pending")
        continue
    for r in runs:
        model = r['name'].replace('local-bench__','')[:28]
        if r['status'] == 'COMPLETED' and r['completed_combinations'] > 0:
            pct, label = compute_score(r, ds)
            print(f"{bid:24} {sf:22} {model:30} {pct:>6.1f}% {r['status']:10}")
        else:
            print(f"{bid:24} {sf:22} {model:30} {'—':>7} {r['status']} {r['completed_combinations']}/{r['total_combinations']}")
