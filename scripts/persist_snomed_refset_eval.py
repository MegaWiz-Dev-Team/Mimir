#!/usr/bin/env python3
"""B4 — persist Sprint 58 SNOMED refset + dose-link coverage as a Mimir eval run.

Records the ingest/coverage metrics (refset member counts, EDQM map size, and the
TMT→SNOMED dose-link tier breakdown) into the standard eval tables so coverage is
tracked over time and a regression (e.g. a re-ingest that drops trusted links) is
visible. Mirrors scripts/persist_primekg_resolver_eval.py.

Each "case" is a coverage assertion (metric ≥ expected floor) → pass/fail, so the
eval surfaces both the live number and whether it regressed below the baseline.

Run after every dose-link / refset re-ingest:
  python3 scripts/persist_snomed_refset_eval.py
"""
from __future__ import annotations
import argparse
import json
import subprocess
import sys
import uuid

TENANT = "asgard_platform"   # PII-free benchmark tenant (mirrors primekg eval)
INFRA_NS = "asgard-infra"

# Coverage floors — the run fails a case if the live count drops below these.
# Set just under the 2026-06-02 actuals so a real regression trips but normal
# re-ingest of the same release stays green.
BASELINE = {
    "ips_refset_members": 12000,
    "gpfp_refset_members": 4000,
    "edqm_dose_maps": 300,
    "dose_links_trusted": 8000,
}


def sql(q: str) -> str:
    r = subprocess.run(
        ["kubectl", "-n", INFRA_NS, "exec", "-i", "deploy/mariadb", "--",
         "mariadb", "-uroot", "-proot", "--default-character-set=utf8mb4",
         "mimir", "-B", "-N", "-e", q],
        capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.decode()[:300])
    return r.stdout.decode("utf-8").strip()


def qstr(s) -> str:
    if s is None:
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def count(q: str) -> int:
    out = sql(q)
    return int(out) if out else 0


def gather() -> list[dict]:
    metrics = {
        "ips_refset_members": count(
            "SELECT COUNT(*) FROM snomed_refset_members WHERE tenant_id IS NULL AND refset_key='ips'"),
        "gpfp_refset_members": count(
            "SELECT COUNT(*) FROM snomed_refset_members WHERE tenant_id IS NULL AND refset_key='gpfp'"),
        "edqm_dose_maps": count(
            "SELECT COUNT(*) FROM snomed_edqm_dose_map WHERE tenant_id IS NULL"),
        "dose_links_trusted": count(
            "SELECT COUNT(*) FROM snomed_tmt_dose_link WHERE needs_review=0"),
    }
    rows = []
    for k, floor in BASELINE.items():
        live = metrics[k]
        rows.append({
            "item_id": k, "group": "coverage", "input": f"{k} >= {floor}",
            "expected": f">={floor}", "got": live, "ok": live >= floor, "ms": 0,
        })
    # informational tier breakdown (not pass/fail, recorded as got)
    for method in ("curated", "exact", "normalized", "token_subset"):
        n = count(f"SELECT COUNT(*) FROM snomed_tmt_dose_link WHERE match_method='{method}'")
        rows.append({
            "item_id": f"dose_link_{method}", "group": "tier", "input": f"dose link tier {method}",
            "expected": "info", "got": n, "ok": True, "ms": 0,
        })
    return rows


def persist(suite: str, agent: str, rows: list[dict], model_id: str):
    ds_id = sql(f"SELECT id FROM eval_benchmark_datasets WHERE name={qstr(suite)} "
                f"AND tenant_id={qstr(TENANT)} LIMIT 1")
    if not ds_id:
        ds_id = str(uuid.uuid4())
        meta = json.dumps([{"id": r["item_id"], "input": r["input"], "group": r["group"]}
                           for r in rows], ensure_ascii=False)
        sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,scoring_fn,"
            "description,items,total_items,version,is_active) VALUES ("
            + ",".join([qstr(ds_id), qstr(TENANT), qstr(suite), qstr("snomed-refset"),
                        qstr("threshold"), qstr("Sprint 58 SNOMED refset + dose-link coverage"),
                        qstr(meta), str(len(rows)), "1", "1"]) + ")")
    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": model_id, "agent": agent,
           "runner": "persist_snomed_refset_eval", "n": len(rows)}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,"
        "config,tenant_id,variable_under_test) VALUES ("
        + ",".join([qstr(run_id), qstr(f"{suite} — {model_id}"), qstr("RUNNING"),
                    str(len(rows)), "0", qstr(json.dumps(cfg)), qstr(TENANT),
                    qstr("snomed-refset-ingest")]) + ")")
    n_pass = 0
    for r in rows:
        sc = 1.0 if r["ok"] else 0.0
        n_pass += 1 if r["ok"] else 0
        tags = json.dumps({"group": r["group"], "input": r["input"]}, ensure_ascii=False)
        sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,"
            "actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,judge_model,"
            "tenant_id) VALUES ("
            + ",".join([qstr(run_id), qstr(agent), qstr(model_id), qstr(r["input"][:500]),
                        qstr(r["expected"][:200]), qstr(str(r["got"])[:500]), str(sc),
                        str(r["ms"]), qstr(r["item_id"]), qstr(tags),
                        qstr("threshold"), qstr(TENANT)]) + ")")
    graded = [r for r in rows if r["group"] == "coverage"]
    avg = sum(1 for r in graded if r["ok"]) / len(graded) if graded else 0
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,avg_accuracy,"
        "avg_latency_ms,overall_score,unsafe_count,tenant_id) VALUES ("
        + ",".join([qstr(run_id), qstr(agent), qstr(model_id), str(len(rows)),
                    str(round(avg, 4)), "0", str(round(avg, 4)), "0", qstr(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={len(rows)}, "
        f"finished_at=NOW() WHERE id={qstr(run_id)}")
    return run_id, n_pass, avg


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="sprint58")
    args = ap.parse_args()
    model_id = f"asgard-mimir-kb:{args.version}"
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES ("
        + ",".join([qstr(model_id), qstr("mimir-kb"), qstr("service"), "1",
                    qstr(json.dumps({"kind": "kb-ingest"}))])
        + ") ON DUPLICATE KEY UPDATE updated_at=NOW()")
    rows = gather()
    run_id, n_pass, avg = persist("snomed-refset-coverage", "snomed-refset", rows, model_id)
    print(f"{'metric':<26} {'got':>8}  status")
    for r in rows:
        mark = "ok" if r["ok"] else ("FAIL" if r["group"] == "coverage" else "info")
        print(f"{r['item_id']:<26} {str(r['got']):>8}  {mark}")
    cov = [r for r in rows if r["group"] == "coverage"]
    cov_pass = sum(1 for r in cov if r["ok"])
    print(f"\ncoverage cases: {cov_pass}/{len(cov)} pass, accuracy={avg:.2%}, run_id={run_id}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
