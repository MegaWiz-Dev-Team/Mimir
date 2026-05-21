#!/usr/bin/env python3
"""
Sprint 54 — SNOMED CT → ICD-10-TM map ingest (POC: insurance + medical).

Two inputs, both optional/independent:
  --map-file   data/cts_transformed/202605-sct_transformed.txt
               MoPH-transformed ExtendedMap (id, active, ConceptId, ICD10,
               Gender, Age, mapAdvice1..5). Normalized into snomed_icd10_map:
               pipe targets exploded to one row each, advice verb → role, and
               each WHO target resolved to ICD-10-TM by joining icd10_codes.
  --desc-file  SNOMED RF2 sct2_Description_Snapshot file. Active FSN (and
               optionally synonyms) → snomed_descriptions for text→concept search.

WHO→TM bridge is self-derived from icd10_codes (no external valueset needed):
  exact (dot-stripped) → rollup 4-char → rollup 3-char → absent(needs_review).

Usage:
  python3 snomed_icd10_map_ingest.py \\
      --map-file data/cts_transformed/202605-sct_transformed.txt \\
      --desc-file data/SnomedCT/.../sct2_Description_Snapshot-en_INT_20260501.txt \\
      --source-version sct-20260501
"""
from __future__ import annotations
import argparse
import os
import re
import subprocess
import sys
import uuid
from pathlib import Path

FSN_TYPE_ID = "900000000000003001"
SYNONYM_TYPE_ID = "900000000000013009"
AGE_GROUPS = {"neonatal", "pediatric", "adolescent", "adult", "geriatric"}
SEMTAG_RE = re.compile(r"\(([^()]+)\)\s*$")


def _have_mysql_cli() -> bool:
    try:
        subprocess.run(["mysql", "--version"], capture_output=True, check=True)
        return True
    except (FileNotFoundError, subprocess.CalledProcessError):
        return False


def mariadb_exec(sql: str) -> str:
    user = os.environ.get("MARIADB_USER", "root")
    pw = os.environ.get("MARIADB_PASS", "root")
    db = os.environ.get("MARIADB_DB", "mimir")
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


def batched_insert(prefix: str, rows: list[str], batch: int = 500, dry: bool = False) -> int:
    """rows = list of '(...)' value tuples. Returns count inserted."""
    n = 0
    for i in range(0, len(rows), batch):
        chunk = rows[i:i + batch]
        sql = prefix + ",\n".join(chunk) + ";"
        if dry:
            if i == 0:
                print(f"  [dry-run] sample: {sql[:300]}…")
        else:
            mariadb_exec(sql)
        n += len(chunk)
    return n


# ── WHO → ICD-10-TM bridge (self-derived from icd10_codes) ───────────────────

def load_tm_codes() -> set[str]:
    out = mariadb_exec("SELECT DISTINCT code FROM icd10_codes WHERE tenant_id IS NULL")
    return {ln.strip() for ln in out.splitlines() if ln.strip()}


def resolve_tm(who_with_dot: str, tm: set[str]) -> tuple[str | None, str]:
    code = who_with_dot.replace(".", "").strip()
    if code in tm:
        return code, "exact"
    if len(code) >= 4 and code[:4] in tm:
        return code[:4], "rollup"
    if len(code) >= 3 and code[:3] in tm:
        return code[:3], "rollup"
    return None, "absent"


# ── advice verb → role; match advice cell to a target by code substring ──────

def advice_for_target(target_with_dot: str, advices: list[str]) -> tuple[str, str]:
    """Return (role, advice_text) for the advice cell that names this code.
    Falls back to mandatory + joined advice when no code-bearing cell matches."""
    for a in advices:
        if target_with_dot and target_with_dot in a:
            if "CHOOSE" in a:
                role = "conditional"
            elif "ALWAYS" in a:
                role = "mandatory"
            else:
                role = "advisory"
            return role, a
    return "mandatory", " | ".join([a for a in advices if a])


def parse_map_file(path: Path, tm: set[str], source_version: str, dry: bool):
    rows_map: list[str] = []
    counts = {"exact": 0, "rollup": 0, "absent": 0}
    with path.open(encoding="utf-8") as f:
        header = f.readline()  # id active ConceptId ICD10 Gender Age mapAdvice1..5
        for line in f:
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 6:
                continue
            active = cols[1].strip()
            if active != "1":
                continue
            concept_id = cols[2].strip()
            icd10_field = cols[3].strip()
            gender = cols[4].strip() or None
            age = cols[5].strip().lower() or None
            if age is not None and age not in AGE_GROUPS:
                age = None
            advices = [c.strip() for c in cols[6:11]] if len(cols) > 6 else []
            advices = [a for a in advices if a]
            if not icd10_field:
                continue
            for target in icd10_field.split("|"):
                target = target.strip()
                if not target:
                    continue
                tm_code, tier = resolve_tm(target, tm)
                counts[tier] += 1
                role, advice = advice_for_target(target, advices)
                joined = " ".join(advices).upper()
                needs_review = 1 if (
                    tier == "absent"
                    or "CANNOT BE CLASSIFIED" in joined
                    or "CONTEXT DEPENDENT" in joined
                ) else 0
                rows_map.append(
                    "(" + ",".join([
                        sql_quote(source_version),
                        sql_quote(concept_id),
                        sql_quote(gender),
                        sql_quote(age),
                        sql_quote(target),
                        sql_quote(tm_code),
                        sql_quote(tier),
                        sql_quote(role),
                        sql_quote(advice),
                        str(needs_review),
                    ]) + ")"
                )
    prefix = (
        "INSERT INTO snomed_icd10_map "
        "(source_version, concept_id, gender, age_group, icd10_who, icd10_tm, "
        "match_tier, target_role, map_advice, needs_review) VALUES\n"
    )
    inserted = batched_insert(prefix, rows_map, dry=dry)
    return inserted, counts


def parse_desc_file(path: Path, source_version: str, include_syn: bool, dry: bool,
                    synonyms_only: bool = False):
    rows: list[str] = []
    with path.open(encoding="utf-8") as f:
        f.readline()  # header
        for line in f:
            cols = line.rstrip("\n").split("\t")
            # id eff active module conceptId lang typeId term caseSig
            if len(cols) < 8 or cols[2].strip() != "1":
                continue
            type_id = cols[6].strip()
            if type_id == FSN_TYPE_ID and not synonyms_only:
                term_type = "fsn"
            elif type_id == SYNONYM_TYPE_ID and include_syn:
                term_type = "synonym"
            else:
                continue
            concept_id = cols[4].strip()
            term = cols[7].strip()
            if not term:
                continue
            semtag = None
            if term_type == "fsn":
                m = SEMTAG_RE.search(term)
                if m:
                    semtag = m.group(1)[:64]
            rows.append(
                "(" + ",".join([
                    sql_quote(concept_id),
                    sql_quote(source_version),
                    sql_quote(term),
                    sql_quote(term_type),
                    sql_quote(semtag),
                    "1",
                ]) + ")"
            )
    prefix = (
        "INSERT INTO snomed_descriptions "
        "(concept_id, source_version, term, term_type, semantic_tag, active) VALUES\n"
    )
    return batched_insert(prefix, rows, dry=dry)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--map-file", type=Path)
    ap.add_argument("--desc-file", type=Path)
    ap.add_argument("--source-version", default="sct-20260501")
    ap.add_argument("--source-label", default="moph-sct-transform-202605")
    ap.add_argument("--include-synonyms", action="store_true")
    ap.add_argument("--synonyms-only", action="store_true",
                    help="insert only synonym rows (skip FSN) — zero-downtime add to existing FSN")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if not args.map_file and not args.desc_file:
        ap.error("provide --map-file and/or --desc-file")

    run_id = str(uuid.uuid4())
    if not args.dry_run:
        mariadb_exec(
            "INSERT INTO snomed_map_ingest_runs (id, source_version, source_label, status) "
            f"VALUES ({sql_quote(run_id)}, {sql_quote(args.source_version)}, "
            f"{sql_quote(args.source_label)}, 'running')"
        )

    n_desc = n_map = 0
    counts = {"exact": 0, "rollup": 0, "absent": 0}

    if args.desc_file:
        print(f"Parsing descriptions: {args.desc_file}")
        n_desc = parse_desc_file(args.desc_file, args.source_version,
                                 args.include_synonyms or args.synonyms_only,
                                 args.dry_run, synonyms_only=args.synonyms_only)
        print(f"  descriptions inserted: {n_desc}")

    if args.map_file:
        print("Loading ICD-10-TM codes for WHO→TM bridge…")
        tm = load_tm_codes()
        print(f"  TM codes: {len(tm)}")
        print(f"Parsing map: {args.map_file}")
        n_map, counts = parse_map_file(args.map_file, tm, args.source_version, args.dry_run)
        total = sum(counts.values()) or 1
        print(f"  map rows inserted: {n_map}")
        print(f"  WHO→TM: exact={counts['exact']} rollup={counts['rollup']} "
              f"absent={counts['absent']} "
              f"({100*(counts['exact']+counts['rollup'])/total:.1f}% mappable)")

    if not args.dry_run:
        mariadb_exec(
            "UPDATE snomed_map_ingest_runs SET finished_at=NOW(), status='done', "
            f"rows_descriptions={n_desc}, rows_map={n_map}, "
            f"rows_tm_exact={counts['exact']}, rows_tm_rollup={counts['rollup']}, "
            f"rows_tm_absent={counts['absent']} WHERE id={sql_quote(run_id)}"
        )
    print("Done." + (" (dry-run)" if args.dry_run else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
