#!/usr/bin/env python3
"""
Thai→English medical-term normalization eval — accuracy + performance —
persisted into Mimir's generic eval framework (eval_benchmark_datasets /
eval_runs / eval_scores / eval_summary).

Per model (run as ONE process so RAM frees between models — Mac mini guardrail):
  load model (MLX) → for each Thai term: translate → /resolve → score hit
  capture latency + tokens/sec; write everything to Mimir.

Usage (Heimdall venv has mlx_lm; kubectl reaches the cluster):
  /Users/mimir/Developer/Heimdall/.venv/bin/python thai_normalize_eval.py \
      --model scb10x/typhoon2.1-gemma3-12b-mlx-4bit \
      --gold scripts/thai_normalize_gold.tsv
"""
import argparse, json, re, subprocess, sys, time, uuid

NS = "asgard"
INFRA_NS = "asgard-infra"
TENANT = "asgard_medical"  # where the user views /evaluations for medical work
PROMPT = (
    "You are a medical translator. Translate the Thai disease/condition term to "
    "its single canonical English name as used in medical terminologies (SNOMED "
    "CT / ICD-10). Output ONLY the English term — no Thai, no explanation.\n\n"
    "Thai: {term}\nEnglish:"
)


def sh(cmd, inp=None):
    r = subprocess.run(cmd, input=inp, capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.decode()[:400])
    return r.stdout.decode("utf-8")


def sql(q):
    return sh(["kubectl", "-n", INFRA_NS, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-uroot", "-proot", "--default-character-set=utf8mb4",
               "mimir", "-B", "-N", "-e", q])


def sql_quote(s):
    if s is None:
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def get_api_pod():
    out = sh(["kubectl", "-n", NS, "get", "pods", "-l", "app=mimir-api",
              "-o", "jsonpath={range .items[?(@.status.containerStatuses[0].ready==true)]}{.metadata.name}{\"\\n\"}{end}"])
    return out.split("\n")[0].strip()


def resolve_top(pod, text):
    q = text.replace('"', "'")
    out = sh(["kubectl", "-n", NS, "exec", pod, "--", "curl", "-s", "-m12",
              "-X", "POST", "http://localhost:8080/api/v1/knowledge/primekg/resolve",
              "-H", "Content-Type: application/json", "-d", json.dumps({"text": text, "limit": 3})])
    try:
        d = json.loads(out)
        r = d.get("resolved") or [{}]
        return (r[0].get("name") or "").lower(), d.get("via")
    except Exception:
        return "", None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--gold", required=True)
    ap.add_argument("--max-tokens", type=int, default=24)
    ap.add_argument("--server", help="OpenAI-compatible base URL (e.g. http://localhost:8081/v1) — reuse a loaded model instead of local MLX load")
    ap.add_argument("--auth-key", help="Bearer token for --server (e.g. Google GEMINI_API_KEY for the cloud OpenAI-compat endpoint)")
    ap.add_argument("--provider", default="heimdall", help="provider label for ai_models registration")
    ap.add_argument("--reasoning-effort", default="none", help="cloud thinking budget: none|low|medium|high (gemini). 'none' = disable thinking for this normalize task")
    ap.add_argument("--dataset-name", default="thai-disease-normalize-v1", help="eval_benchmark_datasets name (use v2 for the expanded set)")
    args = ap.parse_args()

    # gold
    gold = []
    with open(args.gold, encoding="utf-8") as f:
        for line in f:
            p = line.rstrip("\n").split("\t")
            if p and p[0].strip():
                gold.append((p[0].strip(), p[1] if len(p) > 1 else "", p[2] if len(p) > 2 else ""))

    # dataset (idempotent by name)
    ds_name = args.dataset_name
    ds_id = sql(f"SELECT id FROM eval_benchmark_datasets WHERE name={sql_quote(ds_name)} AND tenant_id={sql_quote(TENANT)} LIMIT 1").strip()
    if not ds_id:
        ds_id = str(uuid.uuid4())
        items = json.dumps([{"thai": t, "expected": e, "group": g} for t, e, g in gold], ensure_ascii=False)
        sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,scoring_fn,description,items,total_items,version,is_active) VALUES (" +
            ",".join([sql_quote(ds_id), sql_quote(TENANT), sql_quote(ds_name), sql_quote("manual"),
                      sql_quote("keyword_match"), sql_quote("Thai disease term → EN → /resolve → PrimeKG node; accuracy + perf"),
                      sql_quote(items), str(len(gold)), "1", "1"]) + ")")
        print(f"# dataset created {ds_id}", file=sys.stderr)

    # Generation backend: either a local MLX load (separate process, isolated
    # RAM) OR an existing OpenAI-compatible server (e.g. :8081 that already holds
    # the production model — avoids double-loading 26B/27B and OOM risk).
    import urllib.request
    if args.server:
        load_s = 0.0
        def translate(thai):
            payload = {
                "model": args.model,
                "messages": [{"role": "user", "content": PROMPT.format(term=thai)}],
                "max_tokens": args.max_tokens, "temperature": 0,
            }
            if not args.auth_key:  # local MLX (:8081) extension; cloud rejects it
                payload["chat_template_kwargs"] = {"enable_thinking": False}
            elif args.reasoning_effort:  # cloud (gemini): disable/limit thinking
                payload["reasoning_effort"] = args.reasoning_effort
            body = json.dumps(payload).encode()
            ts = time.time()
            hdrs = {"Content-Type": "application/json"}
            if args.auth_key:
                hdrs["Authorization"] = "Bearer " + args.auth_key
            req = urllib.request.Request(args.server.rstrip("/") + "/chat/completions",
                                         data=body, headers=hdrs)
            with urllib.request.urlopen(req, timeout=120) as r:
                d = json.loads(r.read())
            ms = int((time.time() - ts) * 1000)
            out = d.get("choices", [{}])[0].get("message", {}).get("content", "") or ""
            ntok = d.get("usage", {}).get("completion_tokens", len(out.split()))
            return out, ntok, ms
        print(f"# using server {args.server} for {args.model}", file=sys.stderr)
    else:
        from mlx_lm import load, generate
        t0 = time.time()
        model, tok = load(args.model)
        load_s = round(time.time() - t0, 1)
        print(f"# loaded {args.model} in {load_s}s", file=sys.stderr)
        def translate(thai):
            msgs = [{"role": "user", "content": PROMPT.format(term=thai)}]
            # Thinking models (Qwen3, etc.) otherwise burn the token budget on a
            # "Thinking Process:" preamble and never emit the term. enable_thinking
            # is honored by their chat template; ignored by non-thinking ones (gemma).
            try:
                prompt = tok.apply_chat_template(msgs, add_generation_prompt=True,
                                                 tokenize=False, enable_thinking=False)
            except TypeError:
                prompt = tok.apply_chat_template(msgs, add_generation_prompt=True, tokenize=False)
            ts = time.time()
            out = generate(model, tok, prompt=prompt, max_tokens=args.max_tokens, verbose=False)
            ms = int((time.time() - ts) * 1000)
            return out, len(tok.encode(out)), ms

    # Register model in ai_models (eval_scores.model_id has a FK to it). Idempotent.
    meta = json.dumps({"runtime": "cloud" if args.auth_key else "mlx", "thai_normalizer_candidate": True})
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES (" +
        ",".join([sql_quote(args.model), sql_quote(args.provider), sql_quote("chat"), "1", sql_quote(meta)]) +
        ") ON DUPLICATE KEY UPDATE updated_at=NOW()")

    pod = get_api_pod()
    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": args.model,
           "runner": "mlx_normalize+primekg_resolve", "load_seconds": load_s}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,config,tenant_id,variable_under_test) VALUES (" +
        ",".join([sql_quote(run_id), sql_quote(f"Thai normalize — {args.model.split('/')[-1]}"),
                  sql_quote("RUNNING"), str(len(gold)), "0", sql_quote(json.dumps(cfg)),
                  sql_quote(TENANT), sql_quote("normalizer_model")]) + ")")

    hits = 0; lat = []; tps = []
    for idx, (thai, expect, grp) in enumerate(gold):
        out, ntok, ms = translate(thai)
        eng = out.strip().splitlines()[0].strip().strip(".").strip() if out.strip() else ""
        tok_s = round(ntok / max(ms / 1000, 1e-3), 1)
        top, via = resolve_top(pod, eng)
        hit = 1 if (expect and re.search(expect, top, re.I)) else 0
        hits += hit; lat.append(ms); tps.append(tok_s)
        tags = json.dumps({"group": grp, "via": via, "tokens_per_sec": tok_s, "en_output": eng})
        sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,tenant_id) VALUES (" +
            ",".join([sql_quote(run_id), sql_quote("thai-normalizer"), sql_quote(args.model), sql_quote(thai), sql_quote(expect),
                      sql_quote(f"{eng} -> {top}"), str(hit), str(ms), sql_quote(f"thai-{idx}"),
                      sql_quote(tags), sql_quote(TENANT)]) + ")")
        print(f"  {'OK' if hit else '..'} {thai} → {eng} → {top or '(none)'}  [{ms}ms {tok_s}tok/s]")

    n = len(gold)
    avg_lat = sum(lat) / n
    p90 = sorted(lat)[int(0.9 * n) - 1] if n else 0
    avg_tps = sum(tps) / n
    acc = hits / n
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,avg_accuracy,avg_latency_ms,overall_score,tenant_id) VALUES (" +
        ",".join([sql_quote(run_id), sql_quote("thai-normalizer"), sql_quote(args.model), str(n), str(round(acc, 4)),
                  str(round(avg_lat, 1)), str(round(acc, 4)), sql_quote(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={n}, finished_at=NOW(), "
        f"config=JSON_SET(config,'$.accuracy',{round(acc,4)},'$.avg_latency_ms',{round(avg_lat,1)},"
        f"'$.p90_latency_ms',{p90},'$.avg_tokens_per_sec',{round(avg_tps,1)}) WHERE id={sql_quote(run_id)}")

    print(f"\n## {args.model}")
    print(f"   accuracy: {hits}/{n} = {acc*100:.0f}%")
    print(f"   perf: avg {avg_lat:.0f}ms  p90 {p90}ms  {avg_tps:.0f}tok/s  load {load_s}s")
    print(f"   run_id: {run_id}  (Mimir eval, tenant={TENANT})")


if __name__ == "__main__":
    sys.exit(main())
