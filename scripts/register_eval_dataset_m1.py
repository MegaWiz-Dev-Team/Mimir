#!/usr/bin/env python3
"""
Sprint 2 W2.1 — register M1 medical retrieval dataset in eval_benchmark_datasets.

Mirrors the I1/I2 registration pattern (W2.1c). M1 is the Mimir medical
retrieval benchmark — 75 hand-curated TH/EN queries spanning drug names,
diseases, drug interactions, sleep procedures, and clinical scenarios.

Run:
    /opt/homebrew/bin/python3 scripts/register_eval_dataset_m1.py
"""
from __future__ import annotations
import hashlib
import json
import subprocess
import sys
import uuid
from collections import Counter
from pathlib import Path

MARIADB_POD = "mariadb-585d5cd485-fwmjh"
NAMESPACE = "asgard-infra"
TENANT_ID = "asgard_medical"  # M1 = medical, not insurance
REPO_ROOT = Path(__file__).resolve().parents[1]
DATASET_DIR = REPO_ROOT / "tests" / "eval_datasets" / "m1" / "v1.0"
QUERIES_PATH = DATASET_DIR / "queries.jsonl"


def sql_escape(s: str | None) -> str:
    if s is None:
        return "NULL"
    s = s.replace("\\", "\\\\").replace("'", "\\'")
    return f"'{s}'"


def run_mariadb(sql_stdin: str) -> tuple[int, str]:
    cmd = [
        "kubectl", "exec", "-n", NAMESPACE, MARIADB_POD, "-i", "--",
        "bash", "-c", 'mariadb -uroot -p"$MYSQL_ROOT_PASSWORD" mimir',
    ]
    proc = subprocess.run(cmd, input=sql_stdin, capture_output=True, text=True)
    return proc.returncode, (proc.stdout + proc.stderr)


def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    if not QUERIES_PATH.exists():
        print(f"missing: {QUERIES_PATH}", file=sys.stderr)
        return 1

    # Load + summarize
    queries = []
    with QUERIES_PATH.open() as f:
        for line in f:
            line = line.strip()
            if line:
                queries.append(json.loads(line))

    n = len(queries)
    by_category = Counter(q["category"] for q in queries)
    by_locale = Counter(q["locale"] for q in queries)
    by_difficulty = Counter(q.get("difficulty", "?") for q in queries)

    descriptor = {
        "kind": "medical_retrieval_queries",
        "version": "v1.0",
        "files": {
            "queries": {
                "path": str(QUERIES_PATH.relative_to(REPO_ROOT)),
                "rows": n,
                "sha256": file_sha256(QUERIES_PATH),
            },
        },
        "distribution": {
            "by_category": dict(by_category),
            "by_locale": dict(by_locale),
            "by_difficulty": dict(by_difficulty),
        },
        "schema_hint": (
            "Each query has 'expected_*' fields naming the gold answer "
            "(drug generic names, ICD-10 codes, concepts, etc.). Hit@K = "
            "any expected entity in top-K AND no expected_NOT entry in top-K."
        ),
        "scoring": {
            "fn": "hit_rate_at_k",
            "k": 3,
            "decision_gates": {
                ">=75%": "adopt BGE-M3 + current chunking",
                "60-75%": "run hybrid (BGE-M3 + sparse exact-match) + benchmark",
                "<60%": "trigger fine-tune plan (per ner_finetune_plan memory)",
            },
            "subgate_thai": "Thai subset must be within 5pp of EN subset",
        },
        "out_of_scope_v1.0": [
            "M4 PrimeKG entity-id ground truth (separate dataset)",
            "Multi-turn / conversational queries",
            "Patient-context-aware retrieval",
            "Image-based queries",
        ],
    }

    # Stable uuid5
    ns = uuid.UUID("00000000-0000-0000-0000-000000000048")
    ds_id = str(uuid.uuid5(ns, "m1_medical_retrieval/v1.0"))

    sql = (
        "INSERT INTO eval_benchmark_datasets "
        "(id, tenant_id, name, source, scoring_fn, description, items, "
        "total_items, version, is_active) "
        f"VALUES ({sql_escape(ds_id)}, {sql_escape(TENANT_ID)}, "
        f"{sql_escape('Medical Retrieval Benchmark — M1 v1.0 (TH+EN)')}, "
        f"{sql_escape('m1_medical_retrieval_v1')}, "
        f"{sql_escape('mcq_accuracy')}, "
        f"{sql_escape('Sprint 2 W2.1. 75 hand-curated TH+EN medical retrieval queries spanning drug names, diseases, drug interactions, sleep procedures, clinical scenarios. Each query has expected gold answers (drug generics, ICD-10 codes, or concepts). Hit Rate@3 gate: ≥75% adopt, 60-75% hybrid, <60% fine-tune. Thai subset must be within 5pp of EN.')}, "
        f"{sql_escape(json.dumps(descriptor, ensure_ascii=False))}, "
        f"{n}, 1, 1) "
        "ON DUPLICATE KEY UPDATE "
        "name = VALUES(name), description = VALUES(description), "
        "items = VALUES(items), total_items = VALUES(total_items), "
        "is_active = VALUES(is_active);\n"
    )

    rc, out = run_mariadb(sql)
    if rc != 0:
        print(f"INSERT failed: {out}", file=sys.stderr)
        return 1

    print(f"=== W2.1 — M1 dataset registration ===")
    print(f"  id:            {ds_id}")
    print(f"  rows:          {n}")
    print(f"  scoring_fn:    mcq_accuracy (Hit Rate@K)")
    print(f"  tenant:        asgard_medical")
    print()
    print(f"By category:")
    for cat, c in by_category.most_common():
        print(f"  {cat:25s}  {c}")
    print()
    print(f"By locale:")
    for loc, c in by_locale.most_common():
        print(f"  {loc:10s}  {c}")

    # Verify
    verify = (
        "SELECT id, name, source, scoring_fn, total_items, version, is_active "
        f"FROM eval_benchmark_datasets WHERE id = {sql_escape(ds_id)};\n"
    )
    rc, out = run_mariadb(verify)
    print()
    print("--- DB verification ---")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
