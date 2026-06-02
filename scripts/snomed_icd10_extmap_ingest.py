#!/usr/bin/env python3
"""
Sprint 60 — SNOMED CT → ICD-10-CM map ingest (US Edition official ExtendedMap).

Loads the NLM-published SNOMED CT to ICD-10-CM map: an RF2 "Extended Map"
refset (refsetId 6011000124106) shipped inside the SNOMED CT US Edition.

This is the US counterpart to snomed_icd10_map_ingest.py (which builds the Thai
ICD-10-TM crosswalk via a WHO→TM bridge). Here the targets ARE the official
ICD-10-CM codes — no bridge, no self-derivation — so we ingest the RF2 columns
faithfully into snomed_icd10cm_map, one row per candidate (concept, group,
priority), preserving mapRule / mapAdvice / mapCategory.

Input file (RF2 tab-separated, 13 columns):
  der2_iisssccRefset_ExtendedMapSnapshot_US1000124_<YYYYMMDD>.txt
  cols: id  effectiveTime  active  moduleId  refsetId  referencedComponentId
        mapGroup  mapPriority  mapRule  mapAdvice  mapTarget  correlationId
        mapCategoryId
  (Use the *Snapshot* file for current state; *Full* also works — only active=1
   rows are ingested either way.)

The map is RULE-BASED, not 1:1: within a mapGroup, candidates are tried in
mapPriority order and the first matching mapRule wins. mapRule is "TRUE" (always),
"IFA <id> | <term> |" (gender/age/finding gate), or "OTHERWISE TRUE".

LICENSE: requires a UMLS Metathesaurus License (free) in addition to the SNOMED
Affiliate License. US (ICD-10-CM) artifact — for Thai claims use the ICD-10-TM
ingest instead.

Usage:
  python3 snomed_icd10cm_map_ingest.py \\
      --map-file data/SnomedCT_US/.../der2_iisssccRefset_ExtendedMapSnapshot_US1000124_20250901.txt \\
      --source-version sct-us-20250901 \\
      [--refset-id 6011000124106] [--dry-run]

Env (shared with snomed_icd10_map_ingest.py): MARIADB_USER/PASS/DB/HOST/PORT,
MARIADB_NAMESPACE (K8s fallback when mysql CLI is absent).
"""
from __future__ import annotations
import argparse
import os
import subprocess
import sys
import uuid
from pathlib import Path

# RF2 mapCategoryId (SNOMED concept) → short label. The only "clean" category is
# properly-classified; everything else flags needs_review.
PROPERLY_CLASSIFIED = "447637006"
MAP_CATEGORY = {
    "447637006": "properly classified",
    "447638001": "cannot classify (no data)",
    "447639009": "context dependent",
    "447634004": "ambiguous",
    "447635003": "target invalid",
    "450580003": "context dependent",
}
ICD10CM_REFSET = "6011000124106"   # SNOMED CT US Edition → ICD-10-CM
ICD10WHO_REFSET = "447562003"      # SNOMED CT International → ICD-10 (WHO complex map)

# Per-target defaults so one script serves both editions. Adding CM later is just
# a re-run with --target-system icd10cm; no code change.
TARGET_DEFAULTS = {
    "icd10cm": {
        "refset": ICD10CM_REFSET,
        "source_version": "sct-us-icd10cm",
        "source_label": "nlm-snomed-us-icd10cm",
        "source_url": "https://www.nlm.nih.gov/healthit/snomedct/us_edition.html",
    },
    "icd10who": {
        "refset": ICD10WHO_REFSET,
        "source_version": "sct-int-icd10who",
        "source_label": "snomed-international-icd10",
        "source_url": "https://www.snomed.org/",
    },
}


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


def sql_int(s: str | None, default: int = 1) -> str:
    s = (s or "").strip()
    return s if s.lstrip("-").isdigit() else str(default)


def batched_insert(prefix: str, rows: list[str], batch: int = 500, dry: bool = False) -> int:
    """rows = list of '(...)' value tuples. Returns count inserted."""
    n = 0
    for i in range(0, len(rows), batch):
        chunk = rows[i:i + batch]
        sql = prefix + ",\n".join(chunk) + ";"
        if dry:
            if i == 0:
                print(f"  [dry-run] sample: {sql[:320]}…")
        else:
            mariadb_exec(sql)
        n += len(chunk)
    return n


def parse_map_file(path: Path, target_system: str, source_version: str,
                   refset_filter: str | None, dry: bool):
    rows: list[str] = []
    concepts: set[str] = set()
    cat_counts: dict[str, int] = {}
    n_review = 0
    n_skip = 0
    with path.open(encoding="utf-8") as f:
        f.readline()  # header
        for line in f:
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 13:
                n_skip += 1
                continue
            if cols[2].strip() != "1":          # active only
                continue
            refset_id = cols[4].strip()
            if refset_filter and refset_id != refset_filter:
                n_skip += 1
                continue
            concept_id = cols[5].strip()
            map_group = cols[6].strip()
            map_priority = cols[7].strip()
            map_rule = cols[8].strip() or None
            map_advice = cols[9].strip() or None
            target = cols[10].strip()
            correlation_id = cols[11].strip() or None
            category_id = cols[12].strip() or None
            category = MAP_CATEGORY.get(category_id, "other") if category_id else None

            needs_review = 1 if (not target or category_id != PROPERLY_CLASSIFIED) else 0
            if needs_review:
                n_review += 1
            if target:
                concepts.add(concept_id)
            if category is not None:
                cat_counts[category] = cat_counts.get(category, 0) + 1

            rows.append(
                "(" + ",".join([
                    sql_quote(target_system),
                    sql_quote(source_version),
                    sql_quote(refset_id),
                    sql_quote(concept_id),
                    sql_int(map_group),
                    sql_int(map_priority),
                    sql_quote(map_rule),
                    sql_quote(map_advice),
                    sql_quote(target or None),
                    sql_quote(correlation_id),
                    sql_quote(category_id),
                    sql_quote(category),
                    str(needs_review),
                ]) + ")"
            )
    prefix = (
        "INSERT INTO snomed_icd10_extmap "
        "(target_system, source_version, refset_id, concept_id, map_group, map_priority, "
        "map_rule, map_advice, icd10_code, correlation_id, map_category_id, "
        "map_category, needs_review) VALUES\n"
    )
    inserted = batched_insert(prefix, rows, dry=dry)
    return inserted, len(concepts), n_review, n_skip, cat_counts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--target-system", required=True, choices=["icd10cm", "icd10who"],
                    help="which classification this map targets")
    ap.add_argument("--map-file", type=Path, required=True,
                    help="RF2 der2_iisssccRefset_ExtendedMap{Snapshot,Full} file")
    ap.add_argument("--source-version", default=None)
    ap.add_argument("--source-label", default=None)
    ap.add_argument("--source-url", default=None)
    ap.add_argument("--refset-id", default=None,
                    help="filter to this refsetId; defaults per target; '' keeps all")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    # Fill per-target defaults for anything not explicitly given.
    d = TARGET_DEFAULTS[args.target_system]
    args.source_version = args.source_version or d["source_version"]
    args.source_label = args.source_label or d["source_label"]
    args.source_url = args.source_url or d["source_url"]
    refset_arg = d["refset"] if args.refset_id is None else args.refset_id
    refset_filter = refset_arg.strip() or None

    run_id = str(uuid.uuid4())
    if not args.dry_run:
        mariadb_exec(
            "INSERT INTO snomed_icd10_extmap_ingest_runs "
            "(id, target_system, source_version, source_label, source_url, status) VALUES ("
            f"{sql_quote(run_id)}, {sql_quote(args.target_system)}, {sql_quote(args.source_version)}, "
            f"{sql_quote(args.source_label)}, {sql_quote(args.source_url)}, 'RUNNING')"
        )

    print(f"Parsing {args.target_system} map: {args.map_file}")
    try:
        n_map, n_concepts, n_review, n_skip, cats = parse_map_file(
            args.map_file, args.target_system, args.source_version, refset_filter, args.dry_run
        )
    except Exception as e:
        if not args.dry_run:
            mariadb_exec(
                "UPDATE snomed_icd10_extmap_ingest_runs SET status='FAILED', finished_at=NOW(), "
                f"status_message={sql_quote(str(e)[:480])} WHERE id={sql_quote(run_id)}"
            )
        raise

    print(f"  rows inserted:    {n_map}")
    print(f"  concepts mapped:  {n_concepts}")
    print(f"  needs_review:     {n_review}")
    print(f"  rows skipped:     {n_skip}")
    print(f"  by category:      {cats}")

    if not args.dry_run:
        mariadb_exec(
            "UPDATE snomed_icd10_extmap_ingest_runs SET status='COMPLETED', finished_at=NOW(), "
            f"rows_inserted={n_map}, rows_review={n_review}, rows_skipped={n_skip}, "
            f"concepts_mapped={n_concepts} WHERE id={sql_quote(run_id)}"
        )
    print("Done." + (" (dry-run)" if args.dry_run else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
