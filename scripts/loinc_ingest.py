#!/usr/bin/env python3
"""
Sprint 49 W2.3a — LOINC master ingest from official Loinc.csv.

Source:
  https://loinc.org/downloads/  →  LOINC_<version>_Source.zip → Loinc.csv
  (Free; requires LOINC account. Public domain license under LOINC terms.)

Output:
  - INSERT into loinc_codes (tenant_id=NULL, source_version='loinc-<ver>')
  - INSERT into loinc_ingest_runs as audit trail

Loinc.csv has ~50 columns. We pull the clinically-useful subset:
  LOINC_NUM, LONG_COMMON_NAME, SHORTNAME, COMPONENT,
  PROPERTY, TIME_ASPCT, SYSTEM, SCALE_TYP, METHOD_TYP,
  CLASS, STATUS, EXAMPLE_UCUM_UNITS, RELATEDNAMES2

Usage:
    python scripts/loinc_ingest.py --csv /path/to/Loinc.csv \\
        --source-version loinc-2.78 \\
        [--dry-run]
"""
from __future__ import annotations
import argparse
import csv
import hashlib
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path

# Only these columns are persisted. Anything else from Loinc.csv goes into
# locale_metadata JSON if --keep-extras is set.
CORE_COLS = {
    "LOINC_NUM", "LONG_COMMON_NAME", "SHORTNAME", "COMPONENT",
    "PROPERTY", "TIME_ASPCT", "SYSTEM", "SCALE_TYP", "METHOD_TYP",
    "CLASS", "STATUS", "EXAMPLE_UCUM_UNITS",
}
EXTRA_COLS_KEPT = {"RELATEDNAMES2", "CONSUMER_NAME", "VERSION_LAST_CHANGED"}


def mariadb_exec(sql: str) -> str:
    """Run SQL via local MariaDB. Honors DATABASE_URL/MARIADB_URL when set;
    falls back to `mysql -h 127.0.0.1 -P 33306 -u root -proot mimir` matching
    the local-dev port-forward recipe in `s1_e2e_manual_2026_05_18` memory."""
    host = os.environ.get("MARIADB_HOST", "127.0.0.1")
    port = os.environ.get("MARIADB_PORT", "33306")
    user = os.environ.get("MARIADB_USER", "root")
    pw   = os.environ.get("MARIADB_PASS", "root")
    db   = os.environ.get("MARIADB_DB", "mimir")
    r = subprocess.run(
        ["mysql", "-h", host, "-P", port, "-u", user, f"-p{pw}", db, "-B", "-N"],
        input=sql.encode("utf-8"),
        capture_output=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"mysql error: {r.stderr.decode()}")
    return r.stdout.decode("utf-8")


def sql_quote(s: str | None) -> str:
    if s is None or s == "":
        return "NULL"
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


def parse_csv(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            loinc = (row.get("LOINC_NUM") or "").strip()
            if not loinc:
                continue
            lcn = (row.get("LONG_COMMON_NAME") or "").strip()
            if not lcn:
                # LOINC requires LONG_COMMON_NAME; skip malformed lines.
                continue
            extras = {k: row[k] for k in EXTRA_COLS_KEPT if row.get(k)}
            rows.append({
                "loinc_num": loinc,
                "long_common_name": lcn,
                "short_name":  (row.get("SHORTNAME") or "").strip() or None,
                "component":   (row.get("COMPONENT") or "").strip() or None,
                "property":    (row.get("PROPERTY") or "").strip() or None,
                "time_aspct":  (row.get("TIME_ASPCT") or "").strip() or None,
                "system_axis": (row.get("SYSTEM") or "").strip() or None,
                "scale_typ":   (row.get("SCALE_TYP") or "").strip() or None,
                "method_typ":  (row.get("METHOD_TYP") or "").strip() or None,
                "class":       (row.get("CLASS") or "").strip() or None,
                "status":      (row.get("STATUS") or "").strip() or None,
                "example_ucum":(row.get("EXAMPLE_UCUM_UNITS") or "").strip() or None,
                "extras":      extras,
            })
    return rows


def insert_run(run_id: str, source_version: str, source_label: str,
               source_url: str | None, sha256: str) -> None:
    sql = (
        "INSERT INTO loinc_ingest_runs "
        "(id, source_version, source_label, source_url, source_sha256, "
        "status, started_at) VALUES "
        f"({sql_quote(run_id)}, {sql_quote(source_version)}, "
        f"{sql_quote(source_label)}, {sql_quote(source_url)}, "
        f"{sql_quote(sha256)}, 'RUNNING', NOW())"
    )
    mariadb_exec(sql)


def finalize_run(run_id: str, inserted: int, skipped: int,
                 status: str, msg: str) -> None:
    sql = (
        "UPDATE loinc_ingest_runs SET "
        f"rows_inserted={inserted}, rows_skipped={skipped}, "
        f"status={sql_quote(status)}, status_message={sql_quote(msg)}, "
        "finished_at=NOW() "
        f"WHERE id={sql_quote(run_id)}"
    )
    mariadb_exec(sql)


def insert_codes(rows: list[dict], source_version: str,
                 batch_size: int = 500) -> tuple[int, int]:
    inserted = skipped = 0
    for i in range(0, len(rows), batch_size):
        chunk = rows[i:i + batch_size]
        values = []
        for r in chunk:
            if not r["long_common_name"]:
                skipped += 1
                continue
            extras_json = json.dumps(r["extras"], ensure_ascii=False) if r["extras"] else None
            values.append(
                "("
                f"{sql_quote(r['loinc_num'])}, {sql_quote(source_version)}, "
                f"{sql_quote(r['long_common_name'])}, {sql_quote(r['short_name'])}, "
                f"{sql_quote(r['component'])}, {sql_quote(r['property'])}, "
                f"{sql_quote(r['time_aspct'])}, {sql_quote(r['system_axis'])}, "
                f"{sql_quote(r['scale_typ'])}, {sql_quote(r['method_typ'])}, "
                f"{sql_quote(r['class'])}, {sql_quote(r['status'])}, "
                f"{sql_quote(r['example_ucum'])}, {sql_quote(extras_json)}, NULL, "
                "NOW(), NOW()"
                ")"
            )
        if not values:
            continue
        sql = (
            "INSERT INTO loinc_codes "
            "(loinc_num, source_version, long_common_name, short_name, component, "
            " property, time_aspct, system_axis, scale_typ, method_typ, class, "
            " status, example_ucum, locale_metadata, tenant_id, created_at, updated_at) "
            "VALUES " + ",\n".join(values) +
            " ON DUPLICATE KEY UPDATE "
            "long_common_name=VALUES(long_common_name), "
            "short_name=VALUES(short_name), component=VALUES(component), "
            "property=VALUES(property), time_aspct=VALUES(time_aspct), "
            "system_axis=VALUES(system_axis), scale_typ=VALUES(scale_typ), "
            "method_typ=VALUES(method_typ), class=VALUES(class), "
            "status=VALUES(status), example_ucum=VALUES(example_ucum), "
            "locale_metadata=VALUES(locale_metadata), updated_at=NOW()"
        )
        mariadb_exec(sql)
        inserted += len(values)
    return inserted, skipped


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--csv", required=True, type=Path,
                    help="Path to Loinc.csv from LOINC_<ver>_Source.zip")
    ap.add_argument("--source-version", required=True,
                    help="e.g. 'loinc-2.78'")
    ap.add_argument("--source-label", default=None,
                    help="Default: same as --source-version")
    ap.add_argument("--source-url",
                    default="https://loinc.org/downloads/")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if not args.csv.exists():
        print(f"ERR: csv not found: {args.csv}", file=sys.stderr)
        return 1

    label = args.source_label or args.source_version
    print(f"=== Reading {args.csv} ===")
    sha = hashlib.sha256(args.csv.read_bytes()).hexdigest()
    print(f"  sha256: {sha[:16]}…  size: {args.csv.stat().st_size/1_000_000:.1f} MB")

    print("=== Parsing CSV ===")
    rows = parse_csv(args.csv)
    print(f"  parsed rows: {len(rows):,}")

    # Class distribution + status sanity check.
    by_class: dict[str, int] = {}
    by_status: dict[str, int] = {}
    for r in rows:
        by_class[r.get("class") or "(none)"] = by_class.get(r.get("class") or "(none)", 0) + 1
        by_status[r.get("status") or "(none)"] = by_status.get(r.get("status") or "(none)", 0) + 1
    top_classes = sorted(by_class.items(), key=lambda kv: -kv[1])[:8]
    print("  top classes:", ", ".join(f"{c}={n:,}" for c, n in top_classes))
    print("  status dist:", ", ".join(f"{s}={n:,}" for s, n in sorted(by_status.items())))

    print("\n=== Sample (first 3) ===")
    for r in rows[:3]:
        print(f"  {r['loinc_num']:<10}  [{r.get('class')}] {r['long_common_name'][:60]}")

    if args.dry_run:
        print("\n[dry-run] skipping DB ingest")
        return 0

    run_id = str(uuid.uuid4())
    print(f"\n=== Audit run: {run_id} ===")
    insert_run(run_id, args.source_version, label, args.source_url, sha)

    try:
        print(f"=== Inserting {len(rows):,} rows into loinc_codes (batch=500) ===")
        inserted, skipped = insert_codes(rows, args.source_version)
        finalize_run(run_id, inserted, skipped, "COMPLETED",
                     f"LOINC bootstrap from {label}")
        print(f"  ✓ inserted/upserted: {inserted:,}, skipped: {skipped}")
    except Exception as e:
        finalize_run(run_id, 0, 0, "FAILED", str(e))
        raise

    n = mariadb_exec(
        f"SELECT COUNT(*) FROM loinc_codes WHERE source_version='{args.source_version}'"
    ).strip()
    print(f"\n=== Verify === loinc_codes rows for source_version={args.source_version}: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
