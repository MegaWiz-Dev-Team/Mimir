#!/usr/bin/env python3
"""
Sprint 55 — RxNorm ingest (brand/generic → ingredient, + UNII bridge key).

Replaces the v1 static 57-brand seed table
(ro-ai-domain-medical/data/rxnorm_brand_ingredient.tsv, built dev-time from the RxNav
API) with the full RxNorm release loaded into MariaDB master tables — the same shape
as tmt_codes/loinc_codes. RxNorm is US-gov **public domain** so it ships in commercial
Asgard (unlike DrugBank). See NORMALIZER.md §5.

Source: RxNorm Full monthly release (free UMLS account) — unzip and point --rrf-dir at
the `rrf/` folder holding RXNCONSO.RRF, RXNREL.RRF, RXNSAT.RRF (pipe-delimited, no
header, trailing pipe).

  RXNCONSO: RXCUI|LAT|TS|LUI|STT|SUI|ISPREF|RXAUI|SAUI|SCUI|SDUI|SAB|TTY|CODE|STR|SRL|SUPPRESS|CVF|
  RXNREL:   RXCUI1|RXAUI1|STYPE1|REL|RXCUI2|RXAUI2|STYPE2|RELA|RUI|SRUI|SAB|SL|RG|DIR|SUPPRESS|CVF|
  RXNSAT:   RXCUI|LUI|SUI|RXAUI|STYPE|CODE|ATUI|SATUI|ATN|SAB|ATV|SUPPRESS|CVF|

Usage:
    python3 scripts/rxnorm_ingest.py \\
        --rrf-dir data/RxNorm/RxNorm_full_20260601/rrf \\
        --source-version rxnorm-20260601
    # add --dry-run to parse + report counts without writing to the DB
"""
from __future__ import annotations
import argparse
import hashlib
import os
import subprocess
import sys
import uuid
from pathlib import Path

# ── DB helper (identical pattern to tmt_ingest.py / loinc_ingest.py) ─────────


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


def norm(s: str) -> str:
    return s.strip().lower()[:255]


# ── RRF parsing ─────────────────────────────────────────────────────────────

# TTY we keep — the ones a resolver ever matches or climbs. Dropping SCDG/dose-group
# noise keeps the atom table lean.
KEEP_TTY = {"IN", "PIN", "MIN", "BN", "SBD", "SCD", "SBDC", "SCDC", "SY", "TMSY", "PSN"}
# RELA we keep — the ingredient-bearing closure a brand/drug climbs to reach an IN.
KEEP_RELA = {
    "has_ingredient", "ingredient_of",
    "tradename_of", "has_tradename",
    "consists_of", "constitutes",
    "form_of", "has_form",
    "precise_ingredient_of", "has_precise_ingredient",
}


def parse_conso(path: Path):
    """RXNCONSO → atom rows (English, kept TTY, not suppressed)."""
    rows = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("|")
            if len(c) < 17:
                continue
            rxcui, lat, rxaui, sab, tty, sstr, suppress = c[0], c[1], c[7], c[11], c[12], c[14], c[16]
            if lat != "ENG" or tty not in KEEP_TTY or suppress in ("O", "Y", "E"):
                continue
            if not sstr.strip():
                continue
            rows.append({
                "rxaui": rxaui, "rxcui": rxcui, "tty": tty, "sab": sab,
                "str": sstr, "str_norm": norm(sstr), "suppress": suppress or "N",
            })
    return rows


def parse_rel(path: Path):
    """RXNREL → ingredient-bearing relation rows (deduped on the kept key)."""
    seen, rows = set(), []
    with open(path, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("|")
            if len(c) < 11:
                continue
            rxcui1, rela, rxcui2, sab = c[0], c[7], c[4], c[10]
            if rela not in KEEP_RELA or not rxcui1 or not rxcui2:
                continue
            k = (rxcui1, rela, rxcui2)
            if k in seen:
                continue
            seen.add(k)
            rows.append({"rxcui1": rxcui1, "rela": rela, "rxcui2": rxcui2, "sab": sab})
    return rows


def parse_sat(path: Path):
    """RXNSAT → UNII rows (ATN='UNII')."""
    seen, rows = set(), []
    with open(path, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("|")
            if len(c) < 11:
                continue
            rxcui, atn, atv = c[0], c[8], c[10]
            if atn != "UNII" or not rxcui or not atv.strip():
                continue
            k = (rxcui, atv.strip())
            if k in seen:
                continue
            seen.add(k)
            rows.append({"rxcui": rxcui, "unii": atv.strip()})
    return rows


# ── Inserts (batched, ON DUPLICATE KEY — same shape as tmt_ingest.insert_*) ──


def insert_atoms(rows, source_version, batch=500):
    n = 0
    for i in range(0, len(rows), batch):
        vals = []
        for r in rows[i:i + batch]:
            vals.append("({}, {}, {}, {}, {}, {}, {}, {}, NULL, NOW(), NOW())".format(
                sql_quote(r["rxaui"]), sql_quote(r["rxcui"]), sql_quote(r["tty"]),
                sql_quote(r["sab"]), sql_quote(r["str"]), sql_quote(r["str_norm"]),
                sql_quote(r["suppress"]), sql_quote(source_version),
            ))
        mariadb_exec(
            "INSERT INTO rxnorm_atoms "
            "(rxaui, rxcui, tty, sab, str, str_norm, suppress, source_version, "
            " tenant_id, created_at, updated_at) VALUES " + ",".join(vals) +
            " ON DUPLICATE KEY UPDATE rxcui=VALUES(rxcui), tty=VALUES(tty), "
            "sab=VALUES(sab), str=VALUES(str), str_norm=VALUES(str_norm), "
            "suppress=VALUES(suppress), updated_at=NOW()")
        n += len(vals)
    return n


def insert_rel(rows, source_version, batch=1000):
    n = 0
    for i in range(0, len(rows), batch):
        vals = []
        for r in rows[i:i + batch]:
            vals.append("({}, {}, {}, {}, {}, NULL, NOW())".format(
                sql_quote(r["rxcui1"]), sql_quote(r["rela"]), sql_quote(r["rxcui2"]),
                sql_quote(r["sab"]), sql_quote(source_version),
            ))
        mariadb_exec(
            "INSERT INTO rxnorm_rel "
            "(rxcui1, rela, rxcui2, sab, source_version, tenant_id, created_at) VALUES "
            + ",".join(vals) + " ON DUPLICATE KEY UPDATE sab=VALUES(sab)")
        n += len(vals)
    return n


def insert_unii(rows, source_version, batch=1000):
    n = 0
    for i in range(0, len(rows), batch):
        vals = []
        for r in rows[i:i + batch]:
            vals.append("({}, {}, {}, NULL, NOW())".format(
                sql_quote(r["rxcui"]), sql_quote(r["unii"]), sql_quote(source_version)))
        mariadb_exec(
            "INSERT INTO rxnorm_unii (rxcui, unii, source_version, tenant_id, created_at) "
            "VALUES " + ",".join(vals) + " ON DUPLICATE KEY UPDATE unii=VALUES(unii)")
        n += len(vals)
    return n


def sha256_dir(paths):
    h = hashlib.sha256()
    for p in sorted(paths):
        with open(p, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rrf-dir", required=True, help="folder with RXNCONSO/RXNREL/RXNSAT.RRF")
    ap.add_argument("--source-version", required=True, help="e.g. rxnorm-20260601")
    ap.add_argument("--dry-run", action="store_true", help="parse + report; no DB writes")
    args = ap.parse_args()

    d = Path(args.rrf_dir)
    conso, rel, sat = d / "RXNCONSO.RRF", d / "RXNREL.RRF", d / "RXNSAT.RRF"
    for p in (conso, rel, sat):
        if not p.exists():
            sys.exit(f"missing {p}")

    print(f"parsing {conso.name} …", flush=True)
    atoms = parse_conso(conso)
    print(f"  {len(atoms):>8} atoms kept (TTY {sorted(KEEP_TTY)})", flush=True)
    ingredients = sum(1 for a in atoms if a["tty"] in ("IN", "PIN"))
    brands = sum(1 for a in atoms if a["tty"] == "BN")
    inn_syn = sum(1 for a in atoms if a["tty"] in ("SY", "TMSY"))
    print(f"     {ingredients} ingredient atoms · {brands} brand atoms · {inn_syn} synonyms(incl INN)", flush=True)
    print(f"parsing {rel.name} …", flush=True)
    rels = parse_rel(rel)
    print(f"  {len(rels):>8} ingredient-bearing relations", flush=True)
    print(f"parsing {sat.name} …", flush=True)
    uniis = parse_sat(sat)
    print(f"  {len(uniis):>8} UNII mappings", flush=True)

    if args.dry_run:
        print("dry-run: no DB writes. (coverage of the 57-brand v1 set is a superset by construction)")
        return

    run_id = str(uuid.uuid4())
    checksum = sha256_dir([conso, rel, sat])
    mariadb_exec(
        "INSERT INTO rxnorm_ingest_runs (id, source_version, source_label, source_sha256, "
        "status, created_at, updated_at) VALUES ("
        f"{sql_quote(run_id)}, {sql_quote(args.source_version)}, "
        f"{sql_quote('RxNorm Full')}, {sql_quote(checksum)}, 'RUNNING', NOW(), NOW())")

    try:
        na = insert_atoms(atoms, args.source_version)
        print(f"  inserted {na} atoms", flush=True)
        nr = insert_rel(rels, args.source_version)
        print(f"  inserted {nr} relations", flush=True)
        nu = insert_unii(uniis, args.source_version)
        print(f"  inserted {nu} UNII rows", flush=True)
        mariadb_exec(
            "UPDATE rxnorm_ingest_runs SET status='DONE', rows_atoms=%d, rows_rel=%d, "
            "rows_unii=%d, updated_at=NOW() WHERE id=%s" % (na, nr, nu, "'" + run_id + "'"))
    except Exception as e:
        mariadb_exec(f"UPDATE rxnorm_ingest_runs SET status='FAILED', "
                     f"notes={sql_quote(str(e)[:400])}, updated_at=NOW() WHERE id={sql_quote(run_id)}")
        raise

    print(f"\nDONE {args.source_version}: atoms={na} rel={nr} unii={nu}")
    print("next: python3 scripts/rxnorm_primekg_bridge.py --source-version "
          f"{args.source_version}   # build the UNII → PrimeKG crosswalk + coverage")
    print("then: add a /api/v1/knowledge/shared catalog row for 'rxnorm' (same PR).")


if __name__ == "__main__":
    main()
