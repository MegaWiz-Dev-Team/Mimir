#!/usr/bin/env python3
"""
Sprint 52 W2.3c — ICD-9-CM Volume 3 (procedures) ingest as TPC fallback.

Source: US CMS public-domain ICD-9-CM (FY15) PDF + 2018 errata.
Populates `tpc_codes` table with source_version='icd9cm-cms-fy15'.

Per `feedback_no_ollama` & the deployment runbook: this is a fallback for
the official Thai TPC license which is blocked at MoPH. When the real TPC
arrives, ingest as source_version='tpc-moph-YYYY' into the same table —
multi-version PK lets cascade lookup pick Thai-first, US-fallback.

Strategy:
- Parse `icd9cm.pdf` Tabular List of Procedures (pages 154–~290).
- Extract each leaf code `XX.YY <description>` plus block headers `XX.Y`.
- Skip diagnosis tables (Volumes 1-2) by tracking page headers.
- Skip "Includes:" / "Excludes:" / "Code also" annotations.
- Optionally parse the 2018 errata for the 46 new codes.

Usage:
    python3 scripts/icd9cm_ingest.py \\
        --pdf data/ICD-9-CM/icd9cm.pdf \\
        --errata data/ICD-9-CM/new-invalid-icd9cm-2561.pdf \\
        --source-version icd9cm-cms-fy15
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


# ── DB helper (kubectl exec fallback) ──────────────────────────────────────


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
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


# ── ICD-9-CM Procedure chapter ranges (US official) ───────────────────────
# Chapter is the leading 2 digits; ranges below.
CHAPTER_RANGES = [
    ("00",       0,  0),     # Procedures and Interventions, NEC
    ("01-05",    1,  5),     # Operations on the Nervous System
    ("06-07",    6,  7),     # Operations on the Endocrine System
    ("08-16",    8, 16),     # Operations on the Eye
    ("17",      17, 17),     # Misc Diagnostic and Therapeutic Procedures
    ("18-20",   18, 20),     # Operations on the Ear
    ("21-29",   21, 29),     # Operations on the Nose, Mouth, Pharynx
    ("30-34",   30, 34),     # Operations on the Respiratory System
    ("35-39",   35, 39),     # Operations on the Cardiovascular System
    ("40-41",   40, 41),     # Operations on the Hemic and Lymphatic System
    ("42-54",   42, 54),     # Operations on the Digestive System
    ("55-59",   55, 59),     # Operations on the Urinary System
    ("60-64",   60, 64),     # Operations on the Male Genital Organs
    ("65-71",   65, 71),     # Operations on the Female Genital Organs
    ("72-75",   72, 75),     # Obstetrical Procedures
    ("76-84",   76, 84),     # Operations on the Musculoskeletal System
    ("85-86",   85, 86),     # Operations on the Integumentary System
    ("87-99",   87, 99),     # Misc Diagnostic and Therapeutic Procedures
]


def derive_chapter(code: str) -> str | None:
    """code is 'XX.YY' or 'XX.Y' or 'XX' — chapter is the XX bucket."""
    m = re.match(r"^(\d{1,2})", code)
    if not m: return None
    xx = int(m.group(1))
    for label, start, end in CHAPTER_RANGES:
        if start <= xx <= end:
            return label
    return None


# ── PDF parser ─────────────────────────────────────────────────────────────


def pdftotext_layout(pdf: Path) -> str:
    r = subprocess.run(["pdftotext", "-layout", str(pdf), "-"],
                       capture_output=True, check=True)
    return r.stdout.decode("utf-8")


# A procedure code line looks like:
#     "    XX.YY <description>"   (leaf code, 4 chars)
#     "  XX.Y <block header>"    (block, 3 chars)
# But also annotations:
#     "         Includes: ..."
#     "         Excludes: ..."
#     "          Code also ..."
# We only keep the code-line itself.
CODE_LINE = re.compile(r"^\s+(\d{1,2}\.\d{1,2})\s+(\S.*?)\s*$")
BLOCK_LINE = re.compile(r"^\s+(\d{1,2}\.\d)\s+(\S.*?)\s*$")  # 3-char block

# Don't parse diagnosis-side codes (3-digit XXX.YY).
DIAG_LINE = re.compile(r"^\s+\d{3}\.\d")

PROCEDURES_HEADER = re.compile(r"TABULAR LIST OF PROCEDURES")
DIAGNOSES_HEADER = re.compile(r"TABULAR LIST OF DISEASES|TABULAR LIST OF DIAGNOSES")
INDEX_HEADER = re.compile(r"INDEX TO (PROCEDURES|DISEASES)")


def parse_tabular_procedures(text: str) -> list[dict]:
    rows: list[dict] = []
    in_procedures = False
    seen_codes: set[str] = set()

    for line in text.splitlines():
        # Track section context
        if PROCEDURES_HEADER.search(line):
            in_procedures = True
            continue
        if DIAGNOSES_HEADER.search(line) or INDEX_HEADER.search(line):
            in_procedures = False
            continue
        if not in_procedures:
            continue

        # Hard guard: skip diagnosis-shape codes
        if DIAG_LINE.match(line):
            continue

        # Try leaf code first (4 chars XX.YY).
        m = CODE_LINE.match(line)
        if m:
            code = m.group(1)
            desc = m.group(2).strip()
            # Skip if description is obviously a note
            if desc.startswith(("Includes:", "Excludes:", "Code also",
                                "Note:", "Use additional")):
                continue
            if code in seen_codes:
                continue
            # Verify the XX portion is in procedure chapter range
            chapter = derive_chapter(code)
            if chapter is None:
                continue
            seen_codes.add(code)
            # Block = XX.Y (first 4 chars including '.')
            block = code.rsplit(".", 1)[0] + "." + code.rsplit(".", 1)[1][:1]
            rows.append({
                "code": code,
                "en_label": desc,
                "chapter": chapter,
                "block": block,
                "billable_flag": True,
            })
            continue

        # Block headers also exist but we keep them lower priority.
        # The CODE_LINE pattern already matched both leaf + block; this is
        # for the rare case where a block heading line is parsed differently.
    return rows


def parse_errata_2018(text: str) -> list[dict]:
    """Errata PDF parser. pdftotext -layout gives `<code>   <desc>` per line.
    Plain pdftotext gives codes-then-descs in 2 sequential blocks. Handle both."""
    lines = [l for l in text.splitlines() if l.strip()]
    codes: list[str] = []
    descs: list[str] = []

    # Try layout-mode first: each line has "0060    Ins d-e stent..."
    layout_re = re.compile(r"^(\d{4})\s{2,}(\S.+?)\s*$")
    for line in lines:
        m = layout_re.match(line)
        if m:
            codes.append(m.group(1))
            descs.append(m.group(2))

    # Fall back to plain mode (codes-then-descs sequence) if layout failed.
    if not codes:
        mode = None
        for line in lines:
            s = line.strip()
            if s.lower() == "code":
                mode = "codes"; continue
            if s.lower() == "desc":
                mode = "descs"; continue
            if mode == "codes" and re.fullmatch(r"\d{4}", s):
                codes.append(s)
            elif mode == "descs" and s and not re.match(r"^[A-Z]+$", s):
                descs.append(s)
    # Align by index — codes are 4-digit compact, convert to XX.YY canonical
    rows = []
    for code_compact, desc in zip(codes, descs):
        # "0060" → "00.60"  ;  "8690" → "86.90"
        code = f"{code_compact[:2]}.{code_compact[2:]}"
        chapter = derive_chapter(code)
        if chapter is None:
            continue
        # Block = XX.Y (3-char prefix)
        head, tail = code.split(".", 1)
        block = f"{head}.{tail[:1]}"
        rows.append({
            "code": code,
            "en_label": desc,
            "chapter": chapter,
            "block": block,
            "billable_flag": True,
        })
    return rows


# ── DB writers ─────────────────────────────────────────────────────────────


def insert_run(run_id: str, source_version: str, label: str,
               url: str | None, sha: str | None) -> None:
    sql = (
        "INSERT INTO tpc_ingest_runs "
        "(id, source_version, source_label, source_url, source_sha256, "
        " status, started_at) VALUES "
        f"({sql_quote(run_id)}, {sql_quote(source_version)}, "
        f"{sql_quote(label)}, {sql_quote(url)}, "
        f"{sql_quote(sha)}, 'RUNNING', NOW())"
    )
    mariadb_exec(sql)


def finalize_run(run_id: str, ins: int, skipped: int, status: str, msg: str) -> None:
    sql = (
        "UPDATE tpc_ingest_runs SET "
        f"rows_inserted={ins}, rows_skipped={skipped}, "
        f"status={sql_quote(status)}, status_message={sql_quote(msg[:1000])}, "
        "finished_at=NOW() "
        f"WHERE id={sql_quote(run_id)}"
    )
    mariadb_exec(sql)


def insert_codes(rows: list[dict], source_version: str, batch: int = 500) -> int:
    inserted = 0
    for i in range(0, len(rows), batch):
        chunk = rows[i:i + batch]
        values = []
        for r in chunk:
            values.append(
                "({}, {}, {}, NULL, {}, {}, {}, NULL, NULL, NOW(), NOW())".format(
                    sql_quote(r["code"]),
                    sql_quote(source_version),
                    sql_quote(r["en_label"]),
                    sql_quote(r["chapter"]),
                    sql_quote(r["block"]),
                    "TRUE" if r["billable_flag"] else "FALSE",
                )
            )
        sql = (
            "INSERT INTO tpc_codes "
            "(code, source_version, en_label, th_label, chapter, block, "
            " billable_flag, locale_metadata, tenant_id, created_at, updated_at) "
            "VALUES " + ",".join(values) +
            " ON DUPLICATE KEY UPDATE "
            "en_label=VALUES(en_label), chapter=VALUES(chapter), "
            "block=VALUES(block), updated_at=NOW()"
        )
        mariadb_exec(sql)
        inserted += len(values)
    return inserted


# ── Main ───────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", required=True, type=Path,
                    help="Path to icd9cm.pdf (FY15 tabular list)")
    ap.add_argument("--errata", type=Path, default=None,
                    help="Optional: 2018 errata PDF with 46 new codes")
    ap.add_argument("--source-version", default="icd9cm-cms-fy15")
    ap.add_argument("--source-url",
                    default="https://www.cms.gov/medicare/coding-billing/icd-10-codes")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if not args.pdf.exists():
        print(f"ERR: {args.pdf} not found", file=sys.stderr); return 1

    print(f"=== ICD-9-CM ingest from {args.pdf} ===")
    sha = hashlib.sha256(args.pdf.read_bytes()).hexdigest()
    print(f"  sha256: {sha[:16]}…  size: {args.pdf.stat().st_size/1_000_000:.1f} MB")

    print("\n=== Running pdftotext -layout ===")
    text = pdftotext_layout(args.pdf)
    print(f"  text bytes: {len(text):,}")

    print("\n=== Parsing Tabular List of Procedures ===")
    rows = parse_tabular_procedures(text)
    print(f"  procedure codes parsed: {len(rows):,}")

    if args.errata and args.errata.exists():
        print(f"\n=== Parsing errata {args.errata.name} ===")
        errata_text = pdftotext_layout(args.errata)
        errata_rows = parse_errata_2018(errata_text)
        print(f"  errata codes: {len(errata_rows):,}")
        # Errata overlays — last write wins via ON DUPLICATE KEY UPDATE
        rows.extend(errata_rows)

    # Chapter distribution sanity
    by_chapter: dict[str, int] = {}
    for r in rows:
        by_chapter[r["chapter"]] = by_chapter.get(r["chapter"], 0) + 1
    print("\n=== Chapter distribution ===")
    for ch in sorted(by_chapter):
        print(f"  {ch:6s}: {by_chapter[ch]:>4} codes")

    print("\n=== Sample (first 5) ===")
    for r in rows[:5]:
        print(f"  {r['code']:<7} [{r['chapter']:<6}] {r['en_label'][:70]}")

    if args.dry_run:
        print("\n[dry-run] skipping DB writes")
        return 0

    run_id = str(uuid.uuid4())
    print(f"\n=== Audit run: {run_id} ===")
    insert_run(run_id, args.source_version, args.source_version, args.source_url, sha)

    try:
        print(f"\n=== Inserting {len(rows):,} rows into tpc_codes ===")
        inserted = insert_codes(rows, args.source_version)
        finalize_run(run_id, inserted, 0, "COMPLETED",
                     f"ICD-9-CM CMS FY15 + 2018 errata ingested as TPC fallback")
        print(f"  ✓ inserted/upserted: {inserted:,}")
    except Exception as e:
        finalize_run(run_id, 0, 0, "FAILED", str(e)[:1000])
        raise

    n = mariadb_exec(
        f"SELECT COUNT(*) FROM tpc_codes WHERE source_version='{args.source_version}'"
    ).strip()
    print(f"\n=== Verify === tpc_codes rows for {args.source_version}: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
