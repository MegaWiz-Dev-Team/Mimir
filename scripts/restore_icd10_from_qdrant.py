#!/usr/bin/env python3
"""
Sprint 48 B-48d Phase A — restore icd10_codes from Qdrant icd10-th payloads.

Used when MariaDB icd10_codes is empty (e.g. after Sprint 51e rotation that
reset the schema) but the Qdrant `icd10-th` collection still has all 15,376
payloads from the original anamai-moph-2010 ingest. Faster than re-parsing
the anamai PDF: pulls payloads directly, batch-inserts, and writes an audit
run row to `icd10_ingest_runs`.

This is a one-shot restore — should NOT be the primary ingest path for
new source_versions. For new versions use icd10_tm_anamai_ingest.py (which
parses the source PDF and stores source_sha256 in the audit).

Requires:
  - Qdrant port-forwarded:  kubectl port-forward svc/qdrant 6333:6333 -n asgard-infra
  - kubectl access to mariadb pod in asgard-infra namespace
  - pymysql installed (/opt/homebrew Python 3.14 has it)
"""
from __future__ import annotations
import argparse
import json
import subprocess
import sys
import time
import uuid
from urllib.request import Request, urlopen

QDRANT_SCROLL = "http://localhost:6333/collections/icd10-th/points/scroll"
DEFAULT_SOURCE_VERSION = "anamai-moph-2010"
SOURCE_URL = "https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf"
MARIADB_POD = "mariadb-585d5cd485-fwmjh"
NAMESPACE = "asgard-infra"


def http_post_json(url: str, body: dict, timeout: float = 60.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    req = Request(url, data=data, headers={"Content-Type": "application/json"})
    with urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def load_payloads(source_version: str) -> list[dict]:
    """Scroll all icd10-th points for source_version, return payloads only."""
    out = []
    next_page = None
    while True:
        body = {"limit": 500, "with_payload": True, "with_vector": False}
        if next_page is not None:
            body["offset"] = next_page
        resp = http_post_json(QDRANT_SCROLL, body)
        for p in resp["result"]["points"]:
            pl = p["payload"]
            if pl.get("source_version") == source_version:
                out.append(pl)
        next_page = resp["result"].get("next_page_offset")
        if next_page is None:
            break
    return out


def sql_escape(s: str | None) -> str:
    """MySQL string literal escaping. Backslash + single-quote only."""
    if s is None:
        return "NULL"
    s = s.replace("\\", "\\\\").replace("'", "\\'")
    return f"'{s}'"


def run_mariadb(sql_stdin: str) -> tuple[int, str]:
    """Pipe SQL to mariadb via kubectl exec. Returns (rc, output)."""
    cmd = [
        "kubectl", "exec", "-n", NAMESPACE, MARIADB_POD, "-i", "--",
        "bash", "-c", 'mariadb -uroot -p"$MYSQL_ROOT_PASSWORD" mimir',
    ]
    proc = subprocess.run(cmd, input=sql_stdin, capture_output=True, text=True)
    return proc.returncode, (proc.stdout + proc.stderr)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-version", default=DEFAULT_SOURCE_VERSION)
    ap.add_argument("--batch-size", type=int, default=500)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    print(f"=== Restore icd10_codes from Qdrant icd10-th payloads ===")
    print(f"source_version: {args.source_version}")
    print(f"batch size:     {args.batch_size}")
    print()

    print("Pulling payloads from Qdrant...")
    t0 = time.time()
    payloads = load_payloads(args.source_version)
    print(f"  Loaded {len(payloads)} payloads in {time.time()-t0:.1f}s")
    if not payloads:
        print("  No payloads found — abort.", file=sys.stderr)
        return 1

    # Start an audit row first so the SQL can reference its uuid.
    run_id = str(uuid.uuid4())
    rows_count = len(payloads)
    start_iso = time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())

    print(f"\nAudit run id: {run_id}")
    print(f"Inserting...")
    if args.dry_run:
        print("[dry-run] would insert audit row + batch icd10_codes.")
        return 0

    audit_sql = (
        "INSERT INTO icd10_ingest_runs "
        "(id, source_version, source_label, source_url, rows_inserted, status, status_message, started_at, finished_at, notes) "
        f"VALUES ({sql_escape(run_id)}, {sql_escape(args.source_version)}, "
        f"{sql_escape('anamai-moph-2010-restore-from-qdrant')}, "
        f"{sql_escape(SOURCE_URL)}, 0, 'RUNNING', "
        f"{sql_escape('Restored from icd10-th Qdrant payloads (MariaDB rotation reset)')}, "
        f"{sql_escape(start_iso)}, NULL, "
        f"{sql_escape('Source-of-truth = Qdrant collection icd10-th. PDF not re-parsed.')});\n"
    )
    rc, out = run_mariadb(audit_sql)
    if rc != 0:
        print(f"audit insert failed: {out}", file=sys.stderr)
        return 1

    # Batch-insert icd10_codes
    inserted = 0
    t_ins = time.time()
    for start in range(0, len(payloads), args.batch_size):
        batch = payloads[start:start + args.batch_size]
        values = []
        for p in batch:
            code = p.get("code", "")
            en = p.get("en_label", "")
            th = p.get("th_label")
            chap = p.get("chapter")
            sv = p.get("source_version", args.source_version)
            values.append(
                "({c}, {sv}, {en}, {th}, {chap}, NULL, 1, NULL, NULL, NULL, NOW(), NOW())".format(
                    c=sql_escape(code), sv=sql_escape(sv),
                    en=sql_escape(en), th=sql_escape(th),
                    chap=sql_escape(chap),
                )
            )
        sql = (
            "INSERT INTO icd10_codes "
            "(code, source_version, en_label, th_label, chapter, block, billable_flag, "
            "drg_id, locale_metadata, tenant_id, created_at, updated_at) VALUES\n"
            + ",\n".join(values) + ";\n"
        )
        rc, out = run_mariadb(sql)
        if rc != 0:
            print(f"\nbatch [{start}..{start+len(batch)}] FAILED: {out[:300]}", file=sys.stderr)
            return 1
        inserted += len(batch)
        sys.stdout.write(f"\r  inserted {inserted}/{rows_count}")
        sys.stdout.flush()
    print()
    print(f"  Inserted {inserted} rows in {time.time()-t_ins:.1f}s")

    # Close out audit row
    finish_iso = time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())
    close_sql = (
        "UPDATE icd10_ingest_runs SET "
        f"rows_inserted = {inserted}, status = 'COMPLETED', "
        f"finished_at = {sql_escape(finish_iso)} "
        f"WHERE id = {sql_escape(run_id)};\n"
    )
    rc, out = run_mariadb(close_sql)
    if rc != 0:
        print(f"audit close failed: {out}", file=sys.stderr)
        return 1

    # Verify
    verify_sql = (
        "SELECT COUNT(*) FROM icd10_codes WHERE source_version = "
        f"{sql_escape(args.source_version)};\n"
        "SELECT id, status, rows_inserted, finished_at FROM icd10_ingest_runs "
        f"WHERE id = {sql_escape(run_id)};\n"
    )
    rc, out = run_mariadb(verify_sql)
    print(f"\n--- Verification ---")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
