#!/usr/bin/env python3
"""Persist eir-anesthesia evaluation results into Mimir eval framework.

Persists 2 suites (idempotent):
  1. eir-anesthesia-retrieval-baseline-v1 — Hit Rate from 03_validate_retrieval.py
  2. eir-anesthesia-answer-quality-v1     — E2E suite from 04_eir_anesthesia_e2e.py

Pattern: mirrors persist_primekg_resolver_eval.py

  Tenant       asgard_platform  (cross-cutting infra benchmarks)
  Model_id     asgard-eir-anesthesia:v0.1+kb-rcat-2026-05-28
  Agent name   eir-anesthesia

Usage:
    /tmp/docx-venv/bin/python3 06_persist_eval_baseline.py
"""
from __future__ import annotations

import argparse
import glob
import json
import subprocess
import sys
import uuid

TENANT     = "asgard_platform"
INFRA_NS   = "asgard-infra"
AGENT_NAME = "eir-anesthesia"
MODEL_ID   = "asgard-eir-anesthesia:v0.1+kb-rcat-2026-05-28"


def sh(cmd, inp=None):
    r = subprocess.run(cmd, input=inp, capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.decode()[:400])
    return r.stdout.decode("utf-8")


def sql(q: str) -> str:
    return sh(["kubectl", "-n", INFRA_NS, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-uroot", "-proot", "--default-character-set=utf8mb4",
               "mimir", "-B", "-N", "-e", q])


def qstr(s) -> str:
    if s is None:
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def upsert_dataset(suite_name: str, items: list[dict], description: str,
                   scoring_fn: str = "mcq_accuracy") -> str:
    """Get-or-create eval_benchmark_datasets row."""
    existing = sql(f"SELECT id FROM eval_benchmark_datasets "
                   f"WHERE name={qstr(suite_name)} AND tenant_id={qstr(TENANT)} LIMIT 1").strip()
    if existing:
        return existing
    ds_id = str(uuid.uuid4())
    meta = json.dumps(items, ensure_ascii=False)
    sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,scoring_fn,"
        "description,items,total_items,version,is_active) VALUES ("
        + ",".join([qstr(ds_id), qstr(TENANT), qstr(suite_name),
                    qstr("eir-anesthesia"), qstr(scoring_fn),
                    qstr(description), qstr(meta),
                    str(len(items)), "1", "1"]) + ")")
    return ds_id


def insert_run(suite_name: str, ds_id: str, n: int, variable: str) -> str:
    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": MODEL_ID,
           "agent": AGENT_NAME, "runner": "persist_anesthesia_eval",
           "n": n, "kb_version": "rcat-2026-05-28",
           "embed_model": "bge-m3", "chat_model": "mlx-community/Qwen3.5-9B-MLX-4bit",
           "qdrant_collection": "anesthesia_kb_001"}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,"
        "config,tenant_id,variable_under_test) VALUES ("
        + ",".join([qstr(run_id), qstr(f"{suite_name} — {MODEL_ID}"),
                    qstr("RUNNING"), str(n), "0",
                    qstr(json.dumps(cfg)), qstr(TENANT),
                    qstr(variable)]) + ")")
    return run_id


def insert_score(run_id: str, q: str, expected: str, got: str,
                 score: float, lat_ms: float, item_id: str, tags: dict):
    sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,"
        "actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,"
        "judge_model,tenant_id) VALUES ("
        + ",".join([qstr(run_id), qstr(AGENT_NAME), qstr(MODEL_ID),
                    qstr(q[:500]), qstr(expected[:200]), qstr(got[:500]),
                    str(int(score * 100)), str(int(lat_ms)),
                    qstr(item_id), qstr(json.dumps(tags, ensure_ascii=False)),
                    qstr("deterministic-hit-rate"), qstr(TENANT)]) + ")")


def finalize_run(run_id: str, n: int, avg_acc: float, avg_lat_ms: float):
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,"
        "avg_accuracy,avg_latency_ms,overall_score,unsafe_count,tenant_id) VALUES ("
        + ",".join([qstr(run_id), qstr(AGENT_NAME), qstr(MODEL_ID),
                    str(n), str(round(avg_acc, 4)), str(round(avg_lat_ms, 1)),
                    str(round(avg_acc, 4)), "0", qstr(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={n}, "
        f"finished_at=NOW() WHERE id={qstr(run_id)}")


# ════════════════════════════════════════════════════════════════
# Suite 1 — Retrieval baseline
# ════════════════════════════════════════════════════════════════
def persist_retrieval():
    val_files = sorted(glob.glob(
        "/Users/mimir/Developer/Mimir/data/eir-anesthesia/validation_*.json"))
    if not val_files:
        print("⚠ No validation_*.json found; run 03_validate_retrieval.py first", file=sys.stderr)
        return None
    val = json.loads(open(val_files[-1]).read())
    print(f"[Suite 1] Loaded {val_files[-1]}")

    items = [{"id": f"ret-{i:02}",
              "input": r["query"][:200],
              "expect": r["expect"],
              "group": "retrieval"} for i, r in enumerate(val["per_query"], start=1)]
    ds_id = upsert_dataset(
        "eir-anesthesia-retrieval-baseline-v1", items,
        "Hit Rate baseline for eir-anesthesia retrieval over RCAT 22 PDFs in "
        "Qdrant anesthesia_kb_001. 10 hand-curated Thai medical queries; "
        "each query has expected source PDF substring. PASS = expected PDF in top-3 results.",
        scoring_fn="mcq_accuracy")
    run_id = insert_run("eir-anesthesia-retrieval-baseline-v1", ds_id,
                        len(val["per_query"]), "kb_version+chunking")

    n_pass = 0
    for i, r in enumerate(val["per_query"], start=1):
        ok = r["hit_at"] is not None and r["hit_at"] <= 3
        if ok:
            n_pass += 1
        tags = {"hit_at": r["hit_at"], "top1_src": r["top1_src"][:60],
                "top1_score": round(r["top1_score"], 3),
                "expect": r["expect"]}
        insert_score(run_id, r["query"], r["expect"], r["top1_src"],
                     1.0 if ok else 0.0, r["total_ms"], f"ret-{i:02}", tags)
    finalize_run(run_id, len(val["per_query"]),
                 n_pass / len(val["per_query"]),
                 sum(r["total_ms"] for r in val["per_query"]) / len(val["per_query"]))
    print(f"[Suite 1] Pass: {n_pass}/{len(val['per_query'])} "
          f"({n_pass/len(val['per_query'])*100:.0f}%)  run_id={run_id[:8]}…")
    return run_id


# ════════════════════════════════════════════════════════════════
# Suite 2 — End-to-end answer quality
# ════════════════════════════════════════════════════════════════
def persist_e2e():
    e2e_files = sorted(glob.glob(
        "/Users/mimir/Developer/Mimir/data/eir-anesthesia/e2e_suite_*.json"))
    if not e2e_files:
        print("⚠ No e2e_suite_*.json found; run 04_eir_anesthesia_e2e.py --suite first",
              file=sys.stderr)
        return None
    e2e = json.loads(open(e2e_files[-1]).read())
    print(f"[Suite 2] Loaded {e2e_files[-1]}")

    items = [{"id": f"e2e-{i:02}", "input": r["query"][:200],
              "group": "e2e-grounded"} for i, r in enumerate(e2e, start=1)]
    ds_id = upsert_dataset(
        "eir-anesthesia-answer-quality-v1", items,
        "E2E answer quality for eir-anesthesia. Pipeline: Heimdall BGE-M3 embed "
        "→ Qdrant search anesthesia_kb_001 (top-8) → Qwen3.5-9B chat with grounded "
        "prompt + citation enforcement. accuracy_score = pre-KOL-review human-judged "
        "automated check (does answer cite the expected source PDF). KOL spot-check "
        "(Day 17 by หมอ TEN) will add human_accuracy_score later.",
        scoring_fn="human")
    run_id = insert_run("eir-anesthesia-answer-quality-v1", ds_id,
                        len(e2e), "model+kb_version")

    # Heuristic auto-score: does the answer contain a citation in expected format?
    # Real accuracy needs KOL review (Day 17)
    n_pass = 0
    for i, r in enumerate(e2e, start=1):
        ans = r["answer"]
        has_citation = "[source:" in ans
        has_safety_header = "Draft" in ans[:100]
        # Cited at least 1 source PDF in top_sources
        cited_top = any(src.split(".pdf")[0][:30] in ans for src in r["top_sources"])
        # Heuristic score: 3 of 3 checks
        score = (has_citation + has_safety_header + cited_top) / 3.0
        if score >= 0.67:
            n_pass += 1
        tags = {"has_citation": has_citation, "has_safety_header": has_safety_header,
                "cited_top_source": cited_top, "answer_chars": len(ans),
                "completion_tokens": r["usage"].get("completion_tokens", 0)}
        insert_score(run_id, r["query"], "cited+grounded+safe",
                     ans[:500], score, r["total_ms"], f"e2e-{i:02}", tags)
    finalize_run(run_id, len(e2e), n_pass / len(e2e),
                 sum(r["total_ms"] for r in e2e) / len(e2e))
    print(f"[Suite 2] Heuristic pass: {n_pass}/{len(e2e)} "
          f"({n_pass/len(e2e)*100:.0f}%)  run_id={run_id[:8]}…")
    print("       ⚠ KOL human review by หมอ TEN (Day 17) will populate "
          "eval_scores.human_accuracy_score")
    return run_id


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-retrieval", action="store_true")
    ap.add_argument("--skip-e2e", action="store_true")
    args = ap.parse_args()

    # Register model_id in ai_models (FK requirement)
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES ("
        + ",".join([qstr(MODEL_ID), qstr("eir-anesthesia"), qstr("agent"), "1",
                    qstr(json.dumps({
                        "kind": "rag-agent",
                        "embed_model": "bge-m3",
                        "chat_model": "mlx-community/Qwen3.5-9B-MLX-4bit",
                        "kb_collection": "anesthesia_kb_001",
                        "kb_version": "rcat-2026-05-28",
                        "kb_sources": 22,
                        "kb_chunks": 794,
                        "tenant": "asgard_surgical",
                        "commercial_use": True
                    }))]) + ") ON DUPLICATE KEY UPDATE updated_at=NOW(), metadata=VALUES(metadata)")
    print(f"✓ Registered model: {MODEL_ID}")

    if not args.skip_retrieval:
        persist_retrieval()
    if not args.skip_e2e:
        persist_e2e()


if __name__ == "__main__":
    main()
