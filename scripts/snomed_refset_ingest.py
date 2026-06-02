#!/usr/bin/env python3
"""
Sprint 58 — SNOMED simple-refset + EDQM dose-map ingest.

Source: SNOMED International via MLDS (https://mlds.ihtsdotools.org/#/userDashboard),
Thailand member, IHTSDO Affiliate License 2023. Packages already on disk under
$MIMIR_KB/SnomedCT/. See docs/03_implementation_plans/03_19_*.md.

Four independent sub-commands (run any subset):
  --ips   FILE   der2_Refset_IPSSimpleSnapshot...txt   → snomed_refset_members(ips)
  --gpfp  FILE   der2_Refset_GPFPSimpleSnapshot...txt  → snomed_refset_members(gpfp)
  --edqm  FILE   der2_ssRefset_EDQMSimpleMapSnapshot...txt → snomed_edqm_dose_map
  --tmt-dose-link  derive snomed_tmt_dose_link by text-matching TMT GP/GPU dose-form
                   fragments to SNOMED dose-form concepts (exact|normalized).

Prereq: snomed_descriptions populated with International FSNs (run
snomed_icd10_map_ingest.py --desc-file first). Snapshot files only.

Usage:
  python3 snomed_refset_ingest.py \\
      --ips  "$MIMIR_KB/SnomedCT/SnomedCT_IPS_PRODUCTION_20250930T120000Z/Snapshot/Refset/Content/der2_Refset_IPSSimpleSnapshot_INT_20250701.txt" \\
      --source-version sct-ips-20250701 --source-url https://mlds.ihtsdotools.org/
"""
from __future__ import annotations
import argparse
import os
import re
import subprocess
import sys
import uuid
from pathlib import Path

# EDQM correlation concepts (RF2 correlationId column).
CORRELATION = {
    "447557004": "exact", "447559001": "broad", "447558009": "narrow",
}
DOSE_FORM_SEMTAG = "(dose form)"

# GPU unit-of-use packaging terms that surface in the dose-form tail but are NOT
# pharmaceutical dose forms — excluded from the TMT→SNOMED dose link (reported, not
# silently dropped, so coverage math stays honest).
CONTAINERS = {
    "bottle", "vial", "tube", "jar", "ampoule", "prefilled syr", "prefilled syringe",
    "sachet", "blister", "cartridge", "bag", "pack", "dropper bottle", "prefilled pen",
    "box", "pouch", "can", "strip", "applicator", "stick",
}

# Curated TMT-fragment → SNOMED dose-form concept map. These are the high-frequency
# fragments where TMT OMITS the route ("tablet", "cream") so the form is genuinely
# ambiguous to token-matching (bare "tablet" ⊆ oral/sublingual/buccal/vaginal tablet
# equally) — token_subset deliberately refuses to guess them. Thai TMT convention is
# the default route (oral for tablets/capsules, cutaneous for creams/ointments/gels),
# so these are resolved by curation, not inference. All targets verified EDQM-mapped
# (carry an EDQM code). Curated links are TRUSTED (needs_review=0) and win over the
# token_subset tier (e.g. upgrade "coated tablet").
CURATED_DOSE_ALIASES = {
    "tablet": "421026006",              # Oral tablet
    "capsule, hard": "1217287006",      # Hard oral capsule
    "capsule, soft": "1217288001",      # Soft oral capsule
    "cream": "421628006",               # Cutaneous cream
    "ointment": "425753008",            # Cutaneous ointment
    "gel": "421949005",                 # Cutaneous gel
    "effervescent tablet": "764780001",  # Effervescent oral tablet
    "coated tablet": "1230389004",      # Coated oral tablet
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


def batched_insert(prefix: str, rows: list[str], batch: int = 500, dry: bool = False) -> int:
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


def _guard_appledouble(path: Path) -> None:
    """T7 is exFAT; macOS unzip drops `._*` AppleDouble siblings that are NOT RF2.
    Parsing one silently corrupts the load — refuse it explicitly."""
    if path.name.startswith("._"):
        raise SystemExit(f"refusing AppleDouble resource fork: {path}")


def _rf2_rows(path: Path):
    """Yield active RF2 rows as column lists. Skips header + inactive."""
    _guard_appledouble(path)
    with path.open(encoding="utf-8") as f:
        f.readline()  # header
        for line in f:
            cols = line.rstrip("\n").split("\t")
            # id effectiveTime active moduleId refsetId referencedComponentId [...]
            if len(cols) < 6 or cols[2].strip() != "1":
                continue
            yield cols


# ── simple refset membership (IPS, GP/FP) ────────────────────────────────────

def ingest_simple_refset(path: Path, refset_key: str, source_version: str, dry: bool) -> int:
    rows: list[str] = []
    for cols in _rf2_rows(path):
        refset_id = cols[4].strip()
        concept_id = cols[5].strip()
        if not concept_id:
            continue
        rows.append("(" + ",".join([
            sql_quote(refset_key), sql_quote(refset_id), sql_quote(concept_id),
            sql_quote(source_version), "1",
        ]) + ")")
    prefix = ("INSERT IGNORE INTO snomed_refset_members "
              "(refset_key, refset_id, concept_id, source_version, active) VALUES\n")
    return batched_insert(prefix, rows, dry=dry)


# ── EDQM SimpleMap (SNOMED dose-form concept → EDQM code) ─────────────────────

def ingest_edqm(path: Path, source_version: str, dry: bool) -> int:
    rows: list[str] = []
    for cols in _rf2_rows(path):
        concept_id = cols[5].strip()        # referencedComponentId = SNOMED dose form
        map_target = cols[6].strip() if len(cols) > 6 else ""   # EDQM code
        corr_id = cols[7].strip() if len(cols) > 7 else ""      # correlationId
        if not concept_id or not map_target:
            continue
        rows.append("(" + ",".join([
            sql_quote(concept_id), sql_quote(map_target),
            sql_quote(corr_id or None), sql_quote(source_version), "1",
        ]) + ")")
    prefix = ("INSERT IGNORE INTO snomed_edqm_dose_map "
              "(snomed_concept_id, edqm_code, correlation_id, source_version, active) VALUES\n")
    return batched_insert(prefix, rows, dry=dry)


# ── TMT GP/GPU dose-form → SNOMED dose-form concept (text match) ──────────────

# Strength/ratio tokens, e.g. "5.5 g", "100 mL", "1 mg/1 mL", "5 %".
_STRENGTH_RE = re.compile(
    r"\d+(?:\.\d+)?\s*(?:mg|ml|g|mcg|microgram|iu|%|unit|units|mmol)\b"
    r"(?:\s*/\s*\d+(?:\.\d+)?\s*(?:mg|ml|g|mcg|microgram|iu|%|unit|units|mmol)?\b)?",
    re.I)


def _dose_fragment(fsn: str) -> str:
    """Isolate the dose-form tail of a TMT GP/GPU FSN.

    TMT FSN = '<substance> <strength> <dose form> (GP|GPU)'. The dose form is the
    text AFTER the last strength token, e.g.
      'cefazolin 5.5 g/100 mL ear/eye drops, solution (GP)' → 'ear/eye drops, solution'
      'enalapril maleate 1 mg/1 mL oral suspension (GP)'    → 'oral suspension'
    Matching the WHOLE FSN never hits a pure dose-form concept (the substance name
    is always present) — that is why the naive pass returned 0.
    """
    s = re.sub(r"\s*\((?:GP|GPU|TP|TPU)\)\s*$", "", fsn.strip(), flags=re.I)
    last = None
    for m in _STRENGTH_RE.finditer(s):
        last = m
    tail = s[last.end():] if last else s
    # Drop TMT unit-of-use quantity suffix, e.g. "film-coated tablet, 1 tablet".
    tail = re.sub(r",\s*\d+\s+\w+.*$", "", tail)
    return tail.strip(" ,;").lower()


def _normalize_dose(text: str) -> str:
    t = re.sub(r"[^a-z\s]", " ", text.lower())
    return re.sub(r"\s+", " ", t).strip()


def ingest_tmt_dose_link(source_version: str, dry: bool) -> tuple[int, int]:
    """Match each TMT GP/GPU dose-form fragment to a SNOMED dose-form concept.
    exact → normalized only (no fuzzy dependency); low/none → needs_review."""
    # All terms (FSN + synonym) of dose-form concepts, prioritising EDQM-mapped ones
    # so a hit resolves to a concept that carries an EDQM code for FHIR. The EDQM-
    # aligned strings ("Film-coated oral tablet") live in SYNONYMS, not the verbose
    # "Conventional release ..." FSN — indexing FSN only is why coverage was ~0.
    df = mariadb_exec(
        "SELECT d.concept_id, d.term, "
        "  (e.snomed_concept_id IS NOT NULL) AS has_edqm "
        "FROM snomed_descriptions d "
        "LEFT JOIN snomed_edqm_dose_map e ON e.snomed_concept_id=d.concept_id "
        "WHERE d.active=1 AND d.concept_id IN ("
        "    SELECT concept_id FROM snomed_descriptions WHERE semantic_tag='dose form' "
        "    UNION SELECT snomed_concept_id FROM snomed_edqm_dose_map)")
    exact_idx: dict[str, str] = {}
    norm_idx: dict[str, str] = {}
    # token-set → (cid, is_edqm), for tier-3 subset matching across naming variants
    # (TMT "film-coated tablet" vs SNOMED "film-coated oral tablet").
    tok_idx: list[tuple[frozenset, str, bool]] = []
    edqm_concepts: set[str] = set()
    for ln in df.splitlines():
        parts = ln.split("\t")
        if len(parts) < 2:
            continue
        cid, term = parts[0], parts[1]
        is_edqm = len(parts) > 2 and parts[2] == "1"
        if is_edqm:
            edqm_concepts.add(cid)
        bare = re.sub(r"\s*\(dose form\)\s*$", "", term, flags=re.I).strip().lower()
        # Prefer EDQM-mapped concept when the same string maps to several.
        if bare not in exact_idx or cid in edqm_concepts:
            exact_idx[bare] = cid
        nb = _normalize_dose(bare)
        if nb and (nb not in norm_idx or cid in edqm_concepts):
            norm_idx[nb] = cid
        toks = frozenset(nb.split())
        if toks:
            tok_idx.append((toks, cid, is_edqm))

    def token_subset(frag_tokens: frozenset) -> str | None:
        """Best concept whose token-set is a SUPERSET of the TMT fragment's tokens
        (TMT form is the more specific/abbreviated label). Rank by fewest extra
        tokens, then EDQM-mapped. Returns None when the top rank is a TIE across
        different concepts — e.g. bare "tablet" sits under oral/sublingual/buccal
        tablet equally, so guessing one would assert a wrong subtype. Ambiguous
        fragments fall through to the curation backlog instead."""
        ranked = []  # (extra_count, not_edqm, cid)
        for toks, cid, is_edqm in tok_idx:
            if frag_tokens <= toks:
                ranked.append((len(toks) - len(frag_tokens), not is_edqm, cid))
        if not ranked:
            return None
        ranked.sort()
        top_key = ranked[0][:2]
        winners = {c for ex, ne, c in ranked if (ex, ne) == top_key}
        return ranked[0][2] if len(winners) == 1 else None

    tmt = mariadb_exec(
        "SELECT tmt_id, fsn FROM tmt_codes WHERE concept_type IN ('GP','GPU') "
        "AND tenant_id IS NULL")
    rows: list[str] = []
    review = 0
    container = 0
    for ln in tmt.splitlines():
        if "\t" not in ln:
            continue
        tmt_id, fsn = ln.split("\t", 1)
        frag = _dose_fragment(fsn)
        if frag in CONTAINERS:        # GPU unit packaging, not a dose form — N/A
            container += 1
            continue
        method = conf = cid = None
        if frag in CURATED_DOSE_ALIASES:   # tier 0: human-curated, trusted, wins over token_subset
            cid, method, conf = CURATED_DOSE_ALIASES[frag], "curated", "1.000"
        elif frag in exact_idx:
            cid, method, conf = exact_idx[frag], "exact", "1.000"
        else:
            nf = _normalize_dose(frag)
            if nf and nf in norm_idx:
                cid, method, conf = norm_idx[nf], "normalized", "0.900"
            elif nf:
                # tier 3: token-subset — flagged needs_review (not exact equivalence).
                sc = token_subset(frozenset(nf.split()))
                if sc:
                    cid, method, conf = sc, "token_subset", "0.700"
        if not cid:
            review += 1
            continue
        needs = 1 if float(conf) < 0.85 else 0
        review += needs
        rows.append("(" + ",".join([
            sql_quote(tmt_id), sql_quote(cid), sql_quote(method), conf,
            str(needs), sql_quote(source_version),
        ]) + ")")
    prefix = ("INSERT IGNORE INTO snomed_tmt_dose_link "
              "(tmt_id, snomed_concept_id, match_method, confidence, needs_review, "
              "source_version) VALUES\n")
    inserted = batched_insert(prefix, rows, dry=dry)
    print(f"  container/packaging skipped (not dose forms): {container}")
    return inserted, review


def _record_run(refset_key, source_version, source_label, source_url, inserted,
                review, status, dry):
    if dry:
        return
    rid = str(uuid.uuid4())
    mariadb_exec(
        "INSERT INTO snomed_refset_ingest_runs "
        "(id, refset_key, source_version, source_label, source_url, rows_inserted, "
        "rows_review, status, finished_at) VALUES ("
        f"{sql_quote(rid)}, {sql_quote(refset_key)}, {sql_quote(source_version)}, "
        f"{sql_quote(source_label)}, {sql_quote(source_url)}, {inserted}, {review}, "
        f"{sql_quote(status)}, NOW())")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ips", type=Path)
    ap.add_argument("--gpfp", type=Path)
    ap.add_argument("--edqm", type=Path)
    ap.add_argument("--tmt-dose-link", action="store_true")
    ap.add_argument("--source-version", default="sct-20250701")
    ap.add_argument("--source-label", default="mlds-snomed-intl")
    ap.add_argument("--source-url", default="https://mlds.ihtsdotools.org/")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if not any([args.ips, args.gpfp, args.edqm, args.tmt_dose_link]):
        ap.error("provide at least one of --ips/--gpfp/--edqm/--tmt-dose-link")

    if args.ips:
        n = ingest_simple_refset(args.ips, "ips", args.source_version, args.dry_run)
        print(f"IPS members inserted: {n}")
        _record_run("ips", args.source_version, args.source_label, args.source_url,
                    n, 0, "done", args.dry_run)
    if args.gpfp:
        n = ingest_simple_refset(args.gpfp, "gpfp", args.source_version, args.dry_run)
        print(f"GP/FP members inserted: {n}")
        _record_run("gpfp", args.source_version, args.source_label, args.source_url,
                    n, 0, "done", args.dry_run)
    if args.edqm:
        n = ingest_edqm(args.edqm, args.source_version, args.dry_run)
        print(f"EDQM dose-map rows inserted: {n}")
        _record_run("edqm", args.source_version, args.source_label, args.source_url,
                    n, 0, "done", args.dry_run)
    if args.tmt_dose_link:
        n, review = ingest_tmt_dose_link(args.source_version, args.dry_run)
        print(f"TMT→SNOMED dose links inserted: {n} (needs_review/unmatched: {review})")
        _record_run("tmt_dose_link", args.source_version, args.source_label,
                    args.source_url, n, review, "done", args.dry_run)

    print("Done." + (" (dry-run)" if args.dry_run else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
