#!/usr/bin/env python3
"""Group the FHIR R5 document bundles into one Claim Case + emit a requirement checklist.

A "Case" = one Patient + one Encounter (episode of treatment). This script:
  1. Builds the case-level FHIR scaffold that BINDS every document to the same
     patient & encounter:  Encounter, Coverage, DocumentReference (x7), List.
  2. Scans the gathered FHIR R5 bundles to see which claim-required resources
     are actually present, and renders a Thai-insurance checklist (NHSO / สปสช /
     CSMBS) marking each requirement ✅ present / ❌ missing.

Input : data/abb/fhir_r5/bundle_{1..7}.json
Output: data/abb/cases/<case_id>/
          encounter.json, coverage.json, list.json, docref_{1..7}.json   (FHIR scaffold)
          case.json        (machine-readable manifest + checklist status)
          CHECKLIST.md      (human-readable claim document checklist)
"""
from __future__ import annotations

import json
import glob
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FHIR_DIR = ROOT / "data" / "abb" / "fhir_r5"
CASES_DIR = ROOT / "data" / "abb" / "cases"

BASE = "https://asgard.local/fhir"
PROFILE_BASE = "https://fhir.moph.go.th/StructureDefinition"
SYS_LOINC = "http://loinc.org"

# --------------------------------------------------------------------------- #
# Case definition (would come from HIS / admission desk in production)
# --------------------------------------------------------------------------- #
CASE = {
    "case_id": "case-001",
    "patient_id": "patient-001",
    "encounter_id": "encounter-001",
    "coverage_id": "coverage-001",
    "class": "IMP",                 # IMP = inpatient (IPD)
    "period_start": "2026-04-30",
    "period_end": "2026-05-15",
    # สิทธิการรักษา — drives which checklist columns are mandatory
    "scheme": "NHSO",               # NHSO | SSO | CSMBS
    "hn": "HN-0000001",
    "an": "AN-2026-0001",
}

SCHEME_LABEL = {
    "NHSO": "บัตรทอง (สปสช./NHSO)",
    "SSO": "ประกันสังคม (สปส./SSO)",
    "CSMBS": "ข้าราชการ (กรมบัญชีกลาง/CSMBS)",
}

# --------------------------------------------------------------------------- #
# Claim document requirement checklist — loaded from the SHARED spec so the per-case
# instance (this script) and the master templates (build_requirement_checklists.py)
# never drift. Spec: data/abb/registry/checklist_requirements.json
#   priority: P0 = บังคับเบิกไม่ได้ถ้าขาด, P1 = ควรมี/เพิ่มมูลค่าเคลม, P2 = สนับสนุน
#   detect  : how to mark present from the gathered FHIR resources
# --------------------------------------------------------------------------- #
def _load_requirements():
    spec = json.loads((ROOT / "data" / "abb" / "registry" / "checklist_requirements.json").read_text())
    rows = []
    for r in spec["workflows"]["claim"]["requirements"]:
        d = r["detect"]
        rows.append((r["id"], r["label"], r["fhir"], r["priority"],
                     r.get("required_in", []), (d["kind"], d["key"])))
    return rows

REQUIREMENTS = _load_requirements()

# --------------------------------------------------------------------------- #
# Load gathered bundles & index what's present
# --------------------------------------------------------------------------- #
def scan_bundles():
    bundles = []
    present_types = set()
    composition_codes = set()
    # DocumentReference type codes (the 7 doc pointers carry their Composition LOINC).
    # Specific forms (med-cert 11502-2, consent 59284-0) are NOT among these → stay ❌.
    for fn in sorted(glob.glob(str(FHIR_DIR / "bundle_*.json"))):
        b = json.loads(Path(fn).read_text())
        comp = b["entry"][0]["resource"]  # document bundle → Composition first
        bundles.append({
            "file": Path(fn).name,
            "bundle_id": b["id"],
            "title": comp.get("title", ""),
            "type_code": comp["type"]["coding"][0]["code"],
            "type_display": comp["type"]["coding"][0].get("display", ""),
            "timestamp": b.get("timestamp"),
        })
        for e in b["entry"]:
            present_types.add(e["resource"]["resourceType"])
            if e["resource"]["resourceType"] == "Composition":
                composition_codes.add(e["resource"]["type"]["coding"][0]["code"])
    # DocumentReference type codes available at case level = the bundle Composition types.
    docref_codes = {b["type_code"] for b in bundles}
    return bundles, present_types, composition_codes, docref_codes


def evaluate(present_types, composition_codes, docref_codes):
    rows = []
    for rid, label, fhir, prio, schemes, detect in REQUIREMENTS:
        kind, key = detect
        if kind == "resource":
            present = key in present_types
        elif kind == "composition":
            present = key in composition_codes
        elif kind == "docref":
            present = key in docref_codes
        else:
            present = False
        rows.append({
            "id": rid, "label": label, "fhir": fhir, "priority": prio,
            "required_schemes": schemes, "present": present,
            "required_here": CASE["scheme"] in schemes,
        })
    return rows


# --------------------------------------------------------------------------- #
# Case-level FHIR scaffold (binds docs → one patient & encounter)
# --------------------------------------------------------------------------- #
def build_encounter():
    return {
        "resourceType": "Encounter",
        "id": CASE["encounter_id"],
        "meta": {"profile": [f"{PROFILE_BASE}/moph-pc-encounter"]},
        "identifier": [{"system": f"{BASE}/an", "value": CASE["an"]}],
        "status": "completed",
        "class": [{"coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
            "code": CASE["class"],
            "display": "inpatient encounter" if CASE["class"] == "IMP" else "ambulatory",
        }]}],
        "subject": {"reference": f"Patient/{CASE['patient_id']}"},
        # R5: actualPeriod (renamed from R4 'period')
        "actualPeriod": {"start": CASE["period_start"], "end": CASE["period_end"]},
    }


def build_coverage():
    return {
        "resourceType": "Coverage",
        "id": CASE["coverage_id"],
        "meta": {"profile": [f"{PROFILE_BASE}/moph-pc-coverage"]},
        "status": "active",
        "kind": "insurance",
        "type": {"coding": [{
            "system": "https://terminology.fhir.moph.go.th/CodeSystem/cs-coverage-scheme",
            "code": CASE["scheme"],
            "display": SCHEME_LABEL[CASE["scheme"]],
        }]},
        "beneficiary": {"reference": f"Patient/{CASE['patient_id']}"},
        "payor": [{"display": SCHEME_LABEL[CASE["scheme"]]}],
    }


def build_docref(idx, bundle):
    """One DocumentReference per source document, pinned to this case."""
    return {
        "resourceType": "DocumentReference",
        "id": f"docref-{idx}",
        "meta": {"profile": [f"{PROFILE_BASE}/moph-pc-documentreference"]},
        "status": "current",
        "type": {"coding": [{"system": SYS_LOINC, "code": bundle["type_code"],
                             "display": bundle["type_display"]}],
                 "text": bundle["title"]},
        "subject": {"reference": f"Patient/{CASE['patient_id']}"},
        "date": bundle["timestamp"],
        "context": {"encounter": [{"reference": f"Encounter/{CASE['encounter_id']}"}]},
        "content": [{"attachment": {
            "contentType": "application/fhir+json",
            "url": f"../../fhir_r5/{bundle['file']}",
            "title": bundle["title"],
        }}],
    }


def build_list(bundles):
    """FHIR List = the case 'folder' enumerating every document in order."""
    return {
        "resourceType": "List",
        "id": f"{CASE['case_id']}-documents",
        "status": "current",
        "mode": "working",
        "title": f"Claim documents — {CASE['case_id']} ({CASE['an']})",
        "code": {"coding": [{"system": SYS_LOINC, "code": "52521-2",
                             "display": "Document collection"}]},
        "subject": {"reference": f"Patient/{CASE['patient_id']}"},
        "encounter": {"reference": f"Encounter/{CASE['encounter_id']}"},
        "date": CASE["period_end"],
        "entry": [{"item": {"reference": f"DocumentReference/docref-{i}"}}
                  for i in range(1, len(bundles) + 1)],
    }


# --------------------------------------------------------------------------- #
# Render checklist markdown
# --------------------------------------------------------------------------- #
def render_checklist(bundles, rows):
    here = CASE["scheme"]
    p0 = [r for r in rows if r["priority"] == "P0"]
    blocking = [r for r in p0 if r["required_here"] and not r["present"]]
    ready = not blocking

    def tick(b):
        return "✅" if b else "❌"

    def scheme_cell(r, s):
        if r["present"]:
            return "✅"
        return "บังคับ" if s in r["required_schemes"] else "—"

    L = []
    L.append(f"# Claim Document Checklist — {CASE['case_id']}")
    L.append("")
    L.append(f"- **ผู้ป่วย:** `{CASE['patient_id']}`  HN `{CASE['hn']}`  AN `{CASE['an']}`")
    L.append(f"- **การรักษา:** {('IPD' if CASE['class']=='IMP' else 'OPD')}  "
             f"{CASE['period_start']} → {CASE['period_end']}")
    L.append(f"- **สิทธิ:** {SCHEME_LABEL[here]}")
    L.append(f"- **เอกสารในเคส:** {len(bundles)} ฉบับ")
    L.append("")
    status = "🟢 พร้อมยื่นเคลม (เอกสาร P0 ครบ)" if ready else \
             f"🔴 ยังเบิกไม่ได้ — ขาดเอกสารบังคับ {len(blocking)} รายการ"
    L.append(f"## สถานะ: {status}")
    if blocking:
        L.append("")
        L.append("**ขาด (P0 บังคับ):** " + ", ".join(f"`{r['fhir']}`" for r in blocking))
    L.append("")

    # document inventory
    L.append("## เอกสารที่รวมไว้ในเคสนี้")
    L.append("")
    L.append("| # | เอกสาร | LOINC | FHIR bundle |")
    L.append("|---|--------|-------|-------------|")
    for i, b in enumerate(bundles, 1):
        L.append(f"| {i} | {b['title']} | {b['type_code']} | `{b['file']}` |")
    L.append("")

    # checklist by priority
    for prio, header in [("P0", "P0 — บังคับ (ขาดแล้วเบิกไม่ได้)"),
                         ("P1", "P1 — ควรมี (ความถูกต้อง/มูลค่าเคลม)"),
                         ("P2", "P2 — สนับสนุน (ความสมบูรณ์)")]:
        L.append(f"## {header}")
        L.append("")
        L.append("| สถานะ | เอกสาร / ข้อมูล | FHIR R5 | NHSO | สปสช | CSMBS |")
        L.append("|:---:|------|------|:---:|:---:|:---:|")
        for r in [x for x in rows if x["priority"] == prio]:
            L.append(f"| {tick(r['present'])} | {r['label']} | `{r['fhir']}` | "
                     f"{scheme_cell(r,'NHSO')} | {scheme_cell(r,'SSO')} | {scheme_cell(r,'CSMBS')} |")
        L.append("")

    L.append("> ✅ = มีใน FHIR แล้ว · ❌ = ยังไม่มี · `บังคับ` = สิทธินี้บังคับแต่ยังขาด · `—` = ไม่บังคับสำหรับสิทธินั้น")
    return "\n".join(L) + "\n"


# --------------------------------------------------------------------------- #
def main():
    bundles, present_types, comp_codes, docref_codes = scan_bundles()
    # Case-level scaffold resources this builder always materialises → count as present.
    present_types |= {"Encounter", "Coverage", "DocumentReference", "List"}
    rows = evaluate(present_types, comp_codes, docref_codes)

    out = CASES_DIR / CASE["case_id"]
    out.mkdir(parents=True, exist_ok=True)

    # FHIR scaffold
    (out / "encounter.json").write_text(json.dumps(build_encounter(), ensure_ascii=False, indent=2))
    (out / "coverage.json").write_text(json.dumps(build_coverage(), ensure_ascii=False, indent=2))
    for i, b in enumerate(bundles, 1):
        (out / f"docref_{i}.json").write_text(json.dumps(build_docref(i, b), ensure_ascii=False, indent=2))
    (out / "list.json").write_text(json.dumps(build_list(bundles), ensure_ascii=False, indent=2))

    # Manifest + checklist status (machine-readable)
    manifest = {
        "case": CASE,
        "scheme_label": SCHEME_LABEL[CASE["scheme"]],
        "documents": bundles,
        "fhir_resources_present": sorted(present_types),
        "checklist": rows,
        "blocking_p0": [r["fhir"] for r in rows
                        if r["priority"] == "P0" and r["required_here"] and not r["present"]],
        "ready_to_claim": not any(r["priority"] == "P0" and r["required_here"] and not r["present"]
                                  for r in rows),
    }
    (out / "case.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2))
    (out / "CHECKLIST.md").write_text(render_checklist(bundles, rows))

    # console summary
    print(f"Case {CASE['case_id']} → {out}")
    print(f"  documents : {len(bundles)}  |  FHIR present: {', '.join(sorted(present_types))}")
    have = sum(r["present"] for r in rows)
    print(f"  checklist : {have}/{len(rows)} present")
    if manifest["blocking_p0"]:
        print(f"  🔴 NOT claimable — missing P0: {', '.join(manifest['blocking_p0'])}")
    else:
        print("  🟢 claimable (P0 complete)")


if __name__ == "__main__":
    main()
