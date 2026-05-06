#!/usr/bin/env python3
"""
Sprint 48 — ICD-10 / ICD-10-TM lookup CLI.

Quick way to test the icd10_codes table before the Rust Hermodr handler ships.
Mirrors the planned Hermodr API:

  icd10_lookup(query, mode, locale, limit)

Usage:
  python3 icd10_lookup.py I21
  python3 icd10_lookup.py "หลอดเลือดสมอง" --mode prefix --locale th
  python3 icd10_lookup.py "asthma" --mode prefix --locale en --limit 20
  python3 icd10_lookup.py "เบาหวาน" --mode naive --locale both --json

Modes:
  auto    — try exact code → exact label → prefix → naive (cascade, default)
  exact   — exact code OR exact label match
  prefix  — code prefix OR label LIKE 'q%'
  naive   — substring LIKE '%q%'

Locale:
  en      — search en_label
  th      — search th_label
  both    — search both columns (default)
"""
from __future__ import annotations
import argparse
import json
import subprocess
import sys

MARIADB_POD = (
    "k8s_mariadb_mariadb-fb55894c5-xjjvb_asgard-infra_"
    "78f65c51-6439-4d1c-a9f6-ebcdad463f5c_58"
)
DEFAULT_VERSION = "anamai-moph-2010"


def mariadb_query(sql: str) -> list[dict]:
    """Run SQL via mariadb pod, return list of dict rows. Uses --batch (TSV)."""
    r = subprocess.run(
        ["docker", "exec", "-i", MARIADB_POD,
         "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir", "--batch"],
        input=sql.encode("utf-8"),
        capture_output=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"mariadb error: {r.stderr.decode()}")
    out = r.stdout.decode("utf-8").strip()
    if not out:
        return []
    lines = out.split("\n")
    headers = lines[0].split("\t")
    rows: list[dict] = []
    for line in lines[1:]:
        cells = line.split("\t")
        rows.append({h: c for h, c in zip(headers, cells)})
    return rows


def sql_quote(s: str) -> str:
    return "'" + s.replace("\\", "\\\\").replace("'", "''") + "'"


def build_where(query: str, mode: str, locale: str) -> str:
    """Build the WHERE clause for the given mode + locale. Returns SQL fragment."""
    q = query.strip()
    qs = sql_quote(q)
    qs_prefix = sql_quote(q + "%")
    qs_substr = sql_quote("%" + q + "%")

    label_cols = {
        "en":   ["en_label"],
        "th":   ["th_label"],
        "both": ["en_label", "th_label"],
    }[locale]

    if mode == "exact":
        clauses = [f"code = {qs}"]
        for col in label_cols:
            clauses.append(f"{col} = {qs}")
        return f"({' OR '.join(clauses)})"

    elif mode == "prefix":
        clauses = [f"code LIKE {qs_prefix}"]
        for col in label_cols:
            clauses.append(f"{col} LIKE {qs_prefix}")
        return f"({' OR '.join(clauses)})"

    elif mode == "naive":
        clauses = [f"code LIKE {qs_substr}"]
        for col in label_cols:
            clauses.append(f"{col} LIKE {qs_substr}")
        return f"({' OR '.join(clauses)})"

    raise ValueError(f"unknown mode: {mode}")


def search(query: str, mode: str, locale: str, limit: int,
           source_version: str) -> list[dict]:
    where = build_where(query, mode, locale)
    sql = f"""
        SELECT code, en_label, th_label, chapter,
               billable_flag AS billable, source_version,
               locale_metadata
        FROM icd10_codes
        WHERE {where}
          AND source_version = {sql_quote(source_version)}
          AND (tenant_id IS NULL)
        ORDER BY
            (code = {sql_quote(query.strip())}) DESC,
            (code LIKE {sql_quote(query.strip() + '%')}) DESC,
            CHAR_LENGTH(code) ASC,
            code ASC
        LIMIT {int(limit)}
    """
    return mariadb_query(sql)


def search_auto(query: str, locale: str, limit: int,
                source_version: str) -> tuple[str, list[dict]]:
    """Cascade: exact → prefix → naive. Returns (mode_matched, rows)."""
    for m in ("exact", "prefix", "naive"):
        rows = search(query, m, locale, limit, source_version)
        if rows:
            return m, rows
    return "naive", []


def format_human(rows: list[dict], mode: str, query: str) -> str:
    if not rows:
        return f"  (no results for '{query}' in mode={mode})"
    out: list[str] = []
    for r in rows:
        code = r["code"]
        chap = r.get("chapter") or "?"
        en = (r.get("en_label") or "")[:60]
        th = (r.get("th_label") or "")
        out.append(f"  {code:<8} ch={chap:<5} {en:<62}  {th}")
    return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("query", help="code, label, or natural-language phrase")
    ap.add_argument("--mode", choices=["auto", "exact", "prefix", "naive"], default="auto")
    ap.add_argument("--locale", choices=["en", "th", "both"], default="both")
    ap.add_argument("--limit", type=int, default=10)
    ap.add_argument("--source-version", default=DEFAULT_VERSION)
    ap.add_argument("--json", action="store_true", help="emit JSON instead of human-readable")
    args = ap.parse_args()

    if args.mode == "auto":
        matched_mode, rows = search_auto(args.query, args.locale, args.limit,
                                         args.source_version)
    else:
        matched_mode = args.mode
        rows = search(args.query, args.mode, args.locale, args.limit,
                      args.source_version)

    if args.json:
        out = {
            "query": args.query,
            "mode_used": matched_mode,
            "locale": args.locale,
            "source_version": args.source_version,
            "count": len(rows),
            "results": [
                {
                    "code": r["code"],
                    "en_label": r.get("en_label"),
                    "th_label": r.get("th_label"),
                    "chapter": r.get("chapter"),
                    "billable": r.get("billable") == "1",
                    "source_version": r.get("source_version"),
                }
                for r in rows
            ],
        }
        print(json.dumps(out, ensure_ascii=False, indent=2))
    else:
        print(f"=== query='{args.query}' mode={matched_mode} locale={args.locale} count={len(rows)} ===")
        print(format_human(rows, matched_mode, args.query))

    return 0


if __name__ == "__main__":
    sys.exit(main())
