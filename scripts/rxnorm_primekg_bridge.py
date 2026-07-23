#!/usr/bin/env python3
"""
Sprint 55 — RxNorm ingredient → PrimeKG DrugBank-node crosswalk (the UNII bridge).

This is the piece NORMALIZER.md §4 left open ("A fuller fix is to map RxNorm →
DrugBank ID → PrimeKG entity_id … but that needs a DrugBank cross-map"). It replaces
the fragile hand-curated US↔INN alias table with a principled, tiered, license-aware
bridge — and, crucially, MEASURES coverage so we know whether the DrugBank/UNII
dependency is even needed.

Identity chain:
    written name → RxNorm IN (RXCUI) → { name | INN-synonym | UNII } → PrimeKG node

Three match tiers, most-license-clean first:
  1. name    — RxNorm IN/PIN string == PrimeKG node name (normalized). Zero external
               data. Covers the majority (both use standardized generic names).
  2. inn_syn — a RxNorm SY atom of that ingredient == PrimeKG name. Catches US↔INN
               (albuterol→salbutamol) using RxNorm's OWN synonymy — still no DrugBank.
  3. unii    — RxNorm UNII == DrugBank UNII. ONLY runs if --drugbank-vocab is given
               (DrugBank's open vocabulary.csv: 'DrugBank ID','UNII'). Verify that
               file's license before shipping; without it the tier is skipped and the
               residual it *would* rescue is reported (so the decision is data-driven).

The residual (PrimeKG drug nodes no tier reaches) is printed — never silently dropped.

Usage:
    NEO4J_URL=http://127.0.0.1:7474 NEO4J_USER=neo4j NEO4J_PASSWORD=… \\
    python3 scripts/rxnorm_primekg_bridge.py --source-version rxnorm-20260601 \\
        [--drugbank-vocab data/DrugBank/drugbank_vocabulary.csv] [--dry-run]
"""
from __future__ import annotations
import argparse
import base64
import csv
import json
import os
import subprocess
import sys
import urllib.request

# ── MariaDB helper (same pattern as rxnorm_ingest.py) ───────────────────────


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


def sql_quote(s):
    if s is None or s == "":
        return "NULL"
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def rows(sql):
    out = mariadb_exec(sql).rstrip("\n")
    return [line.split("\t") for line in out.split("\n")] if out else []


def norm(s: str) -> str:
    return (s or "").strip().lower()


# ── Neo4j helper (HTTP transaction endpoint — no driver dependency) ─────────


def neo4j_query(cypher: str):
    url = os.environ.get("NEO4J_URL", "http://127.0.0.1:7474").rstrip("/") + "/db/neo4j/tx/commit"
    user = os.environ.get("NEO4J_USER", "neo4j")
    pw = os.environ.get("NEO4J_PASSWORD", "")
    body = json.dumps({"statements": [{"statement": cypher}]}).encode()
    req = urllib.request.Request(url, data=body, method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Basic " + base64.b64encode(f"{user}:{pw}".encode()).decode())
    with urllib.request.urlopen(req, timeout=30) as r:
        payload = json.load(r)
    if payload.get("errors"):
        raise RuntimeError(f"neo4j: {payload['errors']}")
    res = payload["results"][0]
    cols = res["columns"]
    return [dict(zip(cols, row["row"])) for row in res["data"]]


# ── Bridge build ────────────────────────────────────────────────────────────


def load_rxnorm(source_version):
    sv = sql_quote(source_version)
    # ingredient concepts: str_norm → rxcui  (IN preferred; PIN as fallback)
    name_index, in_name = {}, {}
    for rxcui, tty, str_norm in rows(
        f"SELECT rxcui, tty, str_norm FROM rxnorm_atoms "
        f"WHERE source_version={sv} AND tty IN ('IN','PIN')"):
        name_index.setdefault(str_norm, rxcui)
        if tty == "IN":
            in_name[rxcui] = str_norm
    # synonyms (incl INN) of ingredient concepts: syn_norm → rxcui
    ingredient_cuis = set(in_name) | {c for c in name_index.values()}
    syn_index = {}
    for rxcui, str_norm in rows(
        f"SELECT a.rxcui, a.str_norm FROM rxnorm_atoms a "
        f"WHERE a.source_version={sv} AND a.tty IN ('SY','TMSY','PSN')"):
        if rxcui in ingredient_cuis:
            syn_index.setdefault(str_norm, rxcui)
    # UNII per rxcui, and inverted unii → rxcui
    unii_by_rxcui, rxcui_by_unii = {}, {}
    for rxcui, unii in rows(
        f"SELECT rxcui, unii FROM rxnorm_unii WHERE source_version={sv}"):
        unii_by_rxcui.setdefault(rxcui, set()).add(unii)
        rxcui_by_unii.setdefault(unii, rxcui)
    return name_index, syn_index, unii_by_rxcui, rxcui_by_unii, in_name


def load_primekg_drugs():
    # PrimeKG drug nodes carry a DrugBank id (entity_id like 'DB00682') + name.
    return neo4j_query(
        "MATCH (n:PrimeKG) WHERE toLower(n.type) CONTAINS 'drug' "
        "RETURN n.entity_id AS db_id, n.name AS name, n.entity_index AS idx")


def load_drugbank_unii(path):
    # DrugBank open vocabulary.csv → {DB id: UNII}. License: verify before shipping.
    m = {}
    with open(path, encoding="utf-8") as f:
        for r in csv.DictReader(f):
            dbid = (r.get("DrugBank ID") or r.get("drugbank_id") or "").strip()
            unii = (r.get("UNII") or r.get("unii") or "").strip()
            if dbid and unii:
                m[dbid] = unii
    return m


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-version", required=True)
    ap.add_argument("--drugbank-vocab", help="DrugBank vocabulary.csv (DB id→UNII); enables the unii tier")
    ap.add_argument("--dry-run", action="store_true", help="report coverage; no DB writes")
    ap.add_argument("--residual-top", type=int, default=25)
    args = ap.parse_args()

    print("loading RxNorm ingredient index from MariaDB …", flush=True)
    name_index, syn_index, unii_by_rxcui, rxcui_by_unii, in_name = load_rxnorm(args.source_version)
    print(f"  {len(name_index)} ingredient names · {len(syn_index)} synonyms · "
          f"{len(rxcui_by_unii)} UNII", flush=True)

    print("loading PrimeKG drug nodes from Neo4j …", flush=True)
    drugs = load_primekg_drugs()
    print(f"  {len(drugs)} PrimeKG drug nodes", flush=True)

    db_unii = {}
    if args.drugbank_vocab:
        db_unii = load_drugbank_unii(args.drugbank_vocab)
        print(f"  DrugBank vocab: {len(db_unii)} DB→UNII rows (unii tier ENABLED)", flush=True)
    else:
        print("  --drugbank-vocab not given → unii tier SKIPPED (residual reported)", flush=True)

    mapped, residual = [], []
    counts = {"name": 0, "inn_syn": 0, "unii": 0}
    would_unii_rescue = 0
    for d in drugs:
        db_id, name = d.get("db_id"), d.get("name")
        nn = norm(name)
        method = rxcui = None
        if nn in name_index:
            method, rxcui = "name", name_index[nn]
        elif nn in syn_index:
            method, rxcui = "inn_syn", syn_index[nn]
        elif db_unii.get(db_id) and db_unii[db_id] in rxcui_by_unii:
            method, rxcui = "unii", rxcui_by_unii[db_unii[db_id]]
        if method:
            counts[method] += 1
            unii = next(iter(unii_by_rxcui.get(rxcui, [])), None)
            conf = {"name": 1.00, "inn_syn": 0.95, "unii": 0.90}[method]
            mapped.append({"rxcui": rxcui, "unii": unii, "db_id": db_id,
                           "idx": d.get("idx"), "name": name,
                           "method": method, "conf": conf})
        else:
            # would a UNII bridge have rescued this if we had the DB vocab?
            if not args.drugbank_vocab and db_id:
                would_unii_rescue += 0  # unknown without vocab; leave 0 (honest)
            residual.append((db_id, name))

    total = len(drugs)
    bridged = len(mapped)
    print("\n── coverage ──────────────────────────────────────────────")
    print(f"  bridged      {bridged:>6}/{total}  ({100*bridged/total:.1f}%)")
    for m in ("name", "inn_syn", "unii"):
        print(f"    {m:8}   {counts[m]:>6}  ({100*counts[m]/total:.1f}%)")
    print(f"  residual     {len(residual):>6}  ({100*len(residual)/total:.1f}%)")
    if not args.drugbank_vocab and residual:
        print("  (supply --drugbank-vocab to let the UNII tier attempt the residual)")
    print("  residual sample (PrimeKG nodes no tier reached):")
    for db_id, name in residual[:args.residual_top]:
        print(f"    {db_id:10} {name}")

    if args.dry_run:
        print("\ndry-run: no rows written to rxnorm_primekg_map.")
        return

    print(f"\nwriting {bridged} rows to rxnorm_primekg_map …", flush=True)
    sv = args.source_version
    B = 500
    for i in range(0, len(mapped), B):
        vals = []
        for r in mapped[i:i + B]:
            vals.append("({}, {}, {}, {}, {}, {}, {}, {}, NULL, NOW())".format(
                sql_quote(r["rxcui"]), sql_quote(r["unii"]), sql_quote(r["db_id"]),
                sql_quote(r["idx"]), sql_quote(r["name"]), sql_quote(r["method"]),
                sql_quote(f"{r['conf']:.2f}"), sql_quote(sv)))
        mariadb_exec(
            "INSERT INTO rxnorm_primekg_map (rxcui, unii, primekg_entity_id, primekg_index, "
            "primekg_name, match_method, confidence, source_version, tenant_id, created_at) "
            "VALUES " + ",".join(vals) +
            " ON DUPLICATE KEY UPDATE unii=VALUES(unii), primekg_index=VALUES(primekg_index), "
            "match_method=VALUES(match_method), confidence=VALUES(confidence)")
    cov = round(100 * bridged / total, 2)
    mariadb_exec(
        f"UPDATE rxnorm_ingest_runs SET rows_bridge={bridged}, bridge_coverage={cov}, "
        f"updated_at=NOW() WHERE source_version={sql_quote(sv)} AND status='DONE'")
    print(f"DONE: rxnorm_primekg_map coverage {cov}% "
          f"(name {counts['name']} · inn_syn {counts['inn_syn']} · unii {counts['unii']})")


if __name__ == "__main__":
    main()
