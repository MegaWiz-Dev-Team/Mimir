#!/usr/bin/env python3
"""
Sprint 54 — SNOMED→ICD-10-TM POC eval (insurance + medical).

Replicates the resolver pipeline via SQL (the same logic as the
/knowledge/snomed/search + /resolve-icd10 endpoints) so we can score
end-to-end without deploying the service:

  phrase ──FULLTEXT(snomed_descriptions)──▶ top-K concepts
         ──snomed_icd10_map──▶ ICD-10-TM billable codes
  hit  := any billable code's 3-char prefix == expected category

Expected codes are at 3-char (category) level — robust to TM's extra
specificity while still proving the right chapter/category is reached.
"""
from __future__ import annotations
import os
import subprocess
import sys

TOP_K = 5  # mirror "NER proposes top-K concepts"

# (phrase, expected ICD-10 3-char category)
GOLD = {
    "insurance": [
        ("essential hypertension", "I10"),
        ("type 2 diabetes mellitus", "E11"),
        ("hyperlipidemia", "E78"),
        ("ischemic heart disease", "I25"),
        ("asthma", "J45"),
        ("chronic kidney disease", "N18"),
        ("chronic hepatitis B", "B18"),
        ("malignant neoplasm of breast", "C50"),
        ("cerebral infarction", "I63"),
        ("obesity", "E66"),
        ("major depressive disorder", "F33"),
        ("osteoarthritis of knee", "M17"),
    ],
    "medical": [
        ("obstructive sleep apnea", "G47"),
        ("chronic insomnia", "G47"),
        ("chronic obstructive pulmonary disease", "J44"),
        ("pneumonia", "J18"),
        ("migraine", "G43"),
        ("gastroesophageal reflux disease", "K21"),
        ("urinary tract infection", "N39"),
        ("iron deficiency anemia", "D50"),
        ("hypothyroidism", "E03"),
        ("atrial fibrillation", "I48"),
        ("epilepsy", "G40"),
        ("rheumatoid arthritis", "M06"),
    ],
}


def mariadb(sql: str) -> str:
    user = os.environ.get("MARIADB_USER", "root")
    pw = os.environ.get("MARIADB_PASS", "root")
    db = os.environ.get("MARIADB_DB", "mimir")
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
        raise RuntimeError(r.stderr.decode()[:400])
    return r.stdout.decode("utf-8")


def pipeline(phrase: str) -> tuple[list[str], list[str]]:
    """Return (billable_tm_codes, concepts) for a phrase. Two steps because
    MariaDB rejects LIMIT inside an IN(...) subquery."""
    q = phrase.replace("'", "''")
    # Step 1 — top-K concepts (FSN + synonyms). Exact term match wins first so a
    # lay synonym equal to the query ("heart attack") beats a partial FSN; FSN
    # preference is only a tiebreaker so the canonical generic still wins among
    # partial matches.
    sql_concepts = (
        "SELECT concept_id FROM snomed_descriptions "
        "WHERE tenant_id IS NULL "
        f"  AND MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) "
        "ORDER BY "
        f"  (LOWER(term) = LOWER('{q}')) DESC, "
        f"  (LOWER(term) IN (LOWER('{q} (disorder)'), LOWER('{q} (finding)'))) DESC, "
        f"  (LOWER(term) LIKE LOWER('{q}%')) DESC, "
        "  (term_type = 'fsn') DESC, "
        "  CHAR_LENGTH(term) ASC, "
        f"  MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) DESC "
        f"LIMIT {TOP_K}"
    )
    concepts = [c.strip() for c in mariadb(sql_concepts).splitlines() if c.strip()]
    if not concepts:
        return [], []
    in_list = ",".join(f"'{c}'" for c in concepts if c.isdigit())
    # Step 2 — billable TM codes for those concepts.
    sql_codes = (
        "SELECT DISTINCT icd10_tm FROM snomed_icd10_map "
        f"WHERE tenant_id IS NULL AND concept_id IN ({in_list}) "
        "  AND icd10_tm IS NOT NULL AND target_role='mandatory' AND needs_review=0"
    )
    codes = [c.strip() for c in mariadb(sql_codes).splitlines() if c.strip()]
    return codes, concepts


def main() -> int:
    grand_hit = grand_total = 0
    for tenant, items in GOLD.items():
        print(f"\n=== {tenant.upper()} ({len(items)} cases) ===")
        hit = 0
        for phrase, expected in items:
            codes, _ = pipeline(phrase)
            prefixes = {c[:3] for c in codes}
            ok = expected in prefixes
            hit += ok
            mark = "✓" if ok else "✗"
            shown = ",".join(sorted(prefixes)[:6]) or "(none)"
            print(f"  {mark} {phrase:42s} exp={expected:4s} got=[{shown}]")
        print(f"  Hit Rate@{TOP_K}: {hit}/{len(items)} = {100*hit/len(items):.0f}%")
        grand_hit += hit
        grand_total += len(items)
    print(f"\n=== OVERALL: {grand_hit}/{grand_total} = {100*grand_hit/grand_total:.0f}% ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
