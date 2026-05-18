#!/usr/bin/env python3
"""
Sprint 2 W2.1c — register I1 + I2 synthetic datasets in eval_benchmark_datasets.

W2.1b shipped the synthetic Thai applicant generator. This script registers
its output as evaluation datasets in Mimir's `eval_benchmark_datasets` table
so they appear in https://mimir.asgard.internal/evaluations.

Two datasets:

  I1 — Underwriter Regression (synthetic Thai applicants)
       Source: scripts/synthetic_thai_applicants --seed 42 --applicants 1000
       scoring_fn: composite (custom — deterministic risk-score equality)
       NOTE: scoring_fn is stored as 'paper_rubric_pct' for now (closest enum
       value) until a custom composite scorer lands in Mimir eval pipeline.

  I2 — Fraud Detection (synthetic Thai claims, correlated fraud)
       Source: scripts/synthetic_thai_applicants --seed 42 --claims 500
       scoring_fn: binary_yes_no (is_canonical_fraud Y/N)

Datasets are loaded with **items = JSON descriptor** (regeneration command +
file refs) rather than embedding full data inline. The full canonical
content lives in `tests/eval_datasets/i{1,2}/v1.0/*.jsonl`. Rationale:
- Generator is deterministic per seed → file content is the source of truth
- Descriptor stays small (< 1KB) so DB row inserts/updates are cheap
- Eval runner can read files when needed; descriptor tells it where

Requires:
  - kubectl exec into mariadb pod (asgard-infra namespace)
  - Migration sprint48_icd10_codes.sql already applied (creates the deps)
  - Optionally sprint40_multi_benchmark.sql (adds scoring_fn column)

Run:
  /opt/homebrew/bin/python3 scripts/register_eval_datasets_i1_i2.py
"""
from __future__ import annotations
import argparse
import hashlib
import json
import subprocess
import sys
import uuid
from pathlib import Path

MARIADB_POD = "mariadb-585d5cd485-fwmjh"
NAMESPACE = "asgard-infra"
TENANT_ID = "asgard_insurance"  # I1/I2 are insurance datasets
REPO_ROOT = Path(__file__).resolve().parents[1]


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


def count_jsonl(path: Path) -> int:
    return sum(1 for _ in path.open())


def upsert_dataset(
    name: str,
    source: str,
    scoring_fn: str,
    description: str,
    items_descriptor: dict,
    total_items: int,
    version: int = 1,
) -> str:
    """INSERT ... ON DUPLICATE KEY UPDATE — idempotent registration.
    Returns the dataset id (UUID stable per source+version)."""
    # Deterministic id from source+version so re-running this script
    # doesn't create duplicate rows. uuid5 with a stable namespace.
    ns = uuid.UUID("00000000-0000-0000-0000-000000000048")  # sprint 48 namespace
    ds_id = str(uuid.uuid5(ns, f"{source}/v{version}"))

    items_json = json.dumps(items_descriptor, ensure_ascii=False)

    # MariaDB doesn't have ON CONFLICT; use INSERT ... ON DUPLICATE KEY UPDATE
    sql = (
        "INSERT INTO eval_benchmark_datasets "
        "(id, tenant_id, name, source, scoring_fn, description, items, total_items, version, is_active) "
        f"VALUES ({sql_escape(ds_id)}, {sql_escape(TENANT_ID)}, "
        f"{sql_escape(name)}, {sql_escape(source)}, "
        f"{sql_escape(scoring_fn)}, {sql_escape(description)}, "
        f"{sql_escape(items_json)}, {total_items}, {version}, 1) "
        "ON DUPLICATE KEY UPDATE "
        "name = VALUES(name), "
        "scoring_fn = VALUES(scoring_fn), "
        "description = VALUES(description), "
        "items = VALUES(items), "
        "total_items = VALUES(total_items), "
        "is_active = VALUES(is_active);\n"
    )
    rc, out = run_mariadb(sql)
    if rc != 0:
        print(f"INSERT failed for {name}: {out}", file=sys.stderr)
        sys.exit(1)
    return ds_id


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset-dir", type=Path,
                    default=REPO_ROOT / "tests" / "eval_datasets" / "i1" / "v1.0",
                    help="Where generated JSONL files live")
    ap.add_argument("--version", type=int, default=1)
    args = ap.parse_args()

    applicants_path = args.dataset_dir / "applicants.jsonl"
    medical_path = args.dataset_dir / "medical_records.jsonl"
    claims_path = args.dataset_dir / "claims.jsonl"

    for p in (applicants_path, medical_path, claims_path):
        if not p.exists():
            print(f"missing: {p}", file=sys.stderr)
            print("Regenerate with:", file=sys.stderr)
            print(f"  PYTHONPATH=scripts /opt/homebrew/bin/python3 -m synthetic_thai_applicants "
                  f"--applicants 1000 --claims 500 --seed 42 --output {args.dataset_dir}",
                  file=sys.stderr)
            return 1

    # ─── I1 — Underwriter Regression ──────────────────────────────────────
    n_applicants = count_jsonl(applicants_path)
    n_medical = count_jsonl(medical_path)

    i1_descriptor = {
        "kind": "synthetic_thai_applicants",
        "version": "v1.0",
        "seed": 42,
        "generator": "scripts/synthetic_thai_applicants",
        "regenerate_cmd": (
            "PYTHONPATH=scripts /opt/homebrew/bin/python3 -m synthetic_thai_applicants "
            "--applicants 1000 --claims 500 --seed 42 "
            f"--output {args.dataset_dir.relative_to(REPO_ROOT)}"
        ),
        "files": {
            "applicants": {
                "path": str(applicants_path.relative_to(REPO_ROOT)),
                "rows": n_applicants,
                "sha256": file_sha256(applicants_path),
            },
            "medical_records": {
                "path": str(medical_path.relative_to(REPO_ROOT)),
                "rows": n_medical,
                "sha256": file_sha256(medical_path),
            },
        },
        "scoring": {
            "fn": "composite_regression",
            "rule": "expected_risk_score = pipeline(applicant, medical_record). "
                    "Tolerance ±5% per case. Drift detection: if >5% of cases "
                    "fall outside tolerance, fail regression.",
        },
        "decision_gate": "0% drift after Phase B trait refactor → trait refactor accepted",
    }

    i1_id = upsert_dataset(
        name="Underwriter Regression — Synthetic Thai Applicants v1.0",
        source="synthetic_thai_applicants_v1_seed42_applicants",
        scoring_fn="paper_rubric_pct",  # placeholder; custom composite TODO
        description=(
            "Sprint 2 I1. Synthetic 1000 Thai insurance applicants + matching "
            "medical records, generated deterministically at seed=42. Each applicant "
            "is fed through the Underwriter v3 pipeline; the recorded risk score is "
            "the expected output for regression. Drift > 5% across cases = fail. "
            "Tests determinism after refactors (Phase B trait refactor, Phase C tool "
            "catalog wiring). Source of truth: tests/eval_datasets/i1/v1.0/."
        ),
        items_descriptor=i1_descriptor,
        total_items=n_applicants,
        version=args.version,
    )

    # ─── I2 — Fraud Detection ─────────────────────────────────────────────
    # Reuses the same claims.jsonl produced by the same generator run.
    n_claims = count_jsonl(claims_path)

    # Count canonical-fraud labels for quick stats.
    n_canonical_fraud = 0
    for line in claims_path.open():
        try:
            row = json.loads(line)
            if row.get("is_canonical_fraud"):
                n_canonical_fraud += 1
        except Exception:
            pass

    i2_descriptor = {
        "kind": "synthetic_thai_claims_fraud",
        "version": "v1.0",
        "seed": 42,
        "generator": "scripts/synthetic_thai_applicants",
        "regenerate_cmd": i1_descriptor["regenerate_cmd"],  # same run
        "files": {
            "claims": {
                "path": str(claims_path.relative_to(REPO_ROOT)),
                "rows": n_claims,
                "sha256": file_sha256(claims_path),
            },
        },
        "label_field": "is_canonical_fraud",
        "label_distribution": {
            "fraud": n_canonical_fraud,
            "non_fraud": n_claims - n_canonical_fraud,
            "fraud_rate": round(n_canonical_fraud / n_claims, 3) if n_claims else 0.0,
        },
        "fraud_rules": [
            "short_policy_high_amount (+3): days_since<90 AND amount>500K THB",
            "very_short_policy (+2): days_since<30",
            "amount_near_limit (+2): amount/policy_limit > 0.8",
            "under_investigation (+2): status == 'Under Investigation'",
            "repeat_claimant_3plus (+1): 3+ claims same applicant",
        ],
        "scoring": {
            "fn": "binary_yes_no",
            "rule": "predict is_canonical_fraud (Y/N) from claim features. "
                    "Target: TPR ≥85%, FPR ≤15%.",
        },
        "decision_gate": "TPR ≥85% and FPR ≤15% → fraud detection ships",
    }

    i2_id = upsert_dataset(
        name="Fraud Detection — Synthetic Thai Claims v1.0",
        source="synthetic_thai_applicants_v1_seed42_claims",
        scoring_fn="binary_yes_no",
        description=(
            "Sprint 2 I2. Synthetic 500 Thai insurance claims with correlated fraud "
            "injection (5 named rules in scripts/synthetic_thai_applicants/claim.py). "
            f"Current run: {n_canonical_fraud}/{n_claims} canonical-fraud "
            f"({n_canonical_fraud/n_claims:.1%}). Label = is_canonical_fraud. "
            "Target gates: TPR ≥85%, FPR ≤15%. Source: tests/eval_datasets/i1/v1.0/claims.jsonl."
        ),
        items_descriptor=i2_descriptor,
        total_items=n_claims,
        version=args.version,
    )

    # ─── Summary ──────────────────────────────────────────────────────────
    print(f"=== Sprint 2 W2.1c — eval dataset registration ===")
    print(f"I1 Underwriter Regression:")
    print(f"  id:         {i1_id}")
    print(f"  rows:       {n_applicants} applicants ({n_medical} medical records)")
    print(f"  scoring_fn: paper_rubric_pct (placeholder for composite_regression)")
    print()
    print(f"I2 Fraud Detection:")
    print(f"  id:                 {i2_id}")
    print(f"  rows:               {n_claims} claims")
    print(f"  canonical fraud:    {n_canonical_fraud} ({n_canonical_fraud/n_claims:.1%})")
    print(f"  scoring_fn:         binary_yes_no")
    print()
    print(f"Verify in DB:")
    print(f"  /api/v1/eval/benchmark-datasets")
    print(f"  https://mimir.asgard.internal/evaluations")

    # Quick verify
    verify = (
        "SELECT id, name, source, scoring_fn, total_items, version, is_active "
        f"FROM eval_benchmark_datasets WHERE id IN ({sql_escape(i1_id)}, {sql_escape(i2_id)}) "
        "ORDER BY source;\n"
    )
    rc, out = run_mariadb(verify)
    print()
    print("--- DB verification ---")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
