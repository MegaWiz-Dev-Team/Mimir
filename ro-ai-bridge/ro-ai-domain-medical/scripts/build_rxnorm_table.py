#!/usr/bin/env python3
"""Build the brand -> ingredient TSV that the normalizer ships (data/rxnorm_brand_ingredient.tsv).

Two sources, same output format (`brand<TAB>ingredient(s)`; ';'-sep combos; loader takes the
first; lowercased). RxNorm is public domain, so the table ships.

  --from-db     dump ALL brands from the ingested rxnorm_atoms/rxnorm_rel (MariaDB). This is the
                full table (~13.7k brands) that replaces the seed. Requires scripts/rxnorm_ingest.py
                to have run first. (default)
  --from-rxnav  legacy: query RxNav for the ~60-brand seed list (no DB needed; the original path).

Usage:
    python3 build_rxnorm_table.py --from-db          # writes data/rxnorm_brand_ingredient.tsv
    python3 build_rxnorm_table.py --from-rxnav
"""
import argparse
import json
import os
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "data" / "rxnorm_brand_ingredient.tsv"

# ── DB path (full table) ────────────────────────────────────────────────────

# BN --has_tradename/tradename_of--> IN, grouped per brand. Shorter ingredient first so the
# loader's "take first" prefers the base moiety (albuterol before salbutamol → alias handles INN).
DUMP_SQL = """
SELECT bn.str_norm,
       GROUP_CONCAT(DISTINCT LOWER(ing.str) ORDER BY LENGTH(ing.str), ing.str SEPARATOR ';')
FROM rxnorm_atoms bn
JOIN rxnorm_rel   r   ON r.rxcui1 = bn.rxcui AND r.rela IN ('has_tradename','tradename_of')
JOIN rxnorm_atoms ing ON ing.rxcui = r.rxcui2 AND ing.tty='IN'
WHERE bn.tty='BN' AND bn.str_norm <> ''
GROUP BY bn.str_norm;
"""


def mariadb_exec(sql: str) -> str:
    user = os.environ.get("MARIADB_USER", "root")
    pw   = os.environ.get("MARIADB_PASS", "root")
    db   = os.environ.get("MARIADB_DB",   "mimir")
    try:
        subprocess.run(["mysql", "--version"], capture_output=True, check=True)
        host = os.environ.get("MARIADB_HOST", "127.0.0.1")
        port = os.environ.get("MARIADB_PORT", "33306")
        cmd = ["mysql", "-h", host, "-P", port, "-u", user, f"-p{pw}", db, "-B", "-N"]
    except (FileNotFoundError, subprocess.CalledProcessError):
        ns = os.environ.get("MARIADB_NAMESPACE", "asgard-infra")
        cmd = ["kubectl", "-n", ns, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-u", user, f"-p{pw}", db, "-B", "-N"]
    r = subprocess.run(cmd, input=sql.encode("utf-8"), capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(f"mariadb: {r.stderr.decode()[:400]}")
    return r.stdout.decode("utf-8")


def build_alias(pk_file):
    """Generalize us_inn_alias.tsv (was 2 hand entries) — map every RxNorm ingredient synonym to the
    name PrimeKG actually uses, via UNII/DrugBank-id. E.g. paracetamol→acetaminophen, albuterol→
    salbutamol. This is what lets ANY layer (RxNorm or the Thai TMT layer, which emits INN names)
    resolve to a PrimeKG node. Needs rxnorm_ingest.py to have run + primekg_drugs.tsv (id, name)."""
    id2name = {}
    with open(pk_file, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("\t")
            if len(c) >= 2 and c[0].strip() and c[1].strip():
                id2name[c[0].strip()] = c[1].strip().lower()
    raw = mariadb_exec(
        "SELECT LOWER(a.str), u.drugbank_id FROM rxnorm_unii u "
        "JOIN rxnorm_atoms a ON a.rxcui = u.rxcui AND a.tty IN ('IN','PIN','SY','TMSY') "
        "WHERE u.drugbank_id IS NOT NULL AND a.str <> ''"
    ).rstrip("\n")
    by_name = {}  # synonym -> set(primekg canonical names)
    for line in raw.split("\n") if raw else []:
        c = line.split("\t")
        if len(c) < 2:
            continue
        name, pk = c[0].strip(), id2name.get(c[1].strip())
        if not pk or "+" in name or ";" in name or name == pk or len(name) < 3:
            continue
        by_name.setdefault(name, set()).add(pk)
    clean = {n: next(iter(s)) for n, s in by_name.items() if len(s) == 1}  # unambiguous only
    out = OUT.parent / "us_inn_alias.tsv"
    with open(out, "w", encoding="utf-8") as f:
        f.write("# us_or_common<TAB>primekg_canonical — generalized from RxNorm synonyms mapped to the\n")
        f.write("# PrimeKG-preferred name (RxNorm rxcui → UNII/DrugBank-id → primekg_drugs). Replaces\n")
        f.write("# the 2-entry hand seed. Regenerate: build_rxnorm_table.py --build-alias --primekg-names …\n")
        for n in sorted(clean):
            f.write(f"{n}\t{clean[n]}\n")
    print(f"wrote {out}: {len(clean)} aliases ({len(by_name) - len(clean)} ambiguous dropped)")


def load_primekg_names(path):
    """PrimeKG drug node names (lowercased) → set. Used to order each brand's ingredients so the
    name PrimeKG actually uses (acetaminophen not paracetamol, salbutamol not albuterol) is FIRST,
    which is what the loader takes. Built from kg.csv: awk -F, '$5=="drug"{print $4"\\t"$6}'."""
    names = set()
    with open(path, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("\t")
            if len(c) >= 2 and c[1].strip():
                names.add(c[1].strip().lower())
    return names


def build_from_db(source_version_note: str, pk_names=None):
    raw = mariadb_exec(DUMP_SQL).rstrip("\n")
    reordered = 0
    rows = []
    for line in raw.split("\n") if raw else []:
        cols = line.split("\t")
        if len(cols) < 2 or not cols[0] or not cols[1]:
            continue
        brand = cols[0].strip().lower()
        parts = [p for p in cols[1].strip().lower().split(";") if p]
        if pk_names:
            before = parts[0]
            # stable: PrimeKG-present names first, keeping the SQL length order within each group
            parts.sort(key=lambda x: 0 if x in pk_names else 1)
            if parts and parts[0] != before:
                reordered += 1
        rows.append((brand, ";".join(parts)))
    rows.sort()
    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# brand<TAB>ingredient(s) — FULL RxNorm (public domain), dumped from the ingested\n")
        f.write(f"# rxnorm_atoms/rxnorm_rel ({source_version_note}). ';'-sep combos; loader takes first.\n")
        f.write("# First ingredient prefers the name PrimeKG uses (via --primekg-names).\n")
        f.write("# Regenerate: scripts/rxnorm_ingest.py then build_rxnorm_table.py --from-db\n")
        for b, ing in rows:
            f.write(f"{b}\t{ing}\n")
    combos = sum(1 for _, ing in rows if ";" in ing)
    note = f", {reordered} reordered to PrimeKG name" if pk_names else ""
    print(f"wrote {OUT}: {len(rows)} brands ({combos} multi-ingredient{note})")


# ── RxNav path (legacy seed) ────────────────────────────────────────────────

SEED_BRANDS = """tylenol advil motrin aleve coumadin plavix lipitor crestor zocor nexium prilosec
prevacid glucophage januvia lantus ventolin proair advair singulair zyrtec claritin allegra
benadryl prozac zoloft lexapro effexor wellbutrin xanax ativan ambien norvasc toprol cozaar
diovan lasix coreg zithromax cipro levaquin augmentin keflex bactrim flagyl viagra cialis
synthroid medrol neurontin lyrica percocet vicodin morphine eliquis xarelto pradaxa flonase
glucotrol amaryl protonix""".split()


def _get(url):
    with urllib.request.urlopen(url, timeout=8) as r:
        return json.load(r)


def build_from_rxnav():
    def rxcui(name):
        u = "https://rxnav.nlm.nih.gov/REST/rxcui.json?" + urllib.parse.urlencode({"name": name, "search": 2})
        ids = (_get(u).get("idGroup") or {}).get("rxnormId") or []
        return ids[0] if ids else None

    def ingredients(cui):
        d = _get(f"https://rxnav.nlm.nih.gov/REST/rxcui/{cui}/related.json?tty=IN")
        groups = (d.get("relatedGroup") or {}).get("conceptGroup") or []
        return [p["name"].lower() for g in groups if g.get("tty") == "IN"
                for p in g.get("conceptProperties", [])]

    with open(OUT, "w", encoding="utf-8") as out:
        out.write("# brand<TAB>ingredient(s) — built dev-time from RxNav (RxNorm, public domain).\n")
        n = 0
        for b in SEED_BRANDS:
            try:
                cui = rxcui(b)
                ings = ingredients(cui) if cui else []
                if ings:
                    out.write(f"{b}\t{';'.join(ings)}\n"); n += 1
                    print(f"  {b:12} -> {';'.join(ings)}", flush=True)
                else:
                    print(f"  {b:12} -> (miss)", flush=True)
            except Exception as e:
                print(f"  {b:12} -> ERR {e}", flush=True)
    print(f"DONE: {n}/{len(SEED_BRANDS)} resolved", flush=True)


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--from-db", action="store_true", help="dump full table from MariaDB (default)")
    g.add_argument("--from-rxnav", action="store_true", help="legacy ~60-brand seed via RxNav")
    g.add_argument("--build-alias", action="store_true", help="generate us_inn_alias.tsv (needs --primekg-names)")
    ap.add_argument("--source-version", default="rxnorm-full", help="note written into the TSV header")
    ap.add_argument("--primekg-names", help="TSV of PrimeKG drug (id, name) — orders ingredients "
                                            "so PrimeKG's preferred name is first")
    args = ap.parse_args()
    if args.from_rxnav:
        build_from_rxnav()
    elif args.build_alias:
        if not args.primekg_names:
            sys.exit("--build-alias needs --primekg-names")
        build_alias(args.primekg_names)
    else:
        pk = load_primekg_names(args.primekg_names) if args.primekg_names else None
        build_from_db(args.source_version, pk)


if __name__ == "__main__":
    main()
