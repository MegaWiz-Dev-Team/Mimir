#!/usr/bin/env python3
"""Persist the three PrimeKG resolver test suites (bug-class /resolve,
disease_relations, full-coverage 6-endpoints) into the Mimir eval framework so
results are durable and comparable across mimir-api versions.

  Tenant       asgard_platform   (cross-cutting infra benchmarks, PII-free)
  Datasets     3, one per suite, scoring_fn=mcq_accuracy (0/1 pass)
  Runs         3, one per dataset, variable_under_test=mimir-api version
  Agent name   primekg-bug-class | primekg-disease-relations | primekg-full-coverage
  Model_id     asgard-mimir-api:<VERSION>  (registered in ai_models)

  Usage:
    .../python scripts/persist_primekg_resolver_eval.py            # auto-detect version
    .../python scripts/persist_primekg_resolver_eval.py --version v2.3.50
"""
import argparse
import json
import subprocess
import sys
import time
import uuid

TENANT = "asgard_platform"
INFRA_NS = "asgard-infra"
ASGARD_NS = "asgard"
BASE = "http://localhost:8080/api/v1/knowledge/primekg"
POD = "deploy/mimir-api"


def sh(cmd, inp=None):
    r = subprocess.run(cmd, input=inp, capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.decode()[:400])
    return r.stdout.decode("utf-8")


def sql(q):
    return sh(["kubectl", "-n", INFRA_NS, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-uroot", "-proot", "--default-character-set=utf8mb4",
               "mimir", "-B", "-N", "-e", q])


def qstr(s):
    if s is None:
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def call(path, body):
    cmd = ["kubectl", "-n", ASGARD_NS, "exec", "-i", POD, "--", "sh", "-c",
           f"curl -s -m 25 {BASE}{path} -H 'Content-Type: application/json' "
           f"-H 'X-Tenant-Id: asgard_medical' --data-binary @-"]
    t0 = time.time()
    r = subprocess.run(cmd, input=json.dumps(body, ensure_ascii=False).encode(),
                       capture_output=True, timeout=40)
    ms = int((time.time() - t0) * 1000)
    try:
        return json.loads(r.stdout.decode("utf-8") or "{}"), ms
    except Exception:
        return {"error": (r.stdout.decode("utf-8", "replace")[:200]
                          if r.stdout else "(empty)")}, ms


def detect_version():
    img = sh(["kubectl", "-n", ASGARD_NS, "get", "deploy", "mimir-api",
              "-o", "jsonpath={.spec.template.spec.containers[0].image}"]).strip()
    return img.split(":")[-1] if ":" in img else img


# ════════ SUITE 1: /resolve bug-class (24 cases) ════════
def suite_bug_class():
    cases = [
        # (input, expected_keyword, class)
        ("Coats disease",         "coats",        "A apostrophe-stripped"),
        ("Alzheimer disease",     "alzheimer",    "A apostrophe-stripped"),
        ("Alzheimers disease",    "alzheimer",    "A apostrophe-stripped"),
        ("Parkinson disease",     "parkinson",    "A apostrophe-stripped"),
        ("Crohn disease",         "crohn",        "A apostrophe-stripped"),
        ("Hodgkin disease",       "hodgkin",      "A apostrophe-stripped"),
        ("Huntington disease",    "huntington",   "A apostrophe-stripped"),
        ("Sezary disease",        "sezary",       "A apostrophe-stripped"),
        ("Gaucher disease",       "gaucher",      "A apostrophe-stripped"),
        ("Lesch Nyhan syndrome",  "lesch",        "B hyphen-as-space"),
        ("Coffin Siris syndrome", "coffin",       "B hyphen-as-space"),
        ("Kleine Levin syndrome", "kleine",       "B hyphen-as-space"),
        ("Coats' disease",        "coats",        "C punct-preserved"),
        ("Alzheimer's disease",   "alzheimer",    "C punct-preserved"),
        ("Lesch-Nyhan syndrome",  "lesch",        "C punct-preserved"),
        ("diabetes",              "diabetes",     "R regression"),
        ("hypertension",          "hypertens",    "R regression"),
        ("asthma",                "asthma",       "R regression"),
        ("ไข้เลือดออก",            "dengue",       "T thai"),
        ("ภาวะซึมเศร้า",            "depress",      "T thai"),
        ("สิว",                    "acne",         "T thai"),
        ("โรคพาร์กินสัน",          "parkinson",    "T thai"),
        ("T2DM",                   "diabetes",    "X acronym"),
        ("COPD",                   "chronic obstructive", "X acronym"),
    ]
    rows = []
    for inp, kw, klass in cases:
        res, ms = call("/resolve", {"text": inp})
        names = [e.get("name", "") for e in res.get("resolved", [])]
        grouped = [e.get("grouped_name", "") for e in res.get("resolved", [])]
        top = (names[0] if names else "") + " " + (grouped[0] if grouped else "")
        ok = bool(names) and kw.lower() in top.lower()
        rows.append({
            "item_id": f"bc-{len(rows)+1:02d}",
            "input": inp, "expected": kw, "group": klass,
            "got": names[0] if names else "(none)",
            "ok": ok, "ms": ms,
        })
    return rows


# ════════ SUITE 2: /disease_relations (15 cases) ════════
def suite_disease_relations():
    cases = [
        ("Coats' disease",        "coats",        "A apostrophe"),
        ("Alzheimer's disease",   "alzheimer",    "A apostrophe"),
        ("Parkinson's disease",   "parkinson",    "A apostrophe"),
        ("Crohn's disease",       "crohn",        "A apostrophe"),
        ("Huntington's disease",  "huntington",   "A apostrophe"),
        ("Coats disease",         "coats",        "B no-apostrophe"),
        ("Alzheimer disease",     "alzheimer",    "B no-apostrophe"),
        ("Alzheimers disease",    "alzheimer",    "C typo"),
        ("Lesch Nyhan syndrome",  "lesch",        "D hyphen-as-space"),
        ("Coffin Siris syndrome", "coffin",       "D hyphen-as-space"),
        ("diabetes",              "diabetes",     "R regression"),
        ("hypertension",          "hypertens",    "R regression"),
        ("asthma",                "asthma",       "R regression"),
        ("ภาวะซึมเศร้า",          "depress",      "T thai"),
        ("โรคพาร์กินสัน",         "parkinson",    "T thai"),
    ]
    rows = []
    for inp, kw, klass in cases:
        res, ms = call("/disease_relations", {"query": inp})
        seed = (res.get("seed") or {}).get("name", "") or ""
        text = (res.get("resolved_disease", "") + " " + seed).lower()
        ok = res.get("found", False) and kw.lower() in text
        rows.append({
            "item_id": f"dr-{len(rows)+1:02d}",
            "input": inp, "expected": kw, "group": klass,
            "got": seed or res.get("resolved_disease", "(none)"),
            "ok": ok, "ms": ms,
        })
    return rows


# ════════ SUITE 2b: /resolve fuzzy "did you mean" (9 cases) ════════
# Cypher CONTAINS has zero spell-correction, so a single-char typo
# ("Amarosis" instead of "Amaurosis") silently misses. The Jaro-Winkler
# fallback wired into resolve Step 3b should surface the intended disease
# in `did_you_mean[]` even when `resolved[]` is empty. Keep these on the
# 0.85 threshold edge so a regression in jaro_winkler or the cutoff value
# fails CI rather than degrades the Medical Knowledge Assistant silently.
def suite_fuzzy_typos():
    cases = [
        # (typo input, expected keyword in did_you_mean[].name)
        ("Amarosis Fugax",        "amaurosis",   "E single-char-omission"),
        ("Amaurosis Fugaks",      "amaurosis",   "E single-char-swap"),
        ("Alzhiemer disease",     "alzheimer",   "E adjacent-transpose"),
        ("Parkinsom disease",     "parkinson",   "E one-letter-substitute"),
        ("Diabettes Melitus",     "diabetes",    "E double-typo"),
        ("Hodgkins lymfoma",      "hodgkin",     "E phonetic-mishear"),
        ("hypertention",          "hypertens",   "E common-misspelling"),
        ("astma",                 "asthma",      "E silent-letter-drop"),
        ("Chrohns",               "crohn",       "E extra-letter"),
    ]
    rows = []
    for inp, kw, klass in cases:
        # /entity carries the fuzzy "did you mean" fallback (main). The /resolve
        # handler now exists too (mimir-resolve, merged from this branch) — a
        # follow-up can move this assertion to /resolve for an end-to-end
        # SNOMED → MONDO → PrimeKG signal.
        res, ms = call("/entity", {"name": inp, "limit": 5})
        suggestions = res.get("did_you_mean", []) or []
        names = [s.get("name", "") for s in suggestions]
        top = (names[0] if names else "").lower()
        ok = bool(suggestions) and kw.lower() in top
        rows.append({
            "item_id": f"fz-{len(rows)+1:02d}",
            "input": inp, "expected": kw, "group": klass,
            "got": names[0] if names else "(none)",
            "ok": ok, "ms": ms,
        })
    return rows


# ════════ SUITE 3: /entity + /neighbors + /drug_interactions + /disease_drugs + /symptom_to_disease + /path (26 cases) ════════
def lookup_one(name, type_filter=None):
    body = {"name": name, "limit": 1}
    if type_filter:
        body["type"] = type_filter
    res, _ = call("/entity", body)
    items = res.get("items", [])
    return items[0] if items else None


def suite_full_coverage():
    rows = []
    def add(label, group, ok, ms, expected="", got=""):
        rows.append({"item_id": f"fc-{len(rows)+1:02d}", "input": label,
                      "expected": expected, "group": group, "got": got,
                      "ok": ok, "ms": ms})

    # A. /entity
    for name, kw in [("diabetes", "diabetes"), ("Coats disease", "coats"),
                     ("metformin", "metformin"), ("hypertension", "hypertens"),
                     ("xyznonexistentdz", None)]:
        res, ms = call("/entity", {"name": name, "limit": 3})
        items = res.get("items", [])
        if kw is None:
            ok = len(items) == 0
            got = f"count={len(items)}"
        else:
            top = (items[0].get("name", "") if items else "").lower()
            ok = len(items) > 0 and kw in top
            got = top[:50]
        add(f"entity '{name}'", "A /entity", ok, ms, kw or "(empty)", got)

    # Resolve indices for parametric endpoints
    DM = lookup_one("diabetes mellitus", "disease")
    HTN = lookup_one("hypertension", "disease")
    ASTH = lookup_one("asthma", "disease")
    MET = lookup_one("metformin", "drug")
    WARF = lookup_one("warfarin", "drug")

    # B. /neighbors
    if DM:
        r1, ms = call("/neighbors", {"entity_index": DM["entity_index"], "hops": 1, "limit": 25})
        add("neighbors DM 1-hop", "B /neighbors", r1.get("count", 0) > 0, ms,
            ">0", f"count={r1.get('count')}")
        r2, ms = call("/neighbors", {"entity_index": DM["entity_index"], "hops": 2, "limit": 25})
        add("neighbors DM 2-hop>=1-hop", "B /neighbors",
            r2.get("count", 0) >= r1.get("count", 0), ms,
            ">=1hop", f"2hop={r2.get('count')}")
        nf, ms = call("/neighbors", {"entity_index": DM["entity_index"],
                                     "relation_types": ["INDICATION"], "limit": 25})
        is_drug = lambda n: (n.get("type") == "drug"
                              or str(n.get("entity_id", "")).startswith("DB"))
        ok = nf.get("count", 0) > 0 and all(is_drug(i) for i in nf.get("items", []))
        add("neighbors DM filter=INDICATION", "B /neighbors", ok, ms,
            "all drugs", f"count={nf.get('count')}")

    # C. /drug_interactions
    for drug, label in [(WARF, "warfarin"), (MET, "metformin")]:
        if drug:
            r, ms = call("/drug_interactions",
                         {"drug_index": drug["entity_index"], "limit": 25})
            items = r.get("items", [])
            add(f"drug_interactions {label} >0", "C /drug_interactions",
                r.get("count", 0) > 0, ms, ">0", f"count={r.get('count')}")
            all_db = bool(items) and all(
                str(i.get("entity_id", "")).startswith("DB") for i in items)
            add(f"drug_interactions {label} all DB", "C /drug_interactions",
                all_db, ms, "all DB ids", "")
            add(f"drug_interactions {label} severity-contract",
                "C /drug_interactions",
                r.get("severity_filter_supported") is False, ms,
                "False", str(r.get("severity_filter_supported")))

    # D. /disease_drugs
    for d, label in [(HTN, "hypertension"), (DM, "diabetes mellitus"),
                     (ASTH, "asthma")]:
        if d:
            r, ms = call("/disease_drugs",
                         {"disease_index": d["entity_index"],
                          "limit_per_relation": 5})
            groups = r if isinstance(r, dict) and "error" not in r else {}
            non_empty = {k: v for k, v in groups.items()
                         if isinstance(v, list) and v}
            add(f"disease_drugs {label} groups", "D /disease_drugs",
                len(non_empty) > 0, ms, ">=1 group", str(list(non_empty.keys())[:3]))
            has_db = any(any(str(it.get("entity_id", "")).startswith("DB")
                              for it in v) for v in non_empty.values())
            add(f"disease_drugs {label} DB-ids", "D /disease_drugs",
                has_db, ms, "DB ids", "")

    # E. /symptom_to_disease
    for phens, label in [(["Fever"], "Fever"), (["Headache"], "Headache"),
                         (["Fever", "Cough"], "Fever+Cough")]:
        body = {"phenotype_names": phens, "min_match": len(phens), "limit": 10}
        r, ms = call("/symptom_to_disease", body)
        top = (r.get("items") or [{}])[0].get("name", "")[:35]
        add(f"symptom_to_disease {label}", "E /symptom_to_disease",
            r.get("count", 0) > 0, ms, ">0", f"count={r.get('count')} top={top}")
    r, ms = call("/symptom_to_disease",
                 {"phenotype_names": [], "limit": 5})
    add("symptom_to_disease empty->400", "E /symptom_to_disease",
        "error" in r, ms, "400 error", str(r)[:60])

    # F. /path
    if MET and DM:
        r, ms = call("/path", {"from_index": MET["entity_index"],
                               "to_index": DM["entity_index"],
                               "max_hops": 3, "limit_paths": 3})
        add("path metformin->diabetes", "F /path",
            r.get("count", 0) > 0, ms, ">0", f"count={r.get('count')}")
    if WARF and HTN:
        r, ms = call("/path", {"from_index": WARF["entity_index"],
                               "to_index": HTN["entity_index"],
                               "max_hops": 4, "limit_paths": 3})
        add("path warfarin->HTN clean", "F /path",
            "error" not in r, ms, "no error",
            f"count={r.get('count', 'n/a')}")
    return rows


# ════════ Persistence: dataset/run/scores/summary per suite ════════
def persist(suite_name, agent_name, rows, model_id):
    ds_id = sql(f"SELECT id FROM eval_benchmark_datasets WHERE name={qstr(suite_name)} "
                f"AND tenant_id={qstr(TENANT)} LIMIT 1").strip()
    if not ds_id:
        ds_id = str(uuid.uuid4())
        meta = json.dumps([{"id": r["item_id"], "input": r["input"],
                             "group": r["group"]} for r in rows],
                           ensure_ascii=False)
        desc = f"PrimeKG resolver test — {suite_name} (deterministic API contract regression)"
        sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,"
            "scoring_fn,description,items,total_items,version,is_active) VALUES ("
            + ",".join([qstr(ds_id), qstr(TENANT), qstr(suite_name),
                         qstr("primekg-resolver"), qstr("mcq_accuracy"),
                         qstr(desc), qstr(meta), str(len(rows)), "1", "1"])
            + ")")

    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": model_id,
           "agent": agent_name, "runner": "persist_primekg_resolver_eval",
           "n": len(rows)}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,"
        "completed_combinations,config,tenant_id,variable_under_test) VALUES ("
        + ",".join([qstr(run_id),
                     qstr(f"{suite_name} — {model_id}"),
                     qstr("RUNNING"), str(len(rows)), "0",
                     qstr(json.dumps(cfg)), qstr(TENANT),
                     qstr("mimir-api-version")]) + ")")

    n_pass = 0
    for r in rows:
        sc = 1.0 if r["ok"] else 0.0
        if r["ok"]:
            n_pass += 1
        tags = json.dumps({"group": r["group"], "endpoint_suite": suite_name,
                            "input": r["input"]}, ensure_ascii=False)
        sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,"
            "expected_answer,actual_answer,accuracy_score,latency_ms,"
            "benchmark_item_id,tags,judge_model,tenant_id) VALUES ("
            + ",".join([qstr(run_id), qstr(agent_name), qstr(model_id),
                         qstr(r["input"][:500]), qstr(r["expected"][:200]),
                         qstr(str(r["got"])[:500]), str(sc), str(r["ms"]),
                         qstr(r["item_id"]), qstr(tags),
                         qstr("deterministic-contract"), qstr(TENANT)])
            + ")")

    avg_acc = n_pass / len(rows) if rows else 0
    avg_lat = sum(r["ms"] for r in rows) / len(rows) if rows else 0
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,"
        "avg_accuracy,avg_latency_ms,overall_score,unsafe_count,tenant_id) VALUES ("
        + ",".join([qstr(run_id), qstr(agent_name), qstr(model_id),
                     str(len(rows)), str(round(avg_acc, 4)),
                     str(round(avg_lat, 1)), str(round(avg_acc, 4)),
                     "0", qstr(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', "
        f"completed_combinations={len(rows)}, finished_at=NOW() "
        f"WHERE id={qstr(run_id)}")
    return run_id, n_pass, avg_acc


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", help="mimir-api version tag; auto-detect if omitted")
    args = ap.parse_args()
    version = args.version or detect_version()
    model_id = f"asgard-mimir-api:{version}"
    print(f"# model under test: {model_id}", file=sys.stderr)

    # Register the API version as a model (FK requirement)
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES ("
        + ",".join([qstr(model_id), qstr("mimir-api"), qstr("service"),
                     "1", qstr(json.dumps({"kind": "api-version"}))])
        + ") ON DUPLICATE KEY UPDATE updated_at=NOW()")

    suites = [
        ("primekg-resolver-bug-class",     "primekg-bug-class",          suite_bug_class),
        ("primekg-disease-relations",      "primekg-disease-relations",  suite_disease_relations),
        ("primekg-resolver-fuzzy-typos",   "primekg-fuzzy-typos",        suite_fuzzy_typos),
        ("primekg-full-coverage",          "primekg-full-coverage",      suite_full_coverage),
    ]
    print(f"{'suite':<32} {'cases':<7} {'pass':<6} {'acc':<7} run_id")
    print("-" * 90)
    for name, agent, fn in suites:
        rows = fn()
        run_id, n_pass, avg_acc = persist(name, agent, rows, model_id)
        print(f"{name:<32} {len(rows):<7} {n_pass:<6} {avg_acc*100:>5.1f}% {run_id}")


if __name__ == "__main__":
    sys.exit(main())
