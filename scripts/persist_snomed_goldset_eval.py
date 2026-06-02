#!/usr/bin/env python3
"""X2 (Sprint 59) — SNOMED gold-set regression benchmarks → Mimir eval.

Extends the B4 coverage eval (scripts/persist_snomed_refset_eval.py) with three
*gold-set* datasets so every later change (X1 enrichment, dose-link curation,
extraction-model swap) is measured, not assumed. Same persist pattern / eval
tables / tenant; low-level helpers (sql/qstr/count) are imported from the B4
script so there is one source of truth for the DB plumbing.

Three datasets (each persisted as its own eval run, tenant asgard_platform):

  1. snomed-doseform-precision
     Per match tier, does the TMT→SNOMED dose-link resolve to a *real* SNOMED
     dose-form concept (FSN ends "(dose form)")? Plus EDQM-enrichment coverage.
     HONEST SCOPE: this is a VALIDITY check (target is a genuine dose form), NOT
     subtype accuracy — we have no per-TMT human gold beyond the `curated` tier.
     token_subset is the risk tier: needs_review=1, so the resolver returns null
     (never auto-codes it); it is reported as info, not graded.

  2. snomed-coding-validity
     The 7 labelled claims-pipeline fixtures (data/abb/extractions +
     data/abb/fhir_r5) → every ICD-10-TM code asserted in-scope (resolves in
     icd10_codes). HONEST SCOPE: a vocabulary in-scope check ("is the code real
     & known to Mimir"), NOT human-label accuracy. Low rate is itself the
     signal: the fixtures carry ICD-10-CM/WHO 4th-character codes (e.g. N39.0)
     that are outside Mimir's ICD-10-TM table — a real interoperability gap X1
     must close. SNOMED-coded and TMT-dose-coded entities are counted too (the
     fixtures currently carry none → reported n=0, not silently skipped).

  3. snomed-ips-coverage
     The fixture Condition concepts (ICD-10-TM) mapped via snomed_icd10_map →
     SNOMED → checked for IPS refset membership. Coverage = % of distinct
     fixture codes that reach at least one IPS-member SNOMED concept — an
     interoperability score tracked over time.

Baseline floors are set just under the 2026-06-02 actuals so a real regression
fails while a same-release re-ingest stays green.

RUNBOOK: run after any dose-link / refset / ICD-10-TM re-ingest (alongside
scripts/persist_snomed_refset_eval.py):
  python3 scripts/persist_snomed_goldset_eval.py
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import sys
import urllib.request
import uuid

# Reuse the proven DB plumbing from the B4 coverage eval (single source of truth).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from persist_snomed_refset_eval import sql, qstr, count  # noqa: E402

TENANT = "asgard_platform"
FIXTURE_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "abb")
SYS_SNOMED = "http://snomed.info/sct"
# Mimir knowledge API for the term-path IPS check (the path Iris actually uses via
# /claims/:id/ips-coverage). On-host → in-cluster NodePort uses the node IP.
MIMIR_BASE = os.environ.get("MIMIR_BASE", "http://192.168.139.2:30000")

# Floors — a graded case fails when the live value drops below these.
FLOORS = {
    "doseform_valid_curated": 0.99,
    "doseform_valid_exact": 0.99,
    "doseform_valid_normalized": 0.99,
    "icd10tm_in_scope": 0.95,
    "ips_coverage": 0.75,
    "term_ips_coverage": 0.65,
}


# ─── generalized persist (mirrors persist_snomed_refset_eval.persist) ──────────
def persist(suite, source, description, variable, rows, model_id, graded_groups):
    """Persist one dataset's rows as an eval run. `graded_groups` = the row
    groups that count toward pass/fail + accuracy; everything else is info."""
    ds_id = sql(f"SELECT id FROM eval_benchmark_datasets WHERE name={qstr(suite)} "
                f"AND tenant_id={qstr(TENANT)} LIMIT 1")
    if not ds_id:
        ds_id = str(uuid.uuid4())
        meta = json.dumps([{"id": r["item_id"], "input": r["input"], "group": r["group"]}
                           for r in rows], ensure_ascii=False)
        sql("INSERT INTO eval_benchmark_datasets (id,tenant_id,name,source,scoring_fn,"
            "description,items,total_items,version,is_active) VALUES ("
            + ",".join([qstr(ds_id), qstr(TENANT), qstr(suite), qstr(source),
                        qstr("threshold"), qstr(description),
                        qstr(meta), str(len(rows)), "1", "1"]) + ")")
    run_id = str(uuid.uuid4())
    cfg = {"benchmark_dataset_id": ds_id, "model": model_id, "agent": source,
           "runner": "persist_snomed_goldset_eval", "n": len(rows)}
    sql("INSERT INTO eval_runs (id,name,status,total_combinations,completed_combinations,"
        "config,tenant_id,variable_under_test) VALUES ("
        + ",".join([qstr(run_id), qstr(f"{suite} — {model_id}"), qstr("RUNNING"),
                    str(len(rows)), "0", qstr(json.dumps(cfg)), qstr(TENANT),
                    qstr(variable)]) + ")")
    for r in rows:
        sc = 1.0 if r["ok"] else 0.0
        tags = json.dumps({"group": r["group"], "input": r["input"],
                           "graded": r["group"] in graded_groups}, ensure_ascii=False)
        sql("INSERT INTO eval_scores (run_id,agent_name,model_id,question,expected_answer,"
            "actual_answer,accuracy_score,latency_ms,benchmark_item_id,tags,judge_model,"
            "tenant_id) VALUES ("
            + ",".join([qstr(run_id), qstr(source), qstr(model_id), qstr(r["input"][:500]),
                        qstr(str(r["expected"])[:200]), qstr(str(r["got"])[:500]), str(sc),
                        "0", qstr(r["item_id"]), qstr(tags), qstr("threshold"),
                        qstr(TENANT)]) + ")")
    graded = [r for r in rows if r["group"] in graded_groups]
    avg = sum(1 for r in graded if r["ok"]) / len(graded) if graded else 0.0
    sql("INSERT INTO eval_summary (run_id,agent_name,model_id,total_questions,avg_accuracy,"
        "avg_latency_ms,overall_score,unsafe_count,tenant_id) VALUES ("
        + ",".join([qstr(run_id), qstr(source), qstr(model_id), str(len(rows)),
                    str(round(avg, 4)), "0", str(round(avg, 4)), "0", qstr(TENANT)]) + ")")
    sql(f"UPDATE eval_runs SET status='COMPLETED', completed_combinations={len(rows)}, "
        f"finished_at=NOW() WHERE id={qstr(run_id)}")
    n_pass = sum(1 for r in graded if r["ok"])
    return run_id, n_pass, len(graded), avg


# ─── dataset 1: dose-form precision (validity by tier) ─────────────────────────
def gather_doseform_precision() -> list[dict]:
    rows = []
    for method in ("curated", "exact", "normalized", "token_subset"):
        tot = count(f"SELECT COUNT(*) FROM snomed_tmt_dose_link WHERE match_method='{method}'")
        valid = count(
            "SELECT COUNT(*) FROM snomed_tmt_dose_link l "
            "JOIN (SELECT DISTINCT concept_id FROM snomed_descriptions "
            "      WHERE term_type='fsn' AND term LIKE '%(dose form)%') d "
            f"  ON d.concept_id=l.snomed_concept_id WHERE l.match_method='{method}'")
        edqm = count(
            "SELECT COUNT(*) FROM snomed_tmt_dose_link l "
            "JOIN (SELECT DISTINCT snomed_concept_id FROM snomed_edqm_dose_map) e "
            f"  ON e.snomed_concept_id=l.snomed_concept_id WHERE l.match_method='{method}'")
        rate = (valid / tot) if tot else 0.0
        # token_subset is gated (needs_review=1) → never auto-coded → info only.
        graded = method in ("curated", "exact", "normalized")
        floor = FLOORS.get(f"doseform_valid_{method}")
        rows.append({
            "item_id": f"doseform_valid_{method}", "group": "validity" if graded else "risk_tier",
            "input": f"{method}: dose-link resolves to a real SNOMED dose-form concept",
            "expected": f">={floor}" if graded else "info(gated needs_review=1)",
            "got": f"{valid}/{tot}={rate:.4f}",
            "ok": (rate >= floor) if graded else True,
        })
        rows.append({
            "item_id": f"doseform_edqm_{method}", "group": "enrichment",
            "input": f"{method}: dose-link also carries an EDQM code",
            "expected": "info", "got": f"{edqm}/{tot}", "ok": True,
        })
    return rows


# ─── fixture loading (shared by datasets 2 & 3) ────────────────────────────────
def _walk_codes(obj, want_system_suffix=None, want_system_exact=None, out=None):
    if out is None:
        out = []
    if isinstance(obj, dict):
        sysv, code = obj.get("system"), obj.get("code")
        if sysv and code:
            if want_system_suffix and sysv.endswith(want_system_suffix):
                out.append(code)
            if want_system_exact and sysv == want_system_exact:
                out.append(code)
        for v in obj.values():
            _walk_codes(v, want_system_suffix, want_system_exact, out)
    elif isinstance(obj, list):
        for v in obj:
            _walk_codes(v, want_system_suffix, want_system_exact, out)
    return out


def load_fixtures():
    """Return per-fixture {id, icd10tm:set, snomed:set, med_count, med_tmt_count}."""
    fixtures = []
    bundles = sorted(glob.glob(os.path.join(FIXTURE_DIR, "fhir_r5", "bundle_*.json")))
    for bpath in bundles:
        fid = os.path.basename(bpath).replace("bundle_", "").replace(".json", "")
        bundle = json.load(open(bpath))
        icd10tm = set(_walk_codes(bundle, want_system_suffix="icd-10-tm"))
        snomed = set(_walk_codes(bundle, want_system_exact=SYS_SNOMED))
        epath = os.path.join(FIXTURE_DIR, "extractions", f"extraction_{fid}.json")
        med_count = med_tmt = 0
        if os.path.exists(epath):
            ex = json.load(open(epath))
            for e in ex.get("entities", []):
                if e.get("type") == "MEDICATION":
                    med_count += 1
                    if "tmt" in json.dumps(e).lower():
                        med_tmt += 1
        fixtures.append({"id": fid, "icd10tm": icd10tm, "snomed": snomed,
                         "dx_terms": _condition_terms(bundle),
                         "med_count": med_count, "med_tmt": med_tmt})
    return fixtures


def _in_clause(codes):
    return ",".join(qstr(c) for c in sorted(codes)) if codes else "''"


def icd_candidates(code: str) -> set:
    """Normalize a fixture ICD-10 code to the forms actually stored in the KB.

    icd10_codes and snomed_icd10_map.icd10_tm store codes WITHOUT the dot and at
    varying specificity, so a literal "N39.0" never matches the stored "N390".
    Mirror the snomed_icd10_map bridge: dot-stripped exact → 4-char → 3-char
    category rollup. A code counts as in-scope if ANY candidate exists.
    """
    s = code.replace(".", "").strip().upper()
    if len(s) < 3:
        return set()
    return {c for c in {s, s[:4], s[:3]} if len(c) >= 3}


def _fetch_codes(query: str) -> set:
    """Run a single-column query and return the values as a set of strings."""
    out = sql(query)
    return {ln.strip() for ln in out.splitlines() if ln.strip()}


def _covered(codes, present: set) -> int:
    """How many raw codes have at least one rollup candidate in `present`."""
    return sum(1 for c in codes if icd_candidates(c) & present)


def ips_term_concept(term: str):
    """Mirror Iris's ips_lookup: POST /knowledge/snomed/search {refset:ips,
    refset_only} and return (concept_id, term) of the top IPS-member hit, else
    None. This is the path the /claims/:id/ips-coverage endpoint actually uses."""
    term = (term or "").strip()
    if not term:
        return None
    body = json.dumps({"text": term, "refset": "ips", "refset_only": True, "limit": 1}).encode()
    req = urllib.request.Request(
        MIMIR_BASE.rstrip("/") + "/api/v1/knowledge/snomed/search",
        data=body, headers={"Content-Type": "application/json"})
    try:
        r = json.loads(urllib.request.urlopen(req, timeout=8).read())
    except Exception:
        return None
    cs = r.get("concepts") or []
    if cs and cs[0].get("in_refset"):
        return (cs[0].get("concept_id"), cs[0].get("term", term))
    return None


def _condition_terms(bundle) -> set:
    """Diagnosis labels from a bundle's Condition.code (text, else first coding
    display) — the raw clinical terms an Iris claim would IPS-check."""
    terms = set()
    for e in bundle.get("entry", []):
        r = e.get("resource", {})
        if r.get("resourceType") == "Condition":
            c = r.get("code", {})
            t = c.get("text") or next((cd.get("display") for cd in c.get("coding", []) if cd.get("display")), None)
            if t and t.strip():
                terms.add(t.strip())
    return terms


# ─── dataset 2: coding validity (fixtures → in-scope) ──────────────────────────
def gather_coding_validity(fixtures) -> list[dict]:
    rows = []
    all_icd, all_snomed = set(), set()
    total_med = total_med_tmt = 0
    for fx in fixtures:
        all_icd |= fx["icd10tm"]
        all_snomed |= fx["snomed"]
        total_med += fx["med_count"]
        total_med_tmt += fx["med_tmt"]
    # Codes are dot-stripped in the KB; match each fixture code by its rollup
    # candidates (dot-stripped → 4-char → 3-char), not the literal dotted form.
    all_cands = set()
    for c in all_icd:
        all_cands |= icd_candidates(c)
    present = _fetch_codes(
        f"SELECT code FROM icd10_codes WHERE tenant_id IS NULL AND code IN ({_in_clause(all_cands)})"
    ) if all_cands else set()
    for fx in fixtures:
        in_scope = _covered(fx["icd10tm"], present)
        rows.append({
            "item_id": f"fixture_{fx['id']}_icd10tm", "group": "per_fixture",
            "input": f"fixture {fx['id']}: ICD-10-TM codes in-scope (icd10_codes, dot-stripped+rollup)",
            "expected": "info", "got": f"{in_scope}/{len(fx['icd10tm'])}", "ok": True,
        })
    icd_in = _covered(all_icd, present)
    rate = (icd_in / len(all_icd)) if all_icd else 0.0
    rows.append({
        "item_id": "icd10tm_in_scope", "group": "validity",
        "input": "all-fixture distinct ICD-10-TM codes resolve in icd10_codes",
        "expected": f">={FLOORS['icd10tm_in_scope']}",
        "got": f"{icd_in}/{len(all_icd)}={rate:.4f}",
        "ok": rate >= FLOORS["icd10tm_in_scope"],
    })
    # SNOMED-coded + TMT-dose-coded entities: counted, not silently skipped.
    snomed_in = count(
        f"SELECT COUNT(DISTINCT concept_id) FROM snomed_descriptions WHERE concept_id IN ({_in_clause(all_snomed)})"
    ) if all_snomed else 0
    rows.append({
        "item_id": "snomed_codes", "group": "validity_info",
        "input": "fixture SNOMED codes valid (in snomed_descriptions)",
        "expected": "info", "got": f"{snomed_in}/{len(all_snomed)}", "ok": True,
    })
    rows.append({
        "item_id": "doseform_coded_meds", "group": "validity_info",
        "input": "fixture MEDICATION entities carrying a TMT id (dose-form codable)",
        "expected": "info", "got": f"{total_med_tmt}/{total_med}", "ok": True,
    })
    return rows


# ─── dataset 3: IPS coverage (fixture concepts → IPS member) ───────────────────
def gather_ips_coverage(fixtures) -> list[dict]:
    rows = []
    all_icd = set()
    for fx in fixtures:
        all_icd |= fx["icd10tm"]
    # icd10_tm is also dot-stripped — match by rollup candidates, then check which
    # candidates reach an IPS- / GPFP-member SNOMED concept.
    # NB: this measures reachability through the ICD->SNOMED *map* (snomed_icd10_map),
    # a stricter, map-quality signal. It is NOT how Iris checks IPS — the
    # /claims/:id/ips-coverage endpoint resolves by SNOMED term search, which finds
    # IPS concepts the map misses (e.g. septic shock 76571007, AKI 35455006). Misses
    # here are map gaps (ICD-CM-vs-WHO mismatch, 3-char->child expansion), not absence
    # of an IPS concept for the diagnosis.
    all_cands = set()
    for c in all_icd:
        all_cands |= icd_candidates(c)
    ips_tm = _fetch_codes(
        "SELECT DISTINCT m.icd10_tm FROM snomed_icd10_map m "
        "JOIN (SELECT DISTINCT concept_id FROM snomed_refset_members WHERE refset_key='ips') r "
        f"  ON r.concept_id=m.concept_id WHERE m.icd10_tm IN ({_in_clause(all_cands)})"
    ) if all_cands else set()
    n_covered = _covered(all_icd, ips_tm)
    rate = (n_covered / len(all_icd)) if all_icd else 0.0
    rows.append({
        "item_id": "ips_coverage", "group": "coverage",
        "input": "fixture ICD-10-TM concepts reaching an IPS-member SNOMED concept VIA snomed_icd10_map (map-path reachability; Iris itself uses term search, which is higher-recall)",
        "expected": f">={FLOORS['ips_coverage']}",
        "got": f"{n_covered}/{len(all_icd)}={rate:.4f}",
        "ok": rate >= FLOORS["ips_coverage"],
    })
    # GPFP comparison (info): how many of the same concepts are GPFP members.
    gpfp_tm = _fetch_codes(
        "SELECT DISTINCT m.icd10_tm FROM snomed_icd10_map m "
        "JOIN (SELECT DISTINCT concept_id FROM snomed_refset_members WHERE refset_key='gpfp') r "
        f"  ON r.concept_id=m.concept_id WHERE m.icd10_tm IN ({_in_clause(all_cands)})"
    ) if all_cands else set()
    n_gpfp = _covered(all_icd, gpfp_tm)
    rows.append({
        "item_id": "gpfp_coverage", "group": "coverage_info",
        "input": "fixture ICD-10-TM concepts reaching a GPFP-member SNOMED concept",
        "expected": "info", "got": f"{n_gpfp}/{len(all_icd)}", "ok": True,
    })

    # ── TERM-PATH (the path Iris actually uses) ──────────────────────────────
    # Iris's /claims/:id/ips-coverage resolves each diagnosis by SNOMED *term*
    # search against the IPS refset (ips_lookup), not via the ICD→SNOMED map.
    # Measure that path on the raw fixture diagnosis labels (the realistic input)
    # so the benchmark reflects Iris's real IPS interoperability.
    all_terms = set()
    for fx in fixtures:
        all_terms |= fx["dx_terms"]
    term_hits = 0
    miss_terms = []
    for t in sorted(all_terms):
        if ips_term_concept(t):
            term_hits += 1
        else:
            miss_terms.append(t)
    t_rate = (term_hits / len(all_terms)) if all_terms else 0.0
    rows.append({
        "item_id": "term_ips_coverage", "group": "coverage",
        "input": "fixture diagnosis terms resolving to an IPS-member SNOMED concept via term search (the path Iris uses: ips_lookup, /search?refset=ips)",
        "expected": f">={FLOORS['term_ips_coverage']}",
        "got": f"{term_hits}/{len(all_terms)}={t_rate:.4f}",
        "ok": t_rate >= FLOORS["term_ips_coverage"],
    })
    # Surface the misses so the gap is inspectable (these are the diagnoses whose
    # raw label term-search can't map to IPS — typically abbreviations like HT/DLP
    # and synonyms like "bedsore" vs SNOMED "pressure ulcer"; fixable upstream by
    # abbreviation expansion before the IPS lookup, not a KB gap).
    rows.append({
        "item_id": "term_ips_misses", "group": "coverage_info",
        "input": "diagnosis terms with no IPS term-search hit (abbrev/synonym gaps)",
        "expected": "info", "got": "; ".join(miss_terms) or "(none)", "ok": True,
    })
    return rows


def _report(title, rows, graded_groups, run_id, n_pass, n_graded, avg):
    print(f"\n── {title}  (run_id={run_id})")
    for r in rows:
        graded = r["group"] in graded_groups
        mark = ("ok" if r["ok"] else "FAIL") if graded else "info"
        print(f"   {r['item_id']:<28} {str(r['got']):>16}  {mark}")
    print(f"   graded: {n_pass}/{n_graded} pass, accuracy={avg:.2%}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="sprint59")
    args = ap.parse_args()
    model_id = f"asgard-mimir-kb:{args.version}"
    sql("INSERT INTO ai_models (model_id,provider,model_type,is_active,metadata) VALUES ("
        + ",".join([qstr(model_id), qstr("mimir-kb"), qstr("service"), "1",
                    qstr(json.dumps({"kind": "kb-goldset"}))])
        + ") ON DUPLICATE KEY UPDATE updated_at=NOW()")

    fixtures = load_fixtures()
    print(f"loaded {len(fixtures)} fixtures")

    overall_ok = True

    d1 = gather_doseform_precision()
    r = persist("snomed-doseform-precision", "snomed-doseform",
                "X2: TMT->SNOMED dose-link validity by match tier (validity, not subtype accuracy; "
                "token_subset gated needs_review=1)", "dose-link-ingest", d1, model_id, {"validity"})
    _report("DS1 dose-form precision", d1, {"validity"}, *r)
    overall_ok &= (r[1] == r[2])

    d2 = gather_coding_validity(fixtures)
    r = persist("snomed-coding-validity", "snomed-coding",
                "X2: 7 claims-pipeline fixtures — ICD-10-TM codes in-scope (vocab validity, "
                "not human accuracy; low rate = ICD-10-CM/WHO codes outside Mimir ICD-10-TM scope)",
                "extraction-coding", d2, model_id, {"validity"})
    _report("DS2 coding validity", d2, {"validity"}, *r)
    overall_ok &= (r[1] == r[2])

    d3 = gather_ips_coverage(fixtures)
    r = persist("snomed-ips-coverage", "snomed-ips",
                "X2: fixture ICD-10-TM concepts reaching an IPS-member SNOMED concept via "
                "snomed_icd10_map (map-path reachability — a map-quality signal; NOT term-path, "
                "which is what Iris uses and is higher-recall)", "ips-coverage", d3, model_id, {"coverage"})
    _report("DS3 IPS coverage", d3, {"coverage"}, *r)
    overall_ok &= (r[1] == r[2])

    print(f"\n{'ALL GOLD SETS PASS' if overall_ok else 'REGRESSION: a graded case is below its floor'}")
    return 0 if overall_ok else 1


if __name__ == "__main__":
    sys.exit(main())
