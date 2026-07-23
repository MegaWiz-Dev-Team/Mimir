#!/usr/bin/env python3
"""Build Thai-trade → generic TSV from TMT (dm+d), for Lane B of the normalizer.

Dev-time batch job (mirrors build_rxnorm_table.py): traverses the TMT dm+d hierarchy
in MariaDB once and dumps a static `data/tmt_thai_generic.tsv` that ships compiled into
DrugDiseaseNormalizer via include_str! — no runtime DB. This is the Thai lane the RxNorm
(US/EN) table can't cover (NORMALIZER.md §8).

TMT layers flow general→specific (SUBS→VTM→GP→TP). To reach the generic moiety of a
Thai trade product we climb UP (edges traversed in reverse). The canonical general form
is the recursive CTE:

    WITH RECURSIVE climb AS (
      SELECT tmt_id, concept_type, fsn, 0 depth FROM tmt_codes WHERE tmt_id = :start
      UNION ALL
      SELECT c.tmt_id, c.concept_type, c.fsn, climb.depth+1
      FROM climb
      JOIN tmt_relationships r ON r.to_id = climb.tmt_id     -- reverse: specific→general
      JOIN tmt_codes c ON c.tmt_id = r.from_id
      WHERE climb.depth < 6)
    SELECT fsn FROM climb WHERE concept_type='VTM' LIMIT 1;

For the bulk build we use the equivalent fast 4-join along the known TP←GP←VTM path.

TP FSN format:  "SARA (ไทยนครพัฒนา) (paracetamol 325 mg) tablet (TP)"
  brand   = text before the first '('        → "sara"
  generic = VTM FSN minus " (VTM)"           → "paracetamol"

Ambiguity guard: a brand string that resolves to >1 distinct generic is DROPPED (and
logged), never guessed — a wrong brand→generic feeds the safety pruner a false node.

Usage:
    python3 build_tmt_table.py                 # writes data/tmt_thai_generic.tsv
    python3 build_tmt_table.py --limit 50      # sample to stdout, no write
"""
from __future__ import annotations
import argparse
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "data" / "tmt_thai_generic.tsv"

BULK_SQL = """
SELECT tp.fsn, vtm.fsn
FROM tmt_codes tp
JOIN tmt_relationships r1 ON r1.rel_type='GPtoTP'  AND r1.to_id   = tp.tmt_id
JOIN tmt_codes gp         ON gp.tmt_id  = r1.from_id AND gp.concept_type='GP'
JOIN tmt_relationships r2 ON r2.rel_type='VTMtoGP' AND r2.to_id   = gp.tmt_id
JOIN tmt_codes vtm        ON vtm.tmt_id = r2.from_id AND vtm.concept_type='VTM'
WHERE tp.concept_type='TP';
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


def brand_of(tp_fsn: str) -> str | None:
    """Leading brand token(s) before the first parenthetical/type suffix."""
    s = tp_fsn.split("(")[0].strip()
    s = re.sub(r"\s+", " ", s).strip().lower()
    return s or None


# Salt/hydrate suffixes to strip from a single-ingredient moiety so it matches PrimeKG's
# node name ("Metformin", not "Metformin hydrochloride"). Combos (with '+') are left alone.
SALTS = {
    "hydrochloride", "hcl", "sodium", "potassium", "calcium", "magnesium",
    "sulfate", "sulphate", "maleate", "succinate", "tartrate", "mesylate", "mesilate",
    "besylate", "besilate", "fumarate", "acetate", "citrate", "phosphate", "nitrate",
    "bromide", "chloride", "hydrobromide", "gluconate", "lactate", "valerate",
    "propionate", "dipropionate", "furoate", "xinafoate", "embonate", "pamoate",
    "monohydrate", "dihydrate", "trihydrate", "hydrate", "anhydrous",
}


def generic_of(vtm_fsn: str) -> str | None:
    g = re.sub(r"\s*\(vtm\)\s*$", "", vtm_fsn.strip(), flags=re.I)
    g = re.sub(r"\s+", " ", g).strip().lower()
    if g and "+" not in g:  # single ingredient → strip trailing salt/hydrate tokens
        toks = g.split()
        while len(toks) > 1 and toks[-1] in SALTS:
            toks.pop()
        g = " ".join(toks)
    return g or None


def strip_trailing_num(brand: str) -> str | None:
    """'metica 500' → 'metica' (a looser secondary key), if it changes anything."""
    b = re.sub(r"\s+\d[\d.]*\s*$", "", brand).strip()
    return b if b and b != brand else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=0, help="sample N pairs to stdout, no write")
    args = ap.parse_args()

    raw = mariadb_exec(BULK_SQL).rstrip("\n")
    pairs = []
    for line in raw.split("\n") if raw else []:
        cols = line.split("\t")
        if len(cols) < 2:
            continue
        b, g = brand_of(cols[0]), generic_of(cols[1])
        if not (b and g and len(b) >= 2):
            continue
        # skip generic-name-as-brand: if the "brand" is just the generic (a prefix either
        # way), it's not a trade name — RxNorm/pass-through already handle it, and mapping
        # e.g. metformin→"metformin" would clobber a canonical generic.
        if g.startswith(b) or b.startswith(g):
            continue
        pairs.append((b, g))
        sb = strip_trailing_num(b)
        if sb and len(sb) >= 3 and not (g.startswith(sb) or sb.startswith(g)):
            pairs.append((sb, g))

    # brand → set of generics; keep only unambiguous (exactly one distinct generic)
    by_brand: dict[str, set[str]] = defaultdict(set)
    for b, g in pairs:
        by_brand[b].add(g)
    clean = {b: next(iter(gs)) for b, gs in by_brand.items() if len(gs) == 1}
    ambiguous = {b: gs for b, gs in by_brand.items() if len(gs) > 1}

    if args.limit:
        for b, g in list(clean.items())[:args.limit]:
            print(f"{b}\t{g}")
        print(f"\n# {len(clean)} unambiguous · {len(ambiguous)} ambiguous(dropped) "
              f"· from {len(pairs)} trade↔generic pairs", file=sys.stderr)
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# thai-brand<TAB>generic — built dev-time from TMT (THIS-Center/MoPH, "
                "free in TH). Lane B of the drug normalizer; ships compiled in.\n")
        f.write(f"# {len(clean)} unambiguous brands; {len(ambiguous)} ambiguous dropped "
                f"(a brand mapping to >1 generic is unsafe to guess).\n")
        for b in sorted(clean):
            f.write(f"{b}\t{clean[b]}\n")
    print(f"wrote {OUT}: {len(clean)} brands "
          f"({len(ambiguous)} ambiguous dropped, {len(pairs)} raw pairs)")
    # never silently ship a partial table as complete — surface a few dropped brands
    for b in list(ambiguous)[:8]:
        print(f"  ambiguous(dropped): {b} -> {sorted(ambiguous[b])[:3]}")


if __name__ == "__main__":
    main()
