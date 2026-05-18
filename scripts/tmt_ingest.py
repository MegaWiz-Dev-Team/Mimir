#!/usr/bin/env python3
"""
Sprint 50 W2.3b — TMT (Thai Medicines Terminology) ingest.

Source: THIS-Center (https://this.or.th/) — `TMTRF<YYYYMMDD>.zip` release.

Loads two table sets:
  - tmt_codes: 8 concept files (SUBS/VTM/GP/GPP/GPU/TP/TPP/TPU)
    111,373 rows in v20260518
  - tmt_relationships: 11 relationship files (SUBStoVTM, VTMtoGP, ...)
    167,138 rows in v20260518

Usage:
    python3 scripts/tmt_ingest.py \\
        --release-dir data/TMT/TMTRF20260518 \\
        --source-version tmt-20260518
"""
from __future__ import annotations
import argparse
import hashlib
import os
import re
import subprocess
import sys
import uuid
from pathlib import Path

import xlrd

# ── DB helper (mirrors loinc_ingest.py — kubectl exec fallback) ────────────


def _have_mysql_cli() -> bool:
    try:
        subprocess.run(["mysql", "--version"], capture_output=True, check=True)
        return True
    except (FileNotFoundError, subprocess.CalledProcessError):
        return False


def mariadb_exec(sql: str) -> str:
    user = os.environ.get("MARIADB_USER", "root")
    pw   = os.environ.get("MARIADB_PASS", "root")
    db   = os.environ.get("MARIADB_DB",   "mimir")
    if _have_mysql_cli():
        host = os.environ.get("MARIADB_HOST", "127.0.0.1")
        port = os.environ.get("MARIADB_PORT", "33306")
        cmd = ["mysql", "-h", host, "-P", port, "-u", user, f"-p{pw}", db, "-B", "-N"]
    else:
        ns = os.environ.get("MARIADB_NAMESPACE", "asgard-infra")
        cmd = ["kubectl", "-n", ns, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-u", user, f"-p{pw}", db, "-B", "-N"]
    r = subprocess.run(cmd, input=sql.encode("utf-8"), capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(f"mariadb exec error: {r.stderr.decode()[:500]}")
    return r.stdout.decode("utf-8")


def sql_quote(s: str | None) -> str:
    if s is None or s == "":
        return "NULL"
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


# ── Concept layer parser ───────────────────────────────────────────────────


CONCEPT_TYPES = ["SUBS", "VTM", "GP", "GPP", "GPU", "TP", "TPP", "TPU"]


def parse_concept_xls(path: Path, concept_type: str) -> list[dict]:
    b = xlrd.open_workbook(str(path))
    s = b.sheet_by_index(0)
    if s.nrows < 2:
        return []
    headers = [str(c.value) for c in s.row(0)]
    # Column names vary slightly: 'TMTID(GP)', 'FSN', 'CHANGEDATE', 'MANUFACTURER'
    tmtid_col = next(i for i, h in enumerate(headers) if h.upper().startswith("TMTID"))
    fsn_col   = headers.index("FSN") if "FSN" in headers else 1
    change_col = next((i for i, h in enumerate(headers) if h.upper() == "CHANGEDATE"), None)
    mfg_col    = next((i for i, h in enumerate(headers) if h.upper() == "MANUFACTURER"), None)

    rows = []
    for ri in range(1, s.nrows):
        r = s.row(ri)
        tmt_id = str(r[tmtid_col].value).strip()
        # xlrd returns floats for numeric cells — coerce
        if tmt_id.endswith(".0"):
            tmt_id = tmt_id[:-2]
        if not tmt_id:
            continue
        fsn = str(r[fsn_col].value).strip()
        if not fsn:
            continue
        mfg = None
        if mfg_col is not None:
            mfg = str(r[mfg_col].value).strip() or None
        chg = None
        if change_col is not None:
            v = str(r[change_col].value).strip()
            if v.endswith(".0"):
                v = v[:-2]
            if re.match(r"^\d{8}$", v):
                chg = f"{v[:4]}-{v[4:6]}-{v[6:]}"
        rows.append({
            "tmt_id": tmt_id,
            "concept_type": concept_type,
            "fsn": fsn,
            "manufacturer": mfg,
            "change_date": chg,
        })
    return rows


# ── Relationship parser ────────────────────────────────────────────────────

# rel_type encoded by filename prefix
REL_FILES = {
    "SUBStoVTM": "SUBStoVTM",
    "VTMtoGP":   "VTMtoGP",
    "GPtoGPU":   "GPtoGPU",
    "GPtoTP":    "GPtoTP",
    "GPUtoGPP":  "GPUtoGPP",
    "GPUtoTPU":  "GPUtoTPU",
    "GPPtoGPP":  "GPPtoGPP",
    "GPPtoTPP":  "GPPtoTPP",
    "TPtoTPU":   "TPtoTPU",
    "TPUtoTPP":  "TPUtoTPP",
    "TPPtoTPP":  "TPPtoTPP",
}


def parse_relationship_xls(path: Path) -> list[tuple[str, str]]:
    b = xlrd.open_workbook(str(path))
    s = b.sheet_by_index(0)
    if s.nrows < 2:
        return []
    pairs = []
    for ri in range(1, s.nrows):
        r = s.row(ri)
        a = str(r[0].value).strip()
        b_id = str(r[1].value).strip()
        if a.endswith(".0"): a = a[:-2]
        if b_id.endswith(".0"): b_id = b_id[:-2]
        if a and b_id:
            pairs.append((a, b_id))
    return pairs


# ── Insert helpers ─────────────────────────────────────────────────────────


def insert_codes(rows: list[dict], source_version: str, batch: int = 500) -> int:
    inserted = 0
    for i in range(0, len(rows), batch):
        chunk = rows[i:i + batch]
        values = []
        for r in chunk:
            values.append("({}, {}, {}, {}, {}, {}, NULL, NULL, NOW(), NOW())".format(
                sql_quote(r["tmt_id"]),
                sql_quote(source_version),
                sql_quote(r["concept_type"]),
                sql_quote(r["fsn"]),
                sql_quote(r["manufacturer"]),
                sql_quote(r["change_date"]),
            ))
        sql = (
            "INSERT INTO tmt_codes "
            "(tmt_id, source_version, concept_type, fsn, manufacturer, change_date, "
            " locale_metadata, tenant_id, created_at, updated_at) VALUES "
            + ",".join(values)
            + " ON DUPLICATE KEY UPDATE "
              "concept_type=VALUES(concept_type), fsn=VALUES(fsn), "
              "manufacturer=VALUES(manufacturer), change_date=VALUES(change_date), "
              "updated_at=NOW()"
        )
        mariadb_exec(sql)
        inserted += len(values)
    return inserted


def insert_relationships(pairs: list[tuple[str, str]], rel_type: str,
                          source_version: str, batch: int = 1000) -> int:
    inserted = 0
    for i in range(0, len(pairs), batch):
        chunk = pairs[i:i + batch]
        values = []
        for a, b in chunk:
            values.append("({}, {}, {}, {}, NULL, NOW())".format(
                sql_quote(a), sql_quote(b),
                sql_quote(rel_type), sql_quote(source_version),
            ))
        sql = (
            "INSERT INTO tmt_relationships "
            "(from_id, to_id, rel_type, source_version, tenant_id, created_at) VALUES "
            + ",".join(values)
            + " ON DUPLICATE KEY UPDATE created_at=created_at"
        )
        mariadb_exec(sql)
        inserted += len(values)
    return inserted


# ── Audit ─────────────────────────────────────────────────────────────────


def insert_run(run_id: str, source_version: str, source_label: str,
               source_url: str | None, sha256: str | None) -> None:
    sql = (
        "INSERT INTO tmt_ingest_runs "
        "(id, source_version, source_label, source_url, source_sha256, "
        "status, started_at) VALUES "
        f"({sql_quote(run_id)}, {sql_quote(source_version)}, "
        f"{sql_quote(source_label)}, {sql_quote(source_url)}, "
        f"{sql_quote(sha256)}, 'RUNNING', NOW())"
    )
    mariadb_exec(sql)


def finalize_run(run_id: str, inserted: int, rels: int, status: str, msg: str) -> None:
    sql = (
        "UPDATE tmt_ingest_runs SET "
        f"rows_inserted={inserted}, rows_relationships={rels}, "
        f"status={sql_quote(status)}, status_message={sql_quote(msg[:1000])}, "
        "finished_at=NOW() "
        f"WHERE id={sql_quote(run_id)}"
    )
    mariadb_exec(sql)


# ── Main ───────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--release-dir", required=True, type=Path,
                    help="Path to TMTRF<YYYYMMDD>/ root (contains _BONUS/Concept + _BONUS/Relationship)")
    ap.add_argument("--source-version", required=True,
                    help="e.g. tmt-20260518")
    ap.add_argument("--source-url", default="https://this.or.th/")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if not args.release_dir.exists():
        print(f"ERR: {args.release_dir} not found", file=sys.stderr)
        return 1

    concept_dir = args.release_dir / "TMTRF20260518_BONUS" / "Concept"
    rel_dir     = args.release_dir / "TMTRF20260518_BONUS" / "Relationship"
    # Be release-version-agnostic: also accept any _BONUS sibling
    if not concept_dir.exists():
        for p in args.release_dir.glob("*_BONUS/Concept"):
            concept_dir = p; break
    if not rel_dir.exists():
        for p in args.release_dir.glob("*_BONUS/Relationship"):
            rel_dir = p; break
    if not concept_dir.exists() or not rel_dir.exists():
        print(f"ERR: Concept/Relationship dirs not found under {args.release_dir}",
              file=sys.stderr)
        return 1

    print(f"=== TMT ingest from {args.release_dir} ===")
    print(f"  Concept dir:      {concept_dir}")
    print(f"  Relationship dir: {rel_dir}")

    # Parse all 8 concept files. Filenames are `{TYPE}<YYYYMMDD>.xls` — we must
    # match TYPE exactly (`GP` matches GP but NOT GPP or GPU) so anchor with
    # the 8-digit date suffix.
    all_rows = []
    for ct in CONCEPT_TYPES:
        matches = [p for p in concept_dir.glob(f"{ct}*.xls")
                   if re.fullmatch(rf"{ct}\d{{8}}\.xls", p.name)]
        if not matches:
            print(f"  WARN: no file for concept_type={ct}", file=sys.stderr)
            continue
        rows = parse_concept_xls(matches[0], ct)
        print(f"  {ct:5s}: {len(rows):>6,} rows from {matches[0].name}")
        all_rows.extend(rows)
    print(f"  TOTAL concepts: {len(all_rows):,}")

    # Parse all relationship files
    all_rels: dict[str, list[tuple[str, str]]] = {}
    for fn in sorted(rel_dir.glob("*.xls")):
        # Match against REL_FILES — pick the longest matching key (to handle
        # ambiguities like "GPtoTP" vs "TPtoTPU")
        rel_type = None
        for key in sorted(REL_FILES.keys(), key=lambda k: -len(k)):
            if fn.name.startswith(key):
                rel_type = REL_FILES[key]
                break
        if rel_type is None:
            print(f"  WARN: unknown relationship file {fn.name}", file=sys.stderr)
            continue
        pairs = parse_relationship_xls(fn)
        all_rels[rel_type] = pairs
        print(f"  {rel_type:12s}: {len(pairs):>6,} pairs from {fn.name}")
    total_rels = sum(len(p) for p in all_rels.values())
    print(f"  TOTAL relationships: {total_rels:,}")

    if args.dry_run:
        print("\n[dry-run] skipping DB writes")
        return 0

    # Best-effort source hash — over concept_dir + rel_dir filenames
    sha = hashlib.sha256()
    for fn in sorted(concept_dir.glob("*.xls")) + sorted(rel_dir.glob("*.xls")):
        sha.update(fn.read_bytes())
    sha_hex = sha.hexdigest()

    run_id = str(uuid.uuid4())
    print(f"\n=== Audit run: {run_id} ===")
    insert_run(run_id, args.source_version, args.source_version,
               args.source_url, sha_hex)

    try:
        print(f"\n=== Inserting {len(all_rows):,} concept rows ===")
        inserted = insert_codes(all_rows, args.source_version)
        print(f"  ✓ inserted/upserted: {inserted:,}")

        print(f"\n=== Inserting {total_rels:,} relationships ===")
        rel_count = 0
        for rt, pairs in all_rels.items():
            n = insert_relationships(pairs, rt, args.source_version)
            rel_count += n
            print(f"  {rt:12s}: {n:>6,}")
        print(f"  ✓ relationships: {rel_count:,}")

        finalize_run(run_id, inserted, rel_count, "COMPLETED",
                     f"TMT release {args.source_version} ingested")
    except Exception as e:
        finalize_run(run_id, 0, 0, "FAILED", str(e)[:1000])
        raise

    # Verify
    print()
    print("=== Verify ===")
    out = mariadb_exec(
        f"SELECT concept_type, COUNT(*) FROM tmt_codes "
        f"WHERE source_version='{args.source_version}' GROUP BY concept_type"
    )
    print(out)
    out = mariadb_exec(
        f"SELECT rel_type, COUNT(*) FROM tmt_relationships "
        f"WHERE source_version='{args.source_version}' GROUP BY rel_type"
    )
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
