#!/usr/bin/env python3
"""
Sprint 48 — ICD-10-TM Phase A bootstrap from anamai (Department of Health) PDF.

Source:
  https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf

Output:
  - INSERT into icd10_codes (tenant_id=NULL, source_version='anamai-moph-2010')
  - INSERT into icd10_ingest_runs as audit trail

PDF layout (4-column fixed-width-ish, after `pdftotext -layout`):
    รหัสกลุมโรค  รหัส ICD-10-TM    diagename                 diagtname
       1            A00          Cholera                   อหิวาตกโรค
       1           A000          Cholera due to V...       อหิวาตกโรคจาก...
                                 (continuation)            (continuation)

Heuristic:
  - A "primary line" is one whose code-column matches /^[A-Z][0-9]{2,4}$/
  - Continuation lines (no code) get appended to the previous row's en/th
  - Page-header line "รหัสกลุมโรค รหัส ICD-10-TM ..." is filtered

Chapter derivation: WHO ICD-10 chapter (Roman numeral) by code-prefix range.
Block: deferred (B-48 refresh sprint).
"""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import subprocess
import sys
import uuid
from pathlib import Path

# ─── ICD-10 chapter ranges (WHO standard, used as default) ──────────────────
# Each tuple: (chapter_roman, start_letter, start_num, end_letter, end_num)
CHAPTER_RANGES = [
    ("I",    "A", 0,  "B", 99),   # Infectious & parasitic
    ("II",   "C", 0,  "D", 48),   # Neoplasms
    ("III",  "D", 50, "D", 89),   # Blood disorders
    ("IV",   "E", 0,  "E", 90),   # Endocrine
    ("V",    "F", 0,  "F", 99),   # Mental
    ("VI",   "G", 0,  "G", 99),   # Nervous
    ("VII",  "H", 0,  "H", 59),   # Eye
    ("VIII", "H", 60, "H", 95),   # Ear
    ("IX",   "I", 0,  "I", 99),   # Circulatory
    ("X",    "J", 0,  "J", 99),   # Respiratory
    ("XI",   "K", 0,  "K", 93),   # Digestive
    ("XII",  "L", 0,  "L", 99),   # Skin
    ("XIII", "M", 0,  "M", 99),   # Musculoskeletal
    ("XIV",  "N", 0,  "N", 99),   # Genitourinary
    ("XV",   "O", 0,  "O", 99),   # Pregnancy
    ("XVI",  "P", 0,  "P", 96),   # Perinatal
    ("XVII", "Q", 0,  "Q", 99),   # Congenital
    ("XVIII","R", 0,  "R", 99),   # Symptoms/signs
    ("XIX",  "S", 0,  "T", 98),   # Injury/poisoning
    ("XX",   "V", 0,  "Y", 98),   # External causes
    ("XXI",  "Z", 0,  "Z", 99),   # Factors
    ("XXII", "U", 0,  "U", 85),   # Special purposes
]


def derive_chapter(code: str) -> str | None:
    if not code or len(code) < 3:
        return None
    letter = code[0]
    try:
        num = int(code[1:3])
    except ValueError:
        return None
    for chap, sl, sn, el, en in CHAPTER_RANGES:
        if (letter, num) >= (sl, sn) and (letter, num) <= (el, en):
            return chap
    return None


# ─── PDF parser ─────────────────────────────────────────────────────────────
PRIMARY_LINE_RE = re.compile(
    r"^\s+(\d+)\s{2,}([A-Z]\d{2,4})\s{2,}(\S.*?)(?:\s{4,}(\S.*?))?\s*$"
)
CODE_ONLY_RE = re.compile(r"^[A-Z][0-9]{2,4}$")
HEADER_RE = re.compile(r"รหัสกลุมโรค|รหัส\s*ICD")


def pdftotext_layout(pdf_path: Path) -> str:
    r = subprocess.run(
        ["pdftotext", "-layout", str(pdf_path), "-"],
        capture_output=True, check=True,
    )
    return r.stdout.decode("utf-8")


THAI_RE = re.compile(r"[฀-๿-]")


def normalize_thai(s: str | None) -> str | None:
    """Strip PUA Thai chars (\\uf700-\\uf71f) used by old MoPH PDF font embeds.
    Lossy v0 — refresh sprint should ingest XLS source for correct combining."""
    if not s:
        return s
    s = re.sub(r"[-]", "", s)
    return s.strip()


def parse_rows(text: str) -> list[dict]:
    """
    Walk lines; emit one record per primary line; coalesce continuation lines
    into the previous record's en_label / th_label.

    Continuation rule: line without leading-space-then-digits pattern is
    treated as a continuation of the previous primary row. Strip leading
    whitespace BEFORE splitting on column gap, so the leading space-padding
    doesn't get parsed as the first split target.
    """
    rows: list[dict] = []
    cur: dict | None = None

    for raw in text.splitlines():
        line = raw.replace("\\,", ",")  # pdftotext literal escape
        # Page header — flush + skip.
        if HEADER_RE.search(line):
            if cur:
                rows.append(cur); cur = None
            continue
        if not line.strip():
            # Blank line — flush current row (if any).
            if cur:
                rows.append(cur); cur = None
            continue

        m = PRIMARY_LINE_RE.match(line)
        if m:
            # New record.
            if cur:
                rows.append(cur)
            group = int(m.group(1))
            code = m.group(2)
            en = (m.group(3) or "").strip()
            th = (m.group(4) or "").strip()
            cur = {"group": group, "code": code, "en": en, "th": th}
        else:
            # Continuation: append to previous en + th. Strip leading space
            # before splitting (otherwise the leading 28+ spaces become the
            # first split match and we lose EN content).
            if not cur:
                continue
            content = line.strip()
            if not content:
                continue
            split = re.split(r"\s{4,}", content, maxsplit=1)
            if len(split) == 2:
                en_part = split[0].strip()
                th_part = split[1].strip()
                if en_part:
                    cur["en"] = (cur["en"] + " " + en_part).strip()
                if th_part:
                    cur["th"] = (cur["th"] + " " + th_part).strip() if cur.get("th") else th_part
            else:
                # Single-column continuation — heuristic by Thai chars.
                text = split[0].strip()
                if THAI_RE.search(text):
                    cur["th"] = (cur["th"] + " " + text).strip() if cur.get("th") else text
                else:
                    cur["en"] = (cur["en"] + " " + text).strip()

    if cur:
        rows.append(cur)

    # Final pass: strip PUA Thai chars (lossy v0 — refresh sprint will replace).
    for r in rows:
        r["th"] = normalize_thai(r.get("th"))
    return rows


# ─── DB I/O via mariadb pod ─────────────────────────────────────────────────
MARIADB_POD = (
    "k8s_mariadb_mariadb-fb55894c5-xjjvb_asgard-infra_"
    "78f65c51-6439-4d1c-a9f6-ebcdad463f5c_58"
)


def mariadb_exec(sql: str) -> str:
    r = subprocess.run(
        ["docker", "exec", "-i", MARIADB_POD,
         "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir", "-B", "-N"],
        input=sql.encode("utf-8"),
        capture_output=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"mariadb error: {r.stderr.decode()}")
    return r.stdout.decode("utf-8")


def sql_quote(s: str | None) -> str:
    if s is None:
        return "NULL"
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


def insert_run(run_id: str, source_version: str, source_label: str,
               source_url: str, source_sha256: str) -> None:
    sql = (
        "INSERT INTO icd10_ingest_runs "
        "(id, source_version, source_label, source_url, source_sha256, "
        "status, started_at) VALUES "
        f"({sql_quote(run_id)}, {sql_quote(source_version)}, "
        f"{sql_quote(source_label)}, {sql_quote(source_url)}, "
        f"{sql_quote(source_sha256)}, 'RUNNING', NOW())"
    )
    mariadb_exec(sql)


def finalize_run(run_id: str, inserted: int, updated: int, skipped: int,
                 status: str, msg: str) -> None:
    sql = (
        "UPDATE icd10_ingest_runs SET "
        f"rows_inserted={inserted}, rows_updated={updated}, rows_skipped={skipped}, "
        f"status={sql_quote(status)}, status_message={sql_quote(msg)}, "
        "finished_at=NOW() "
        f"WHERE id={sql_quote(run_id)}"
    )
    mariadb_exec(sql)


def insert_codes(rows: list[dict], source_version: str, batch_size: int = 200) -> tuple[int, int, int]:
    """Bulk INSERT ... ON DUPLICATE KEY UPDATE. Returns (inserted, updated, skipped)."""
    inserted = updated = skipped = 0
    for i in range(0, len(rows), batch_size):
        chunk = rows[i:i + batch_size]
        values = []
        for r in chunk:
            code = r["code"]
            en = r["en"]
            th = r.get("th") or None
            chapter = derive_chapter(code)
            if not en:
                skipped += 1
                continue
            locale_meta = json.dumps({
                "anamai_group_code": r.get("group"),
                "ingest": "anamai-moph-2010-pdf",
            }, ensure_ascii=False)
            values.append(
                f"({sql_quote(code)}, {sql_quote(source_version)}, "
                f"{sql_quote(en)}, {sql_quote(th)}, "
                f"{sql_quote(chapter)}, NULL, TRUE, NULL, "
                f"{sql_quote(locale_meta)}, NULL, NOW(), NOW())"
            )
        if not values:
            continue
        sql = (
            "INSERT INTO icd10_codes "
            "(code, source_version, en_label, th_label, chapter, block, "
            "billable_flag, drg_id, locale_metadata, tenant_id, "
            "created_at, updated_at) VALUES "
            + ",\n".join(values)
            + " ON DUPLICATE KEY UPDATE "
            "en_label=VALUES(en_label), th_label=VALUES(th_label), "
            "chapter=VALUES(chapter), locale_metadata=VALUES(locale_metadata), "
            "updated_at=NOW()"
        )
        mariadb_exec(sql)
        # affected_rows distinguishes insert (1) vs update (2) on ODKU; we don't
        # parse that here — just count chunks. Approximate inserted vs updated
        # via post-ingest count diff.
        inserted += len(values)
    return inserted, updated, skipped


# ─── main ───────────────────────────────────────────────────────────────────
def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", default="/tmp/icd10tm_anamai.pdf")
    ap.add_argument("--source-version", default="anamai-moph-2010")
    ap.add_argument("--source-label", default="anamai-moph-2010-pdf")
    ap.add_argument(
        "--source-url",
        default="https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf",
    )
    ap.add_argument("--dry-run", action="store_true",
                    help="Parse only, don't insert.")
    args = ap.parse_args()

    pdf_path = Path(args.pdf)
    if not pdf_path.exists():
        print(f"ERR: pdf not found: {pdf_path}", file=sys.stderr)
        return 1

    print(f"=== Reading {pdf_path} ===")
    sha256 = hashlib.sha256(pdf_path.read_bytes()).hexdigest()
    print(f"  sha256: {sha256[:16]}…")

    print("=== Running pdftotext -layout ===")
    text = pdftotext_layout(pdf_path)
    print(f"  text bytes: {len(text):,}")

    print("=== Parsing 4-column rows ===")
    rows = parse_rows(text)
    print(f"  rows parsed: {len(rows):,}")

    # Sanity: chapter-distribution + dedupe within source_version.
    seen: set[str] = set()
    dedup: list[dict] = []
    for r in rows:
        if r["code"] in seen:
            continue
        seen.add(r["code"])
        dedup.append(r)
    print(f"  unique codes: {len(dedup):,} (dropped {len(rows)-len(dedup)} duplicates)")

    print("=== Sample (first 5) ===")
    for r in dedup[:5]:
        chap = derive_chapter(r["code"])
        print(f"  {r['code']:<6} ch={chap or '?'} grp={r.get('group')} en='{r['en'][:60]}' th='{(r.get('th') or '')[:40]}'")

    if args.dry_run:
        print("\n[dry-run] skipping DB ingest")
        return 0

    run_id = str(uuid.uuid4())
    print(f"\n=== Audit run: {run_id} ===")
    insert_run(run_id, args.source_version, args.source_label, args.source_url, sha256)

    try:
        print(f"=== Inserting {len(dedup):,} rows into icd10_codes ===")
        inserted, updated, skipped = insert_codes(dedup, args.source_version)
        msg = f"Phase A bootstrap from {args.source_label}; chapter derived from WHO ranges; block deferred."
        finalize_run(run_id, inserted, updated, skipped, "COMPLETED", msg)
        print(f"  ✓ inserted/upserted: {inserted}, skipped: {skipped}")
    except Exception as e:
        finalize_run(run_id, 0, 0, 0, "FAILED", str(e))
        raise

    # Verify counts.
    n = mariadb_exec(
        f"SELECT COUNT(*) FROM icd10_codes WHERE source_version='{args.source_version}'"
    ).strip()
    print(f"\n=== Verify === icd10_codes rows for source_version={args.source_version}: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
