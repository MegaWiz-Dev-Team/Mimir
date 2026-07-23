#!/usr/bin/env python3
"""
Register the drug-disease-resolution-v1 benchmark in eval_benchmark_datasets
(tenant asgard_medical). Mirrors register_eval_dataset_m1.py.

NEW dataset family (distinct from thai-disease-normalize): measures how often the
naive PrimeKG resolver (primekg_lookup_entity) finds the INTENDED node for a
drug/disease name as an LLM/clinician writes it. Scoring = resolution_recall
(match-rank buckets: exact > prefix > substr(SUSPECT) > miss) + labeled correctness.

PREPARED, NOT APPLIED. Applying writes to the asgard_medical prod DB — do it via
the gated path in APPLY.md (backup first, run from a Mimir git worktree).

Run (after placing under Mimir/scripts/ and dataset under Mimir/tests/):
    /opt/homebrew/bin/python3 scripts/register_eval_dataset_resolution.py
"""
from __future__ import annotations
import hashlib
import json
import subprocess
import sys
import uuid
from collections import Counter
from pathlib import Path

NAMESPACE = "asgard-infra"
TENANT_ID = "asgard_medical"
REPO_ROOT = Path(__file__).resolve().parents[1]
DATASET_DIR = REPO_ROOT / "tests" / "eval_datasets" / "drug_disease_resolution" / "v1.0"
PROBE_PATH = DATASET_DIR / "probe.jsonl"


def mariadb_pod() -> str:
    # Resolve the pod dynamically — pod names rotate; the m1 script hardcoded a
    # now-stale one. Fail loudly if none found.
    r = subprocess.run(
        ["kubectl", "get", "pods", "-n", NAMESPACE, "-o", "name"],
        capture_output=True, text=True,
    )
    for line in r.stdout.splitlines():
        if "mariadb" in line:
            return line.split("/")[-1]
    print("no mariadb pod found in " + NAMESPACE, file=sys.stderr)
    sys.exit(2)


def sql_escape(s: str | None) -> str:
    if s is None:
        return "NULL"
    s = s.replace("\\", "\\\\").replace("'", "\\'")
    return f"'{s}'"


def run_mariadb(sql_stdin: str) -> tuple[int, str]:
    cmd = [
        "kubectl", "exec", "-n", NAMESPACE, mariadb_pod(), "-i", "--",
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
    if not PROBE_PATH.exists():
        print(f"missing: {PROBE_PATH}", file=sys.stderr)
        return 1

    rows = []
    with PROBE_PATH.open() as f:
        for line in f:
            line = line.strip()
            if line:
                rows.append(json.loads(line))

    n = len(rows)
    by_type = Counter(r["type"] for r in rows)
    by_locale = Counter(r.get("locale", "?") for r in rows)
    n_gold = sum(1 for r in rows if r.get("gold"))

    descriptor = {
        "kind": "drug_disease_name_resolution",
        "version": "v1.0",
        "files": {
            "probe": {
                "path": str(PROBE_PATH.relative_to(REPO_ROOT)),
                "rows": n,
                "sha256": file_sha256(PROBE_PATH),
            },
        },
        "distribution": {
            "by_type": dict(by_type),
            "by_locale": dict(by_locale),
            "gold_labeled": n_gold,
        },
        "scoring": {
            "fn": "resolution_recall",
            "buckets": ["exact", "prefix", "substr(SUSPECT)", "miss"],
            "headline_metrics": ["exact_rate_per_type", "labeled_correctness_pct"],
            "note": "exact(rank0)=trustworthy; substr(rank2)=wrong-node risk; "
                    "'returned anything' overstates recall — report exact floor.",
        },
        "provenance": {
            "terms": "authored realistic clinical vocabulary (generic/brand/lay). "
                     "NOT sampled from PrimeKG (that would be exact by construction).",
            "license_note": "DDInter/DrugBank interaction GT is benchmark-only and "
                            "MUST NOT be stored here or in the product KG. This dataset "
                            "holds only names + expected canonical nodes.",
        },
        "baseline_result": "see tests/eval_datasets/drug_disease_resolution/v1.0/"
                           "results_baseline.json (naive resolver, prototype, 2026-07-23)",
    }

    ns = uuid.UUID("00000000-0000-0000-0000-000000000048")
    ds_id = str(uuid.uuid5(ns, "drug_disease_resolution/v1.0"))

    sql = (
        "INSERT INTO eval_benchmark_datasets "
        "(id, tenant_id, name, source, scoring_fn, description, items, "
        "total_items, version, is_active) "
        f"VALUES ({sql_escape(ds_id)}, {sql_escape(TENANT_ID)}, "
        f"{sql_escape('drug-disease-resolution-v1')}, "
        f"{sql_escape('drug_disease_resolution_v1')}, "
        f"{sql_escape('resolution_recall')}, "
        f"{sql_escape('Measures naive PrimeKG resolver (primekg_lookup_entity) recall on drug/disease names as written (generic/brand/lay). Buckets: exact>prefix>substr(SUSPECT)>miss. Baseline 2026-07-23: drug exact 73.3%/miss 24.4%, disease exact 45%/prefix 35%, labeled 9/13=69%. Regression gate for the drug/disease NORMALIZER (RxNorm brand->ingredient + DrugBank synonyms + TMT).')}, "
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

    print("=== drug-disease-resolution-v1 registration ===")
    print(f"  id:          {ds_id}")
    print(f"  rows:        {n}  (gold-labeled: {n_gold})")
    print(f"  scoring_fn:  resolution_recall")
    print(f"  tenant:      {TENANT_ID}")
    print(f"  by_type:     {dict(by_type)}")

    verify = (
        "SELECT id, name, source, scoring_fn, total_items, is_active "
        f"FROM eval_benchmark_datasets WHERE id = {sql_escape(ds_id)};\n"
    )
    rc, out = run_mariadb(verify)
    print("\n--- DB verification ---")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
