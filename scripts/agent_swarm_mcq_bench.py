#!/usr/bin/env python3
"""
Medical Agent benchmark — SWARM vs INDIVIDUAL → Mimir eval framework.

Benchmarks the *agents* (system prompt + RAG via Bifrost swarm engine), not the
raw model. Since all 20 asgard_medical agents share gemma-4-26b, differences come
from prompt/RAG/routing. Scoring = MCQ exact-match (MedQA US, 5-option) — no LLM
judge, no cloud, fully local. Results persist to eval_runs / eval_scores /
eval_summary (one summary row per agent + one for 'swarm').

  INDIVIDUAL: POST {bifrost}/v1/agents/{id}/run          (each agent answers)
  SWARM:      router(70) classifies → dispatch to specialist → that answer (2-hop)

Run:
  /Users/mimir/Developer/Heimdall/.venv/bin/python scripts/agent_swarm_mcq_bench.py --n 10
  (env BIFROST_URL default http://localhost:30100, TENANT asgard_medical)
"""
import argparse, json, re, subprocess, sys, time, uuid, random, urllib.request

INFRA_NS = "asgard-infra"
TENANT = "asgard_medical"
BASE = "/Users/mimir/Developer/Mimir/benchmarks/medical"
BIFROST = "http://localhost:30100"

# id → name (live roster, all gemma-4-26b/heimdall)
AGENTS = {
    51: "eir-clinical", 52: "eir-pharmacy", 53: "eir-pediatrics", 54: "eir-psychiatry",
    55: "eir-emergency", 56: "eir-internal-medicine", 57: "eir-surgery", 58: "eir-ophthalmology",
    59: "eir-orthopedics", 60: "eir-ob-gyn", 61: "eir-radiology", 62: "eir-medtech",
    63: "eir-nursing", 64: "eir-pt", 65: "eir-dietitian", 66: "eir-social-work",
    67: "eir-anesthesia", 68: "eir-ent", 69: "eir-urology",
}
ROUTER_ID = 70
# specialty keyword → agent id (for swarm dispatch)
SPECIALTY_MAP = {
    "internal": 56, "internal-medicine": 56, "clinical": 51, "cardio": 56, "pharmac": 52,
    "pediatr": 53, "psychiat": 54, "emergen": 55, "surg": 57, "ophthalm": 58, "orthop": 59,
    "ob": 60, "gyn": 60, "radiol": 61, "medtech": 62, "lab": 62, "nurs": 63, "physical": 64,
    "diet": 65, "nutri": 65, "social": 66, "anesth": 67, "ent": 68, "urol": 69,
}


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


def load_medqa(n, seed):
    rows = []
    with open(f"{BASE}/medqa/data_clean/questions/US/test.jsonl", encoding="utf-8") as f:
        for line in f:
            rows.append(json.loads(line))
    random.Random(seed).shuffle(rows)
    out = []
    for i, d in enumerate(rows[:n]):
        opts = "\n".join(f"{k}. {v}" for k, v in d["options"].items())
        out.append({
            "id": f"medqa-{i}",
            "q": f"{d['question']}\n\n{opts}\n\nReply with ONLY the letter of the single best option.",
            "valid": list(d["options"].keys()),
            "gold": d["answer_idx"].strip().upper(),
        })
    return out


def call_agent(agent_id, query, timeout=180):
    """POST to Bifrost swarm /run; return (final_answer, latency_ms, trace_id, reasoning)."""
    payload = json.dumps({"query": query}).encode()
    req = urllib.request.Request(
        f"{BIFROST}/v1/agents/{agent_id}/run", data=payload,
        headers={"Content-Type": "application/json", "X-Tenant-Id": TENANT})
    ts = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            d = json.loads(r.read())
        ms = int((time.time() - ts) * 1000)
        return (d.get("final_answer") or ""), ms, d.get("trace_id"), (d.get("reasoning") or "")
    except Exception as e:
        return f"(error: {str(e)[:80]})", int((time.time() - ts) * 1000), None, ""


def extract_letter(text, valid):
    """Pull the MCQ letter from a free-form agent answer."""
    if not text:
        return None
    t = text.strip()
    if len(t) == 1 and t.upper() in valid:
        return t.upper()
    m = re.search(r"(?:answer|ตอบ|option|choice)\s*(?:is|:|=)?\s*\(?\s*([A-E])\b", t, re.I)
    if m and m.group(1).upper() in valid:
        return m.group(1).upper()
    hits = [c for c in re.findall(r"\b([A-E])\b", t) if c in valid]
    return hits[-1].upper() if hits else None


def route_specialty(router_out):
    """Parse router output (JSON or text) → specialist agent id."""
    low = (router_out or "").lower()
    try:
        j = json.loads(re.search(r"\{.*\}", router_out, re.S).group(0))
        prim = str(j.get("primary_specialty", "")).lower()
        for k, v in SPECIALTY_MAP.items():
            if k in prim:
                return v, prim
    except Exception:
        pass
    for k, v in SPECIALTY_MAP.items():
        if k in low:
            return v, k
    return 56, "fallback:internal-medicine"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=10)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--agents", help="comma ids subset (default all 19 + swarm)")
    ap.add_argument("--no-swarm", action="store_true")
    ap.add_argument("--run-name")
    args = ap.parse_args()

    items = load_medqa(args.n, args.seed)
    n = len(items)
    ids = [int(x) for x in args.agents.split(",")] if args.agents else list(AGENTS)
    print(f"# MedQA n={n} | agents={len(ids)} | swarm={'no' if args.no_swarm else 'yes'} | bifrost={BIFROST}", file=sys.stderr)

    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES "
        "('gemma-4-26b','heimdall','chat',1,'{\"agent_bench\":true}') "
        "ON DUPLICATE KEY UPDATE updated_at=NOW()")

    run_id = str(uuid.uuid4())
    run_name = args.run_name or f"Eir Agent Swarm-vs-Individual MedQA — {time.strftime('%Y%m%d-%H%M%S')}"
    total = len(ids) * n + (0 if args.no_swarm else n)
    cfg = {"benchmark": "medqa", "runner": "agent_swarm_mcq_bench", "n": n, "seed": args.seed,
           "agents": ids, "swarm": (not args.no_swarm), "bifrost": BIFROST, "scoring": "mcq_accuracy"}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,config,tenant_id,variable_under_test) VALUES (" +
        ",".join([sql_quote(run_id), sql_quote(run_name), sql_quote("RUNNING"), str(total), "0",
                  sql_quote(json.dumps(cfg)), sql_quote(TENANT), sql_quote("agent")]) + ")")
    print(f"# run_id {run_id}", file=sys.stderr)

    summaries = []

    def score_agent(agent_name, answer_fn):
        hits, lat = 0, []
        for it in items:
            ans, ms, extra = answer_fn(it)
            pred = extract_letter(ans, it["valid"])
            hit = 1 if pred and pred == it["gold"] else 0
            hits += hit
            lat.append(ms)
            tags = json.dumps({"benchmark": "medqa", "predicted": pred, "gold": it["gold"], **extra})
            sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,tenant_id) VALUES (" +
                ",".join([sql_quote(run_id), sql_quote(agent_name), sql_quote("gemma-4-26b"),
                          sql_quote(it["q"][:500]), sql_quote(it["gold"]), sql_quote((ans or "(none)")[:500]),
                          str(hit), str(ms), sql_quote(it["id"][:64]), sql_quote(tags), sql_quote(TENANT)]) + ")")
        acc = hits / n if n else 0
        avg = sum(lat) / n if n else 0
        sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,avg_accuracy,avg_latency_ms,overall_score,tenant_id) VALUES (" +
            ",".join([sql_quote(run_id), sql_quote(agent_name), sql_quote("gemma-4-26b"), str(n),
                      str(round(acc, 4)), str(round(avg, 1)), str(round(acc, 4)), sql_quote(TENANT)]) + ")")
        summaries.append((agent_name, hits, n, acc, avg))
        print(f"  [{agent_name:22}] {hits}/{n} = {acc*100:5.1f}%   avg {avg:6.0f}ms", file=sys.stderr)

    # 1) INDIVIDUAL agents
    def individual(it, _id):
        ans, ms, tid, reasoning = call_agent(_id, it["q"])
        return ans, ms, {"trace_id": tid, "reasoning": reasoning}
    for aid in ids:
        name = AGENTS.get(aid, f"agent-{aid}")
        score_agent(name, lambda it, _id=aid: individual(it, _id))

    # 2) SWARM (router → specialist dispatch) — keep both trace_ids for evidence
    if not args.no_swarm:
        def swarm_answer(it):
            rout, ms1, rtid, _ = call_agent(ROUTER_ID, it["q"], timeout=120)
            sid, spec = route_specialty(rout)
            ans, ms2, stid, reasoning = call_agent(sid, it["q"])
            return ans, ms1 + ms2, {"routed_to": AGENTS.get(sid, sid), "specialty": spec,
                                    "router_trace_id": rtid, "trace_id": stid, "reasoning": reasoning}
        score_agent("swarm", swarm_answer)

    done = len(ids) * n + (0 if args.no_swarm else n)
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={done}, finished_at=NOW() WHERE id={sql_quote(run_id)}")

    print("\n## SCOREBOARD (MedQA n=%d) — agent swarm vs individual" % n)
    for name, h, nn, acc, avg in sorted(summaries, key=lambda x: -x[3]):
        print(f"  {name:22} {h:2}/{nn}  {acc*100:5.1f}%   {avg:6.0f}ms")
    print(f"\n  run_id: {run_id}  (tenant={TENANT}, scoring_fn=mcq_accuracy)")


if __name__ == "__main__":
    main()
