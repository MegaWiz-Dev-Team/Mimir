#!/usr/bin/env python3
"""
Sprint 48 — ICD-10 / ICD-10-TM lookup CLI.

⚠ DEPRECATED for application use — the Rust handler shipped.
   - HTTP endpoint: `GET /api/v1/icd10/lookup?q=…&locale=th|en|both`
     (Mimir `ro-ai-bridge/src/routes/icd10.rs`)
   - MCP tool:      `icd10_tm_lookup` in Hermodr `services/eir_medical.rs`
     — calls the Mimir endpoint via the eir_medical sidecar.

   This script remains as an OPERATOR / OPS shim only. It opens a
   `docker exec` shell into the MariaDB pod and queries `icd10_codes`
   directly — useful when the HTTP stack is down and you need to verify
   ingest, sanity-check coverage, or debug a deploy. Don't wire it into
   application or CI flows; use the HTTP endpoint or MCP tool instead.

   For Eir agents in Thai clinical workflows: prefer the Thai-aware
   `icd10_tm_lookup` MCP tool (returns bilingual th_label + en_label +
   DRG mapping). Hermodr's older `icd10_lookup` proxies NLM US
   Clinical Tables (ICD-10-CM, English only) — international research
   only.

─── Original docstring ─────────────────────────────────────────────────

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
QDRANT_URL = "http://localhost:6333"
OLLAMA_URL = "http://localhost:11434"
EMBED_MODEL = "bge-m3"   # multilingual, dim=1024 — fixes Thai semantic gap
QDRANT_COLLECTION = "icd10-th"


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


_MEDICAL_ACRONYMS = {
    # Cardio
    "STEMI": "ST elevation myocardial infarction",
    "NSTEMI": "Non ST elevation myocardial infarction",
    "MI": "myocardial infarction",
    "AMI": "acute myocardial infarction",
    "CHF": "congestive heart failure",
    "CABG": "coronary artery bypass graft",
    "AFib": "atrial fibrillation",
    "AF": "atrial fibrillation",
    "DVT": "deep vein thrombosis",
    "PE": "pulmonary embolism",
    "HTN": "hypertension",
    # Pulm
    "COPD": "chronic obstructive pulmonary disease",
    "URTI": "upper respiratory tract infection",
    "ARDS": "acute respiratory distress syndrome",
    "PNA": "pneumonia",
    # Endo / metabolic
    "T1DM": "type 1 diabetes mellitus",
    "T2DM": "type 2 diabetes mellitus",
    "DM": "diabetes mellitus",
    "DKA": "diabetic ketoacidosis",
    # Neuro
    "CVA": "cerebrovascular accident stroke",
    "TIA": "transient ischemic attack",
    # Renal
    "AKI": "acute kidney injury",
    "CKD": "chronic kidney disease",
    "ESRD": "end stage renal disease",
    "UTI": "urinary tract infection",
    # GI / liver
    "GERD": "gastroesophageal reflux disease",
    "IBD": "inflammatory bowel disease",
    "GIB": "gastrointestinal bleeding",
    # Pediatrics / OB
    "RDS": "respiratory distress syndrome",
    "PROM": "premature rupture of membranes",
    # Psych
    "MDD": "major depressive disorder",
    "GAD": "generalized anxiety disorder",
    "PTSD": "post traumatic stress disorder",
    "OCD": "obsessive compulsive disorder",
}


def expand_acronyms(query: str) -> str:
    """Expand medical acronyms inline — preserves rest of query.
    'STEMI inferior' → 'ST elevation myocardial infarction inferior'.
    Case-insensitive token match; preserves original tokens that aren't
    in the dictionary."""
    import re as _re
    tokens = _re.split(r"(\s+)", query)  # keep whitespace
    expanded = []
    for t in tokens:
        # Strip surrounding punctuation for match.
        m = _re.match(r"^([A-Za-z][A-Za-z0-9]*)([^\w]?)$", t)
        if m:
            word, suffix = m.group(1), m.group(2)
            up = word.upper()
            if up in _MEDICAL_ACRONYMS:
                expanded.append(_MEDICAL_ACRONYMS[up] + suffix)
                continue
        expanded.append(t)
    out = "".join(expanded)
    return out if out != query else query


def search_semantic(query: str, limit: int, source_version: str) -> list[dict]:
    """Qdrant + Ollama embedding semantic search. Returns rows in same
    shape as MariaDB search() — payload is enriched from Qdrant directly.

    Pre-processes query through medical-acronym expansion (STEMI → 'ST
    elevation myocardial infarction'), which closes the acronym gap that
    pure embedding can't bridge (BGE-M3 has no clue what STEMI means)."""
    import urllib.request as _ur
    # Acronym expansion before embedding.
    expanded = expand_acronyms(query)
    # Embed query (use expanded form).
    req = _ur.Request(
        f"{OLLAMA_URL}/api/embeddings",
        data=json.dumps({"model": EMBED_MODEL, "prompt": expanded}).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with _ur.urlopen(req, timeout=15) as resp:
        vec = json.loads(resp.read())["embedding"]

    # Qdrant search.
    body = json.dumps({
        "vector": vec,
        "limit": int(limit),
        "with_payload": True,
        "filter": {"must": [
            {"key": "source_version", "match": {"value": source_version}}
        ]},
    }).encode("utf-8")
    req = _ur.Request(
        f"{QDRANT_URL}/collections/{QDRANT_COLLECTION}/points/search",
        data=body, headers={"Content-Type": "application/json"},
    )
    with _ur.urlopen(req, timeout=10) as resp:
        result = json.loads(resp.read())["result"]

    rows: list[dict] = []
    for hit in result:
        p = hit.get("payload", {}) or {}
        rows.append({
            "code": p.get("code"),
            "en_label": p.get("en_label"),
            "th_label": p.get("th_label"),
            "chapter": p.get("chapter"),
            "billable": "1",
            "source_version": p.get("source_version"),
            "_score": round(hit.get("score", 0.0), 4),
        })
    return rows


_THAI_RANGE = "฀-๿"


def has_thai(s: str) -> bool:
    import re as _re
    return bool(_re.search(f"[{_THAI_RANGE}]", s))


def search_auto(query: str, locale: str, limit: int,
                source_version: str) -> tuple[str, list[dict]]:
    """Cascade: exact → naive → semantic (multilingual).

    Now uses BGE-M3 multilingual embeddings (B-48f.2 upgrade), so Thai
    queries route through semantic too — closes the Thai phrasing gap
    (e.g. 'หลอดเลือดสมองตีบ' → 'เนื้อสมองตายเพราะขาดเลือด' via cosine
    0.60 vs. 0.38 noise).
    """
    rows = search(query, "exact", locale, limit, source_version)
    if rows:
        return "exact", rows
    rows = search(query, "naive", locale, limit, source_version)
    if rows:
        return "naive", rows
    try:
        rows = search_semantic(query, limit, source_version)
        return "semantic", rows
    except Exception as e:
        print(f"  [semantic-fail] {e}", file=sys.stderr)
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
    ap.add_argument("--mode", choices=["auto", "exact", "prefix", "naive", "semantic"], default="auto")
    ap.add_argument("--locale", choices=["en", "th", "both"], default="both")
    ap.add_argument("--limit", type=int, default=10)
    ap.add_argument("--source-version", default=DEFAULT_VERSION)
    ap.add_argument("--json", action="store_true", help="emit JSON instead of human-readable")
    args = ap.parse_args()

    if args.mode == "auto":
        matched_mode, rows = search_auto(args.query, args.locale, args.limit,
                                         args.source_version)
    elif args.mode == "semantic":
        matched_mode = "semantic"
        rows = search_semantic(args.query, args.limit, args.source_version)
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
