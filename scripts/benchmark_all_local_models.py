#!/usr/bin/env python3
"""
Benchmark a list of models against the same HealthBench items.

⚠️ Reality check (MLX backend limitation):
   Heimdall's MLX subprocess can only host ONE local model at a time.
   Switching local models means kill+reload (5-10 min, requires free RAM).
   So we benchmark only models that work without re-loading:
     - All cloud models (Gemini 2.5/3 variants — no local resource)
     - Whichever ONE MLX model is currently active in Heimdall
     - Other MLX models require manual MLX-server restart first

For each model in MODELS:
  1. UPDATE agent_configs.model_id + provider
  2. POST /api/v1/eval/runs (with item_ids locked from first run)
  3. Poll → capture summary

Env (auto-discoverable):
  MODELS               comma-separated 'model_id:provider' pairs
                       default: 'gemini-3-flash-preview:google,
                                 gemini-2.5-flash:google,
                                 <currently-loaded MLX>:heimdall'
  API_BASE             default http://localhost:30000
  HEIMDALL_BASE        default http://localhost:8080
  HEIMDALL_KEY         optional (for probing active MLX model)
  TENANT_ID            auto-discovered
  AGENT_NAME           default 'eir'
  BENCHMARK_ID         auto-discovered (first healthbench)
  ITEMS_PER_RUN        default 3
  TIMEOUT_PER_RUN_SEC  default 600
  RESTORE_MODEL        default 1
"""
import json
import os
import re
import sys
import time
import urllib.request
import urllib.error

API_BASE       = os.environ.get("API_BASE", "http://localhost:30000")
HEIMDALL_BASE  = os.environ.get("HEIMDALL_BASE", "http://localhost:8080")
HEIMDALL_KEY   = os.environ.get("HEIMDALL_KEY", "hml-mimir-ffcad30d20ac3b2cbc0643c0874b738517edb4c6ec6c49698e7518ffad5123ff")
TENANT_ID      = os.environ.get("TENANT_ID", "")
AGENT_NAME     = os.environ.get("AGENT_NAME", "eir")
BENCHMARK_ID   = os.environ.get("BENCHMARK_ID", "")
ITEMS_PER_RUN  = int(os.environ.get("ITEMS_PER_RUN", "3"))
TIMEOUT_SEC    = int(os.environ.get("TIMEOUT_PER_RUN_SEC", "600"))
RESTORE_MODEL  = os.environ.get("RESTORE_MODEL", "1") == "1"
MODELS_OVERRIDE = os.environ.get("MODELS", "")  # 'mid:prov,mid:prov'


# ─── HTTP helpers ─────────────────────────────────────────────────────────────

def http(url, method="GET", body=None, headers=None, timeout=30):
    data = json.dumps(body).encode() if body is not None else None
    h = headers or {}
    if data is not None:
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            text = r.read()
            try: return r.status, json.loads(text), None
            except Exception: return r.status, text.decode("utf-8", errors="replace"), None
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", errors="replace"), str(e)
    except Exception as e:
        return 0, "", str(e)


def auth_headers(extra=None):
    h = {"X-Tenant-Id": TENANT_ID}
    if extra: h.update(extra)
    return h


# ─── Discovery ────────────────────────────────────────────────────────────────

def estimate_size_gb(model_id: str) -> float:
    """Rough param-count heuristic from model name. Returns memory estimate at 4-bit."""
    m = re.search(r'(\d+(?:\.\d+)?)[Bb](?:[-_]|$)', model_id)
    if not m: return 0
    params_b = float(m.group(1))
    return params_b * 0.625  # ~5 bits/param at 4bit (with overhead)


def free_ram_gb() -> float:
    """Estimate macOS free + inactive RAM (which the kernel can reclaim) in GB."""
    import subprocess
    try:
        out = subprocess.check_output(["vm_stat"], text=True, timeout=5)
    except Exception:
        return 0.0
    page_size = 16384  # macOS arm64 default
    pages = {"free": 0, "inactive": 0, "speculative": 0}
    for line in out.splitlines():
        for k in pages:
            if line.startswith(f"Pages {k}:"):
                # "Pages free:                               27645."
                pages[k] = int(line.rsplit(":", 1)[1].strip().rstrip("."))
    reclaimable_bytes = (pages["free"] + pages["inactive"] + pages["speculative"]) * page_size
    return reclaimable_bytes / (1024 ** 3)


# ─── MLX swap helpers ────────────────────────────────────────────────────────

def warmup_mlx_model(model_id: str, timeout_sec: int = 600) -> tuple[bool, str]:
    """Send a tiny prompt to Heimdall to trigger transparent hotswap, then wait.

    Heimdall's gateway proxy.rs detects model_id != active_model and runs hotswap.sh
    automatically. This single warmup call eats the swap latency + first-token JIT
    compile, so the subsequent eval doesn't see a cold start.
    """
    print(f"   ⏳ warmup MLX (≤{timeout_sec}s, free RAM={free_ram_gb():.1f}GB)...")
    body = {
        "model": model_id,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 4,
        "temperature": 0,
    }
    headers = {"Authorization": f"Bearer {HEIMDALL_KEY}"}
    t0 = time.time()
    code, resp, err = http(
        f"{HEIMDALL_BASE}/v1/chat/completions",
        method="POST", body=body, headers=headers, timeout=timeout_sec)
    elapsed = time.time() - t0
    if code == 200:
        active = detect_active_mlx_model()
        ok = (active == model_id)
        if ok:
            print(f"   ✅ warmed in {elapsed:.0f}s (active={active})")
        else:
            print(f"   ⚠️  warmup returned 200 but active={active!r} != requested {model_id!r}")
        return ok, ""
    return False, f"HTTP {code}: {str(resp)[:200]} {err or ''}"


def can_fit_in_ram(model_id: str, headroom_gb: float = 4.0) -> tuple[bool, str]:
    """Check whether the model's estimated size fits in current free RAM + headroom."""
    est = estimate_size_gb(model_id)
    free = free_ram_gb()
    # A model swap kills the old process so its RAM returns; predict post-swap free
    # by adding back the size of whatever is currently loaded.
    active = detect_active_mlx_model()
    reclaim_after_kill = estimate_size_gb(active) if active else 0
    predicted_free = free + reclaim_after_kill
    fits = predicted_free >= (est + headroom_gb)
    msg = (f"need ~{est:.1f}GB + {headroom_gb:.0f}GB headroom, "
           f"will have ~{predicted_free:.1f}GB after swap "
           f"(now free {free:.1f}GB + reclaim {reclaim_after_kill:.1f}GB)")
    return fits, msg


def detect_active_mlx_model() -> str:
    """Return the --model arg of whichever mlx_lm.server actually owns port 8081.

    Earlier this used `ps | grep mlx_lm.server` and returned the FIRST match,
    which broke during failed-swap scenarios where multiple mlx_lm.server
    processes exist (one bound to 8081, others orphaned). Now we resolve the
    port owner via `lsof` first, then read its --model arg via `ps -p`.
    Returns empty string if no MLX server is bound.
    """
    import subprocess
    try:
        pid = subprocess.check_output(
            ["lsof", "-t", "-i", ":8081", "-sTCP:LISTEN"],
            text=True, timeout=5).strip().split("\n")[0]
        if not pid:
            return ""
    except Exception:
        return ""
    try:
        cmd = subprocess.check_output(
            ["ps", "-p", pid, "-o", "command="],
            text=True, timeout=5)
    except Exception:
        return ""
    parts = cmd.split()
    for i, p in enumerate(parts):
        if p == "--model" and i + 1 < len(parts):
            return parts[i + 1]
    return ""


def list_local_models():
    """Return models from Heimdall + flag the currently-active MLX one."""
    code, body, err = http(f"{HEIMDALL_BASE}/v1/models",
                            headers={"Authorization": f"Bearer {HEIMDALL_KEY}"})
    if code != 200 or not isinstance(body, dict):
        return []
    active = detect_active_mlx_model()
    out = []
    for m in body.get("data", []):
        mid = m.get("id", "")
        out.append({"id": mid, "size_gb": estimate_size_gb(mid), "active": mid == active})
    return out


def parse_models_override(s: str):
    """Parse 'a:google,b:heimdall' into [{id, provider}, ...]"""
    out = []
    for pair in s.split(","):
        pair = pair.strip()
        if not pair: continue
        if ":" in pair:
            mid, prov = pair.rsplit(":", 1)
            out.append({"id": mid.strip(), "provider": prov.strip()})
        else:
            out.append({"id": pair, "provider": "google"})
    return out


def build_default_model_list():
    """Default plan: cloud baselines + currently-loaded MLX (if any)."""
    plan = [
        {"id": "gemini-3-flash-preview", "provider": "google", "kind": "cloud"},
        {"id": "gemini-2.5-flash",        "provider": "google", "kind": "cloud"},
    ]
    active_mlx = detect_active_mlx_model()
    if active_mlx:
        plan.append({"id": active_mlx, "provider": "heimdall", "kind": "local-active"})
    return plan


def discover_resources():
    global TENANT_ID, BENCHMARK_ID
    if not TENANT_ID:
        for cand in ("asgard_medical", "default_tenant", "megacare"):
            code, b, _ = http(f"{API_BASE}/api/v1/agents", headers={"X-Tenant-Id": cand})
            if code == 200 and isinstance(b, dict) and b.get("agents"):
                TENANT_ID = cand
                break
        if not TENANT_ID:
            TENANT_ID = "default_tenant"

    if not BENCHMARK_ID:
        code, b, _ = http(f"{API_BASE}/api/v1/eval/benchmark-datasets", headers=auth_headers())
        ds = b if isinstance(b, list) else []
        match = (next((d for d in ds if d.get("source") == "healthbench_professional"), None)
                 or next((d for d in ds if d.get("is_active")), None))
        if match: BENCHMARK_ID = match["id"]


def find_agent():
    code, body, _ = http(f"{API_BASE}/api/v1/agents", headers=auth_headers())
    agents = body.get("agents", []) if isinstance(body, dict) else []
    return next((a for a in agents if a.get("name") == AGENT_NAME), None)


# ─── Agent model swap ────────────────────────────────────────────────────────

def update_agent_model(agent_id: int, model_id: str, provider: str):
    code, body, err = http(
        f"{API_BASE}/api/v1/agents/{agent_id}", method="PUT",
        body={"model_id": model_id, "provider": provider},
        headers=auth_headers())
    if code not in (200, 204):
        raise RuntimeError(f"Failed to update agent: {code} {body}")


# ─── Eval run lifecycle ───────────────────────────────────────────────────────

def trigger_run(agent_name, model_id, item_ids=None, hypothesis="", samples_per_item=None):
    payload = {
        "tenant_id": TENANT_ID,
        "agent_names": [agent_name],
        "model_ids": [model_id],
        "question_limit": ITEMS_PER_RUN,
        "benchmark_dataset_id": BENCHMARK_ID,
        "run_name": f"local-bench__{model_id.split('/')[-1][:40]}",
        "hypothesis": hypothesis or f"Local model comparison — {model_id}",
        "variable_under_test": "model_id",
        "expected_change": "ranking among local MLX models",
        "replicates": 1,
    }
    # Sprint 37 B-22: opt-in self-consistency via SAMPLES_PER_ITEM env (1..5)
    if samples_per_item is None:
        spi = int(os.environ.get("SAMPLES_PER_ITEM", "1"))
        samples_per_item = max(1, min(5, spi))
    if samples_per_item > 1:
        payload["samples_per_item"] = samples_per_item
        payload["variable_under_test"] = "samples_per_item"
        payload["expected_change"] = f"+{samples_per_item}× sampling, mean dim scores"
    if item_ids:
        payload["item_ids"] = item_ids
    code, body, _ = http(f"{API_BASE}/api/v1/eval/runs", "POST", payload, headers=auth_headers())
    if code not in (200, 202):
        raise RuntimeError(f"Trigger failed: {code} {body}")
    return body["run_id"]


def wait_for_run(run_id, max_wait=TIMEOUT_SEC):
    t0 = time.time()
    last = None
    while time.time() - t0 < max_wait:
        code, body, _ = http(f"{API_BASE}/api/v1/eval/runs/{run_id}", headers=auth_headers())
        if code != 200 or not isinstance(body, dict):
            time.sleep(3); continue
        status = body.get("run", {}).get("status", "")
        completed = body.get("run", {}).get("completed_combinations", 0)
        total = body.get("run", {}).get("total_combinations", 0)
        if status != last:
            last = status
            print(f"      ⌛ {status} {completed}/{total}  ({int(time.time()-t0)}s)")
        if status in ("COMPLETED", "FAILED", "CANCELLED"):
            return status, body
        time.sleep(5)
    return "TIMEOUT", None


def get_lock_items(run_id):
    code, body, _ = http(f"{API_BASE}/api/v1/eval/runs/{run_id}/lock-items", "POST",
                          headers=auth_headers())
    if code == 200 and isinstance(body, dict):
        return body.get("item_ids", [])
    return []


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print("═" * 70)
    print("🧪 Local LLM Benchmark — HealthBench")
    print("═" * 70)

    discover_resources()
    print(f"  tenant: {TENANT_ID}")
    print(f"  benchmark: {BENCHMARK_ID}")

    agent = find_agent()
    if not agent:
        print(f"❌ Agent '{AGENT_NAME}' not found in tenant '{TENANT_ID}'"); sys.exit(1)
    agent_id = agent["id"]
    original_model = agent["model_id"]
    original_provider = agent["provider"]
    print(f"  agent: {AGENT_NAME} (id={agent_id})  current model: {original_model}")

    # ── Build model plan ────────────────────────────────────────────
    if MODELS_OVERRIDE:
        runnable = parse_models_override(MODELS_OVERRIDE)
        print(f"\n→ Using MODELS env override ({len(runnable)} models)")
    else:
        runnable = build_default_model_list()
        print(f"\n→ Default plan: cloud baselines + currently-active MLX")

    print(f"\n→ All Heimdall MLX models (informational):")
    all_local = list_local_models()
    for m in all_local:
        flag = "✅ ACTIVE" if m.get("active") else "⏸  not loaded"
        print(f"    {flag}  {m['id']}  (~{m['size_gb']:.1f}GB at 4-bit)")
    print(f"\n→ Will benchmark these {len(runnable)} models:")
    for m in runnable:
        print(f"    RUN   {m['id']}  (provider={m['provider']})")
    print()

    runs = []
    locked_items = None  # capture from first successful run for reproducibility
    t_start = time.time()

    try:
        # ── Order Heimdall (MLX) models smallest-first to amortize RAM pressure
        #    cloud models go first since they don't need swap.
        def sort_key(m):
            is_local = m.get("provider") == "heimdall"
            return (1 if is_local else 0, estimate_size_gb(m["id"]))
        runnable = sorted(runnable, key=sort_key)
        print("\n→ Execution order (cloud first, then MLX smallest→largest):")
        for m in runnable:
            sz = estimate_size_gb(m["id"])
            kind = "cloud" if m.get("provider") != "heimdall" else f"MLX ~{sz:.1f}GB"
            print(f"    · {m['id']:55} ({kind})")

        for i, m in enumerate(runnable, 1):
            mid = m["id"]
            provider = m.get("provider", "google")
            print(f"\n[{i}/{len(runnable)}] 🤖 {mid} (provider={provider})")

            # MLX-only: pre-flight RAM check + warmup swap
            if provider == "heimdall":
                fits, fitmsg = can_fit_in_ram(mid)
                print(f"   📏 RAM: {fitmsg}")
                if not fits:
                    print(f"   ⏭  SKIP — insufficient RAM (close apps or pick smaller model)")
                    runs.append({"model": mid, "run_id": None, "status": "SKIPPED_OOM",
                                 "error": fitmsg})
                    continue
                # Decide warmup timeout based on size (linear ~30s/GB, min 120s, max 900s)
                warmup_timeout = max(120, min(900, int(estimate_size_gb(mid) * 30) + 120))
                ok, err = warmup_mlx_model(mid, timeout_sec=warmup_timeout)
                if not ok:
                    print(f"   ❌ warmup failed: {err}")
                    runs.append({"model": mid, "run_id": None, "status": "WARMUP_FAILED",
                                 "error": err})
                    continue

            try:
                update_agent_model(agent_id, mid, provider)
            except Exception as e:
                print(f"   ❌ update agent failed: {e}")
                runs.append({"model": mid, "run_id": None, "status": "ERROR", "error": str(e)})
                continue

            try:
                run_id = trigger_run(AGENT_NAME, mid, item_ids=locked_items)
                print(f"   run_id: {run_id[:8]}")
                status, body = wait_for_run(run_id)
                if status == "COMPLETED":
                    sums = body.get("summaries", [])
                    if sums:
                        s = sums[0]
                        runs.append({
                            "model": mid,
                            "run_id": run_id,
                            "status": "COMPLETED",
                            "acc": s.get("avg_accuracy"),
                            "comp": s.get("avg_completeness"),
                            "rel": s.get("avg_relevance"),
                            "safety": s.get("avg_safety_score"),
                            "unsafe": s.get("unsafe_count"),
                            "latency_ms": s.get("avg_latency_ms"),
                            "overall": s.get("overall_score"),
                            "cost_usd": body.get("run", {}).get("total_cost_usd", 0),
                        })
                        print(f"   ✅ COMPLETED · acc={s.get('avg_accuracy'):.2f} comp={s.get('avg_completeness'):.2f} rel={s.get('avg_relevance'):.2f} latency={s.get('avg_latency_ms'):.0f}ms")
                    else:
                        runs.append({"model": mid, "run_id": run_id, "status": "NO_SUMMARY"})
                    # Lock items for subsequent runs
                    if not locked_items:
                        locked_items = get_lock_items(run_id)
                        print(f"   📌 locked {len(locked_items)} items for replication")
                else:
                    runs.append({"model": mid, "run_id": run_id, "status": status})
                    print(f"   ❌ {status}")
            except Exception as e:
                print(f"   ❌ run error: {e}")
                runs.append({"model": mid, "run_id": None, "status": "ERROR", "error": str(e)})
    finally:
        if RESTORE_MODEL:
            print(f"\n→ Restoring agent model: {original_model}")
            try:
                update_agent_model(agent_id, original_model, original_provider)
            except Exception as e:
                print(f"   ⚠️  restore failed: {e}")

    elapsed = time.time() - t_start
    print()
    print("═" * 92)
    print(f"🏆 SCOREBOARD — {AGENT_NAME} on HealthBench ({ITEMS_PER_RUN} items, locked, total {elapsed:.0f}s)")
    print("═" * 92)
    print(f"{'Rank':4} {'Model':45} {'Acc':>5} {'Comp':>5} {'Rel':>5} {'Safe':>5} {'Lat(ms)':>8} {'Status':10}")
    print("─" * 92)
    completed = [r for r in runs if r["status"] == "COMPLETED"]
    completed.sort(key=lambda r: -(r.get("overall") or 0))
    for rank, r in enumerate(completed, 1):
        m = r["model"].split("/")[-1][:43]
        print(f"{rank:>4} {m:45} {r.get('acc',0):>5.2f} {r.get('comp',0):>5.2f} {r.get('rel',0):>5.2f} {r.get('safety',0):>5.2f} {r.get('latency_ms',0):>8.0f} {r['status']:10}")
    failed = [r for r in runs if r["status"] != "COMPLETED"]
    for r in failed:
        m = r["model"].split("/")[-1][:43]
        print(f"{'-':>4} {m:45} {'-':>5} {'-':>5} {'-':>5} {'-':>5} {'-':>8} {r['status']:10}")

    print()
    print(f"Run IDs (use for /evaluations):")
    for r in runs:
        if r.get("run_id"):
            print(f"  {r['model'].split('/')[-1][:50]:50}  {r['run_id']}")

    return 0 if completed else 1


if __name__ == "__main__":
    sys.exit(main())
