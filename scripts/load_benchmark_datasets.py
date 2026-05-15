#!/usr/bin/env python3
"""
Load medical benchmark datasets into asgard_medical
Datasets: MedQA, MedMCQA, PubMedQA, HealthBench, MedXpertQA
"""

import os
import json
import subprocess
import sys
from pathlib import Path
from datetime import datetime

DB_PASS = os.environ.get("MIMIR_DB_PASSWORD", "")
TENANT_ID = "asgard_medical"
BENCHMARKS_DIR = Path("/Users/mimir/Developer/Mimir/benchmarks/medical")

# Dataset specifications
DATASETS = {
    "medqa": {
        "name": "MedQA",
        "source": "medical_qa",
        "description": "Medical Question Answering - multiple choice questions",
        "items": 1273,
        "path": BENCHMARKS_DIR / "medqa",
    },
    "medmcqa": {
        "name": "MedMCQA",
        "source": "medical_mcqa",
        "description": "Large-scale multiple choice question answering dataset",
        "items": 6150,
        "path": BENCHMARKS_DIR / "medmcqa",
    },
    "pubmedqa": {
        "name": "PubMedQA",
        "source": "pubmed_qa",
        "description": "PubMed Biomedical QA dataset - yes/no/maybe questions",
        "items": 1000,
        "path": BENCHMARKS_DIR / "pubmedqa",
    },
    "healthbench": {
        "name": "HealthBench",
        "source": "healthbench",
        "description": "HealthBench professional medical benchmark",
        "items": 5000,
        "path": BENCHMARKS_DIR / "healthbench",
    },
    "medxpertqa": {
        "name": "MedXpertQA",
        "source": "medxpert_qa",
        "description": "Multimodal Medical Expert QA",
        "items": 2450,
        "path": BENCHMARKS_DIR / "medxpertqa",
    },
}


def kubectl_mariadb(sql: str, dry_run: bool = False) -> tuple:
    """Execute SQL via kubectl."""
    if dry_run:
        print(f"[DRY-RUN] {sql[:100]}...")
        return 0, ""

    proc = subprocess.run(
        ["kubectl", "exec", "-i", "-n", "asgard-infra", "deploy/mariadb", "--",
         "mariadb", "-u", "mimir", f"--password={DB_PASS}", "mimir"],
        input=sql, capture_output=True, text=True, check=False,
    )
    return proc.returncode, proc.stderr[:200]


def sql_escape(s):
    """Escape SQL string literal."""
    if s is None:
        return "NULL"
    s = str(s).replace("\\", "\\\\").replace("'", "\\'")
    return f"'{s}'"


def load_datasets(dry_run: bool = False) -> int:
    """Load all benchmark datasets into asgard_medical."""
    print("📚 Loading Medical Benchmark Datasets into asgard_medical")
    print("=" * 70)
    print(f"Dry-run: {dry_run}\n")

    failed = 0

    for dataset_id, dataset_info in DATASETS.items():
        print(f"Loading {dataset_info['name']}...")

        # Check if dataset path exists
        if not dataset_info["path"].exists():
            print(f"  ⚠️  Dataset path not found: {dataset_info['path']}")
            print(f"     Skipping {dataset_id}")
            failed += 1
            continue

        # Prepare INSERT statement
        items_json = json.dumps({
            "description": dataset_info["description"],
            "path": str(dataset_info["path"]),
            "loaded_at": datetime.now().isoformat(),
        }).replace("'", "\\'")

        sql = f"""
INSERT INTO eval_benchmark_datasets (
    id, tenant_id, name, source, total_items, version,
    is_active, items, created_at, updated_at
) VALUES (
    '{dataset_id}',
    '{TENANT_ID}',
    {sql_escape(dataset_info['name'])},
    {sql_escape(dataset_info['source'])},
    {dataset_info['items']},
    1,
    1,
    '{items_json}',
    NOW(),
    NOW()
) ON DUPLICATE KEY UPDATE updated_at=NOW();
"""

        rc, err = kubectl_mariadb(sql, dry_run)
        if rc == 0:
            print(f"  ✓ {dataset_info['name']} ({dataset_info['items']} items)")
        else:
            print(f"  ✗ Failed: {err}")
            failed += 1

    print("\n" + "=" * 70)
    return failed


def verify_datasets():
    """Verify datasets were loaded."""
    print("\n🔍 Verification:")
    print("-" * 70)

    sql = f"""
    SELECT
      name,
      source,
      total_items,
      is_active,
      created_at
    FROM eval_benchmark_datasets
    WHERE tenant_id='{TENANT_ID}'
    ORDER BY name;
    """

    proc = subprocess.run(
        ["kubectl", "exec", "-i", "-n", "asgard-infra", "deploy/mariadb", "--",
         "mariadb", "-u", "mimir", f"--password={DB_PASS}", "mimir", "--batch", "--silent"],
        input=sql, capture_output=True, text=True, check=False,
    )

    if proc.stdout:
        lines = proc.stdout.strip().split('\n')
        if lines and lines[0]:
            print(f"{'Dataset':<25} {'Items':<10} {'Active'}")
            print("-" * 70)
            for line in lines:
                parts = line.split('\t')
                if len(parts) >= 4:
                    name = parts[0][:23]
                    items = parts[2]
                    active = "✓" if parts[3] == "1" else "✗"
                    print(f"{name:<25} {items:<10} {active}")
        else:
            print("(No datasets loaded yet)")
    else:
        print("(No datasets loaded yet)")


def main():
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if not DB_PASS:
        print("❌ MIMIR_DB_PASSWORD not set")
        return 1

    failed = load_datasets(args.dry_run)

    if not args.dry_run:
        verify_datasets()

    if failed == 0:
        print(f"\n✅ All {len(DATASETS)} datasets ready!")
        return 0
    else:
        print(f"\n⚠️  {failed} dataset(s) had issues")
        return 1


if __name__ == "__main__":
    sys.exit(main())
