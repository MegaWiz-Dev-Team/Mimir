#!/usr/bin/env python3
"""
Medical MCQ benchmark harness → Mimir eval framework.

Benchmarks (scoring_fn = mcq_accuracy):
  medqa       — USMLE-style 5-option MCQ        (data_clean/questions/US/test.jsonl)
  medmcqa     — 4-option MCQ (validation)        (data/validation-*.parquet)
  medxpertqa  — expert ≤10-option MCQ (Text)     (Text/test.jsonl)
  pubmedqa    — yes/no/maybe over abstract        (pqa_labeled/train-*.parquet)

Per model run as ONE process (Mac mini RAM guardrail). Generation backend is the
same as thai_normalize_eval.py: --server (OpenAI-compatible, e.g. local :8081 or
Google cloud) or a local mlx_lm load.

  /Users/mimir/Developer/Heimdall/.venv/bin/python scripts/mcq_eval.py \
      --benchmark medqa --model mlx-community/gemma-4-26b-a4b-it-4bit \
      --server http://localhost:8081/v1 --n 200
"""
import argparse, json, re, subprocess, sys, time, uuid, random, urllib.request

NS = "asgard"
INFRA_NS = "asgard-infra"
TENANT = "asgard_medical"
BASE = "/Users/mimir/Developer/Mimir/benchmarks/medical"

# ── Mimir persistence helpers (same pattern as thai_normalize_eval.py) ──────
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


# ── Benchmark loaders → list of {id, prompt_q, options, gold, tags} ─────────
def _opts_block(options: dict) -> str:
    return "\n".join(f"{k}. {v}" for k, v in options.items())


def load_medqa(n, seed):
    rows = []
    with open(f"{BASE}/medqa/data_clean/questions/US/test.jsonl", encoding="utf-8") as f:
        for line in f:
            d = json.loads(line)
            rows.append(d)
    random.Random(seed).shuffle(rows)
    out = []
    for i, d in enumerate(rows[:n]):
        out.append({
            "id": f"medqa-{i}",
            "prompt_q": f"{d['question']}\n\n{_opts_block(d['options'])}",
            "kind": "mcq", "valid": list(d["options"].keys()),
            "gold": d["answer_idx"].strip().upper(),
            "tags": {"meta_info": d.get("meta_info")},
        })
    return out


def load_medmcqa(n, seed):
    import pyarrow.parquet as pq
    t = pq.read_table(f"{BASE}/medmcqa/data/validation-00000-of-00001.parquet").to_pylist()
    random.Random(seed).shuffle(t)
    letters = ["A", "B", "C", "D"]
    out = []
    for i, d in enumerate(t[:n]):
        options = {"A": d["opa"], "B": d["opb"], "C": d["opc"], "D": d["opd"]}
        out.append({
            "id": f"medmcqa-{i}",
            "prompt_q": f"{d['question']}\n\n{_opts_block(options)}",
            "kind": "mcq", "valid": letters,
            "gold": letters[int(d["cop"])],
            "tags": {"subject": d.get("subject_name"), "choice_type": d.get("choice_type")},
        })
    return out


def load_medxpertqa(n, seed):
    rows = []
    with open(f"{BASE}/medxpertqa/Text/test.jsonl", encoding="utf-8") as f:
        for line in f:
            rows.append(json.loads(line))
    random.Random(seed).shuffle(rows)
    out = []
    for i, d in enumerate(rows[:n]):
        opts = d.get("options") or {}
        # `question` already embeds the answer choices; use it directly.
        out.append({
            "id": d.get("id", f"medxpert-{i}"),
            "prompt_q": d["question"],
            "kind": "mcq", "valid": list(opts.keys()) or list("ABCDEFGHIJ"),
            "gold": str(d["label"]).strip().upper(),
            "tags": {"medical_task": d.get("medical_task"), "body_system": d.get("body_system"),
                     "question_type": d.get("question_type")},
        })
    return out


def load_pubmedqa(n, seed):
    import pyarrow.parquet as pq
    t = pq.read_table(f"{BASE}/pubmedqa/pqa_labeled/train-00000-of-00001.parquet").to_pylist()
    random.Random(seed).shuffle(t)
    out = []
    for i, d in enumerate(t[:n]):
        ctx = d.get("context")
        # context is a struct with a 'contexts' list of paragraph strings.
        texts = []
        if isinstance(ctx, dict):
            texts = ctx.get("contexts") or []
        elif isinstance(ctx, (list, tuple)):
            for item in ctx:
                if isinstance(item, (list, tuple)) and len(item) == 2 and item[0] == "contexts":
                    texts = item[1]
        ctx_text = " ".join(texts)[:1800] if texts else ""
        out.append({
            "id": f"pubmedqa-{d.get('pubid', i)}",
            "prompt_q": d["question"],
            "context": ctx_text,
            "kind": "pqa", "valid": ["yes", "no", "maybe"],
            "gold": str(d["final_decision"]).strip().lower(),
            "tags": {},
        })
    return out


LOADERS = {"medqa": load_medqa, "medmcqa": load_medmcqa,
           "medxpertqa": load_medxpertqa, "pubmedqa": load_pubmedqa}
DATASET_NAMES = {"medqa": "medqa-usmle", "medmcqa": "medmcqa",
                 "medxpertqa": "medxpertqa-text", "pubmedqa": "pubmedqa-labeled"}


def build_prompt(item):
    if item["kind"] == "pqa":
        ctx = f"Abstract: {item['context']}\n\n" if item.get("context") else ""
        return (f"You are a biomedical expert. {ctx}Question: {item['prompt_q']}\n\n"
                "Answer with ONLY one word — yes, no, or maybe.\nAnswer:")
    return ("You are a medical expert taking a board examination. Choose the single "
            "best answer. Respond with ONLY the letter of the correct option — no "
            f"explanation.\n\n{item['prompt_q']}\n\nAnswer:")


def extract(out, item):
    if item["kind"] == "pqa":
        m = re.search(r"\b(yes|no|maybe)\b", out, re.I)
        return m.group(1).lower() if m else ""
    valid = set(item["valid"])
    for ch in out.upper():
        if ch in valid:
            return ch
    m = re.search(r"[A-J]", out.upper())
    return m.group(0) if m else ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--benchmark", required=True, choices=list(LOADERS))
    ap.add_argument("--model", required=True)
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--max-tokens", type=int, default=12)
    ap.add_argument("--server", help="OpenAI-compatible base URL (reuse a loaded model)")
    ap.add_argument("--auth-key", help="Bearer token for --server (cloud)")
    ap.add_argument("--provider", default="heimdall")
    ap.add_argument("--reasoning-effort", default="none")
    args = ap.parse_args()

    items = LOADERS[args.benchmark](args.n, args.seed)
    n = len(items)
    print(f"# {args.benchmark}: {n} items", file=sys.stderr)

    # dataset (idempotent by name+tenant)
    ds_name = DATASET_NAMES[args.benchmark]
    ds_id = sql(f"SELECT id FROM eval_benchmark_datasets WHERE name={sql_quote(ds_name)} AND tenant_id={sql_quote(TENANT)} LIMIT 1").strip()
    if not ds_id:
        ds_id = str(uuid.uuid4())
        meta_items = json.dumps([{"id": it["id"], "gold": it["gold"], "tags": it["tags"]} for it in items], ensure_ascii=False)
        sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,scoring_fn,description,items,total_items,version,is_active) VALUES (" +
            ",".join([sql_quote(ds_id), sql_quote(TENANT), sql_quote(ds_name), sql_quote("medical-benchmark"),
                      sql_quote("mcq_accuracy"), sql_quote(f"{args.benchmark} multiple-choice medical QA; accuracy"),
                      sql_quote(meta_items), str(n), "1", "1"]) + ")")
        print(f"# dataset created {ds_id}", file=sys.stderr)

    # generation backend (same as thai_normalize_eval.py)
    if args.server:
        load_s = 0.0
        def generate(prompt):
            payload = {"model": args.model,
                       "messages": [{"role": "user", "content": prompt}],
                       "max_tokens": args.max_tokens, "temperature": 0}
            if not args.auth_key:
                payload["chat_template_kwargs"] = {"enable_thinking": False}
            elif args.reasoning_effort:
                payload["reasoning_effort"] = args.reasoning_effort
            hdrs = {"Content-Type": "application/json"}
            if args.auth_key:
                hdrs["Authorization"] = "Bearer " + args.auth_key
            ts = time.time()
            req = urllib.request.Request(args.server.rstrip("/") + "/chat/completions",
                                         data=json.dumps(payload).encode(), headers=hdrs)
            with urllib.request.urlopen(req, timeout=120) as r:
                d = json.loads(r.read())
            ms = int((time.time() - ts) * 1000)
            return d.get("choices", [{}])[0].get("message", {}).get("content", "") or "", ms
        print(f"# using server {args.server} for {args.model}", file=sys.stderr)
    else:
        from mlx_lm import load, generate as mlx_generate
        t0 = time.time()
        model, tok = load(args.model)
        load_s = round(time.time() - t0, 1)
        print(f"# loaded {args.model} in {load_s}s", file=sys.stderr)
        def generate(prompt):
            msgs = [{"role": "user", "content": prompt}]
            try:
                p = tok.apply_chat_template(msgs, add_generation_prompt=True, tokenize=False, enable_thinking=False)
            except TypeError:
                p = tok.apply_chat_template(msgs, add_generation_prompt=True, tokenize=False)
            ts = time.time()
            out = mlx_generate(model, tok, prompt=p, max_tokens=args.max_tokens, verbose=False)
            return out, int((time.time() - ts) * 1000)

    # register model (eval_scores.model_id FK)
    meta = json.dumps({"runtime": "cloud" if args.auth_key else "mlx", "mcq_benchmark": True})
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES (" +
        ",".join([sql_quote(args.model), sql_quote(args.provider), sql_quote("chat"), "1", sql_quote(meta)]) +
        ") ON DUPLICATE KEY UPDATE updated_at=NOW()")

    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": args.model, "benchmark": args.benchmark,
           "runner": "mcq_eval", "n": n, "seed": args.seed, "load_seconds": load_s}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,config,tenant_id,variable_under_test) VALUES (" +
        ",".join([sql_quote(run_id), sql_quote(f"{ds_name} — {args.model.split('/')[-1]}"),
                  sql_quote("RUNNING"), str(n), "0", sql_quote(json.dumps(cfg)),
                  sql_quote(TENANT), sql_quote("model")]) + ")")

    hits = 0
    lat = []
    for idx, it in enumerate(items):
        out, ms = generate(build_prompt(it))
        pred = extract(out, it)
        gold = it["gold"]
        hit = 1 if pred and pred.lower() == gold.lower() else 0
        hits += hit
        lat.append(ms)
        tags = json.dumps({**it["tags"], "benchmark": args.benchmark, "predicted": pred, "gold": gold})
        sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,tenant_id) VALUES (" +
            ",".join([sql_quote(run_id), sql_quote("mcq-bench"), sql_quote(args.model),
                      sql_quote(it["prompt_q"][:500]), sql_quote(gold), sql_quote(pred or "(none)"),
                      str(hit), str(ms), sql_quote(it["id"][:64]), sql_quote(tags), sql_quote(TENANT)]) + ")")
        if (idx + 1) % 25 == 0:
            print(f"  {idx+1}/{n}  acc so far {hits/(idx+1)*100:.1f}%", file=sys.stderr)

    acc = hits / n if n else 0
    avg_lat = sum(lat) / n if n else 0
    p90 = sorted(lat)[int(0.9 * n) - 1] if n else 0
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,avg_accuracy,avg_latency_ms,overall_score,tenant_id) VALUES (" +
        ",".join([sql_quote(run_id), sql_quote("mcq-bench"), sql_quote(args.model), str(n),
                  str(round(acc, 4)), str(round(avg_lat, 1)), str(round(acc, 4)), sql_quote(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={n}, finished_at=NOW(), "
        f"config=JSON_SET(config,'$.accuracy',{round(acc,4)},'$.avg_latency_ms',{round(avg_lat,1)},'$.p90_latency_ms',{p90}) "
        f"WHERE id={sql_quote(run_id)}")

    print(f"\n## {args.benchmark} · {args.model}")
    print(f"   accuracy: {hits}/{n} = {acc*100:.1f}%   avg {avg_lat:.0f}ms  p90 {p90}ms")
    print(f"   run_id: {run_id}  (tenant={TENANT}, scoring_fn=mcq_accuracy)")


if __name__ == "__main__":
    sys.exit(main())
