#!/usr/bin/env python3
"""Render the two master document checklists (Underwriting + Claim) from the shared spec.

Spec   : data/abb/registry/checklist_requirements.json   (single source of truth)
Output : data/abb/registry/UNDERWRITING_DOCUMENT_CHECKLIST.md
         data/abb/registry/CLAIM_DOCUMENT_CHECKLIST.md

These are blank requirement TEMPLATES (☐ to tick). The per-case, data-driven CLAIM
instance (✅/❌ against real FHIR bundles) is produced separately by build_claim_case.py,
which reads the *same* spec file so the two never drift.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "data" / "abb" / "registry" / "checklist_requirements.json"
OUT_DIR = SPEC.parent

# mimir-fhir 21-resource scope (20 MOPH-PC1 + Composition via ADR-015).
MIMIR_FHIR_SCOPE = {
    "Patient", "Practitioner", "Organization", "Location", "Encounter", "Observation",
    "Condition", "Procedure", "AllergyIntolerance", "MedicationRequest", "MedicationStatement",
    "DiagnosticReport", "ImagingStudy", "Specimen", "DocumentReference", "Coverage", "Claim",
    "ClaimResponse", "Immunization", "Device", "Composition", "MedicationAdministration",
}

PRIO_HEADER = {
    "P0": "P0 — บังคับ (ขาดแล้วดำเนินการต่อไม่ได้)",
    "P1": "P1 — ตามเกณฑ์/ควรมี",
    "P2": "P2 — กรณีพิเศษ/สนับสนุน",
}


def base_resource(fhir: str) -> str:
    """'Composition(discharge)' / 'DocumentReference(11502-2)' -> base resource name."""
    return fhir.split("(", 1)[0]


def in_scope(fhir: str) -> bool:
    return base_resource(fhir) in MIMIR_FHIR_SCOPE


def scope_cell(fhir: str) -> str:
    return "✓" if in_scope(fhir) else "✗ ต้องขยาย scope"


def render_underwriting(wf: dict) -> str:
    L = [
        f"# {wf['title_th']}",
        "",
        f"_subject: {wf['subject']} · master template — ☐ ติ๊กเมื่อได้รับเอกสาร_",
        "",
        "ใช้คู่กับ [CLAIM_DOCUMENT_CHECKLIST.md](./CLAIM_DOCUMENT_CHECKLIST.md). คอลัมน์ *mimir-fhir scope* บอกว่า resource นั้นอยู่ใน 21-resource scope ปัจจุบันหรือยัง (✗ = ต้องขยายก่อนทำ FHIR-native).",
        "",
    ]
    out_items = [r for r in wf["requirements"] if r.get("io") == "output"]
    for prio in ("P0", "P1", "P2"):
        rows = [r for r in wf["requirements"] if r["priority"] == prio and r.get("io") != "output"]
        if not rows:
            continue
        L.append(f"## {PRIO_HEADER[prio]}")
        L.append("")
        L.append("| ☐ | เอกสาร | FHIR R5 | เงื่อนไขที่ต้องใช้ | mimir-fhir scope | Asgard type |")
        L.append("|:---:|------|------|------|:---:|------|")
        for r in rows:
            dt = f"`{r['doctype']}`" if r.get("doctype") else "—"
            L.append(f"| ☐ | {r['label']} | `{r['fhir']}` | {r['trigger']} | {scope_cell(r['fhir'])} | {dt} |")
        L.append("")
    if out_items:
        L.append("## ผลลัพธ์ (output ของกระบวนการ)")
        L.append("")
        L.append("| ☐ | ผลลัพธ์ | FHIR R5 | mimir-fhir scope |")
        L.append("|:---:|------|------|:---:|")
        for r in out_items:
            L.append(f"| ☐ | {r['label']} | `{r['fhir']}` | {scope_cell(r['fhir'])} |")
        L.append("")
    # scope gap callout
    gaps = sorted({base_resource(r["fhir"]) for r in wf["requirements"] if not in_scope(r["fhir"])})
    L.append("> **ช่องว่าง scope:** resource ที่ underwriting ต้องใช้แต่ยังไม่อยู่ใน mimir-fhir 21-resource scope: "
             + ", ".join(f"`{g}`" for g in gaps)
             + " — ผูกกับ open point #2 ([SYSTEM_CONTEXT](../../../../Asgard/docs/SYSTEM_CONTEXT.md)).")
    return "\n".join(L) + "\n"


def render_claim(wf: dict) -> str:
    schemes = wf["axis_values"]
    label_th = {"NHSO": "NHSO", "SSO": "สปสช", "CSMBS": "CSMBS"}
    L = [
        f"# {wf['title_th']}",
        "",
        f"_subject: {wf['subject']} · master template — ☐ ติ๊กเมื่อได้รับเอกสาร_",
        "",
        "ใช้คู่กับ [UNDERWRITING_DOCUMENT_CHECKLIST.md](./UNDERWRITING_DOCUMENT_CHECKLIST.md). "
        "เวอร์ชัน *รายเคส* (✅/❌ จาก FHIR จริง) อยู่ที่ `data/abb/cases/<case>/CHECKLIST.md` (สร้างโดย build_claim_case.py จาก spec ไฟล์เดียวกัน).",
        "",
        "> `บังคับ` = สิทธินั้นบังคับ · `แนะนำ` = ควรมี · `—` = ไม่บังคับสำหรับสิทธินั้น",
        "",
    ]

    def cell(r, s):
        return "บังคับ" if s in r.get("required_in", []) else ("แนะนำ" if r["priority"] != "P0" else "—")

    for prio in ("P0", "P1", "P2"):
        rows = [r for r in wf["requirements"] if r["priority"] == prio]
        if not rows:
            continue
        L.append(f"## {PRIO_HEADER[prio]}")
        L.append("")
        head = "| ☐ | เอกสาร / ข้อมูล | FHIR R5 | " + " | ".join(label_th[s] for s in schemes) + " | Asgard type |"
        sep = "|:---:|------|------|" + "|".join([":---:"] * len(schemes)) + "|------|"
        L.append(head)
        L.append(sep)
        for r in rows:
            dt = f"`{r['doctype']}`" if r.get("doctype") else "—"
            cells = " | ".join(cell(r, s) for s in schemes)
            tag = " _(output)_" if r.get("io") == "output" else ""
            L.append(f"| ☐ | {r['label']}{tag} | `{r['fhir']}` | {cells} | {dt} |")
        L.append("")
    return "\n".join(L) + "\n"


def main():
    spec = json.loads(SPEC.read_text())
    wf = spec["workflows"]

    (OUT_DIR / "UNDERWRITING_DOCUMENT_CHECKLIST.md").write_text(render_underwriting(wf["underwriting"]))
    (OUT_DIR / "CLAIM_DOCUMENT_CHECKLIST.md").write_text(render_claim(wf["claim"]))

    for name, w in wf.items():
        n = len(w["requirements"])
        gaps = sorted({base_resource(r["fhir"]) for r in w["requirements"] if not in_scope(r["fhir"])})
        print(f"{name:<13} {n:>2} requirements · scope gaps: {', '.join(gaps) or 'none'}")
    print(f"\nWrote 2 checklists to {OUT_DIR}")


if __name__ == "__main__":
    main()
