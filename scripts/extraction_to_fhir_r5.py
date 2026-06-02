#!/usr/bin/env python3
"""Map extracted clinical documents -> standard Thai-profiled FHIR R5 document Bundles.

Gathers the 7 medical documents used for this patient's treatment + insurance
submission and maps each into a conformant FHIR R5 (5.0.0) ``Bundle`` of
``type = document``.

Input : data/abb/extractions/extraction_{1..7}.json   (Syn OCR + NER + ICD output)
Output: data/abb/fhir_r5/bundle_{1..7}.json            (one document Bundle / doc)
        data/abb/fhir_r5/INDEX.md                       (gather + mapping summary)

Conformance / corrections over the embedded `fhir_resources` blocks:
- FHIR R5: MedicationRequest.medication is a CodeableReference (R5-only shape).
- Adds the Patient resource (was missing) with TH Core profile + MOPH citizen-id slice.
- Composition now carries required status/title and *resolvable* section references.
- ICD codes re-systemed to ICD-10-TM (MOPH) as primary, ICD-9-CM kept as equivalence.
- Safe ASCII resource ids (source ids contained newlines / '/' / Thai characters).
- Conditions de-duplicated per document; vitals parsed to valueQuantity + UCUM/LOINC.
- TH Core + MoPH-PC meta.profile bindings on every resource.
"""
from __future__ import annotations

import json
import os
import re
import sys
import unicodedata
from pathlib import Path

# B3: enrich MedicationRequest with a SNOMED+EDQM-coded Medication.doseForm resolved
# from the TMT id. Opt-in (needs a live MariaDB) so default offline/CI runs are
# unchanged; enable with FHIR_RESOLVE_DOSEFORM=1.
RESOLVE_DOSEFORM = os.environ.get("FHIR_RESOLVE_DOSEFORM") == "1"

# --------------------------------------------------------------------------- #
# Canonical systems & profiles (Thai-profiled R5)
# --------------------------------------------------------------------------- #
SYS_ICD10TM = "https://terminology.fhir.moph.go.th/CodeSystem/icd-10-tm"
SYS_ICD9CM = "http://hl7.org/fhir/sid/icd-9-cm"
SYS_LOINC = "http://loinc.org"
SYS_UCUM = "http://unitsofmeasure.org"
SYS_TMT = "https://terminology.fhir.moph.go.th/CodeSystem/tmt"
SYS_CITIZEN_ID = "https://fhir.moph.go.th/identifier/citizen-id"  # MOPH slice (from platform spec)
SYS_COND_CLINICAL = "http://terminology.hl7.org/CodeSystem/condition-clinical"
SYS_COND_CATEGORY = "http://terminology.hl7.org/CodeSystem/condition-category"
SYS_OBS_CATEGORY = "http://terminology.hl7.org/CodeSystem/observation-category"

PROFILE = {
    "Patient": "https://fhir.moph.go.th/StructureDefinition/th-core-patient",
    "Condition": "https://fhir.moph.go.th/StructureDefinition/moph-pc-condition",
    "Observation": "https://fhir.moph.go.th/StructureDefinition/moph-pc-observation",
    "MedicationRequest": "https://fhir.moph.go.th/StructureDefinition/moph-pc-medicationrequest",
    "Composition": "https://fhir.moph.go.th/StructureDefinition/moph-pc-composition",
}

BASE = "https://asgard.local/fhir"  # base for fullUrl / relative reference resolution

# Composition.type LOINC by source document type
DOC_TYPE_LOINC = {
    "MEDICAL_HISTORY": ("34117-2", "History and physical note"),
    "PHYSICAL_EXAMINATION": ("29545-1", "Physical findings"),
    "PROGRESS_NOTE": ("11506-3", "Progress note"),
    "MEDICATION_ORDER": ("57833-6", "Prescription for medication"),
}
DOC_TYPE_TITLE_TH = {
    "MEDICAL_HISTORY": "ประวัติการเจ็บป่วย (History)",
    "PHYSICAL_EXAMINATION": "การตรวจร่างกาย (Physical Examination)",
    "PROGRESS_NOTE": "บันทึกความก้าวหน้า (Progress Note)",
    "MEDICATION_ORDER": "คำสั่งการใช้ยา (Medication Order)",
}

# Vital sign / lab LOINC + UCUM mapping. Pattern -> (loinc, display, unit, ucum)
VITAL_MAP = {
    "PR": ("8867-4", "Heart rate", "/min", "/min"),
    "HR": ("8867-4", "Heart rate", "/min", "/min"),
    "BT": ("8310-5", "Body temperature", "Cel", "Cel"),
    "T": ("8310-5", "Body temperature", "Cel", "Cel"),
    "SpO2": ("59408-5", "Oxygen saturation in Arterial blood by Pulse oximetry", "%", "%"),
    "RR": ("9279-1", "Respiratory rate", "/min", "/min"),
    "NT proBNP": ("33762-6", "NT-proBNP", "pg/mL", "pg/mL"),
}

# Best-effort cleaned dosing (OCR sliding-window text in source was garbled).
# Raw OCR is preserved in dosageInstruction[].text-derived note for traceability.
# tmt_id: representative TMT GP with a trusted (needs_review=0) dose-form link, so
# Medication.code carries a real TMT code and doseForm resolves (B3). Drugs whose
# only TMT forms are token_subset/needs_review (e.g. colistin powder-for-injection)
# carry no tmt_id and fall back to the concept shape with no doseForm — by design.
MED_CATALOG = {
    "levothyroxine": {"display": "Levothyroxine", "tmt_id": "1154071", "dose": "Levothyroxine 50 mcg 0.5 tab PO ac (ก่อนอาหาร 1 ชม.), then 1 tab PO ac"},
    "albumin": {"display": "Human Albumin", "tmt_id": "750218", "dose": "20% Albumin 100 mL + Furosemide 40 mg IV drip in 1 hr; later 5% Albumin 250 mL IV drip in 4 hr"},
    "furosemide": {"display": "Furosemide", "tmt_id": "989509", "dose": "Furosemide 40 mg IV (with albumin drip)"},
    "colistin": {"display": "Colistin (Colistimethate sodium)", "dose": "Colistin 300 mg + NSS 100 mL IV drip 1 hr (loading), then 100 mg + NSS 100 mL IV drip 1 hr q12h"},
    "stiafloxacin": {"display": "Sitafloxacin", "dose": "Sitafloxacin 50 mg 1 tab PO OD"},
    "cpm": {"display": "Chlorpheniramine (CPM)", "dose": "Chlorpheniramine (CPM) — see order"},
}

ROOT = Path(__file__).resolve().parent.parent
IN_DIR = ROOT / "data" / "abb" / "extractions"
OUT_DIR = ROOT / "data" / "abb" / "fhir_r5"

# --------------------------------------------------------------------------- #
# Helpers
# --------------------------------------------------------------------------- #
def slug(text: str, maxlen: int = 48) -> str:
    """Safe ASCII kebab id (FHIR id = [A-Za-z0-9-.]{1,64})."""
    text = unicodedata.normalize("NFKD", text or "").encode("ascii", "ignore").decode()
    text = re.sub(r"[^A-Za-z0-9]+", "-", text).strip("-").lower()
    return (text[:maxlen].strip("-")) or "x"


def ref(resource_type: str, rid: str) -> dict:
    return {"reference": f"{resource_type}/{rid}"}


def icd_codings(icd_codes: list[dict]) -> list[dict]:
    """Re-system ICD-10-CM -> ICD-10-TM (primary), keep ICD-9-CM as equivalence."""
    out = []
    for c in icd_codes or []:
        sys_in = (c.get("system") or "").lower()
        system = SYS_ICD9CM if "icd-9" in sys_in else SYS_ICD10TM
        out.append({"system": system, "code": c["code"], "display": c.get("display", "")})
    return out


def parse_value(raw: str):
    """Pull a numeric value off vital/lab raw text -> (key, number) or None."""
    m = re.search(r"([A-Za-z/0-9 ]+?)\s+(\d+(?:\.\d+)?)", raw)
    if not m:
        return None
    return m.group(1).strip(), float(m.group(2))


# --------------------------------------------------------------------------- #
# Resource builders
# --------------------------------------------------------------------------- #
def build_patient() -> dict:
    # De-identified synthetic subject for this claim set.
    return {
        "resourceType": "Patient",
        "id": "patient-001",
        "meta": {"profile": [PROFILE["Patient"]]},
        "identifier": [{
            "use": "official",
            "system": SYS_CITIZEN_ID,
            "value": "0000000000000",  # placeholder — de-identified
        }],
        "active": True,
        "name": [{"use": "official", "text": "ผู้ป่วยตัวอย่าง (De-identified Patient)"}],
        "gender": "unknown",
    }


def build_condition(entity: dict, idx: int, when: str) -> dict:
    raw = entity.get("raw_text", "").strip()
    rid = f"cond-{idx:02d}-{slug(raw, 32)}"
    codings = icd_codings(entity.get("icd_codes"))
    code = {"coding": codings, "text": raw} if codings else {"text": raw}
    return {
        "resourceType": "Condition",
        "id": rid,
        "meta": {"profile": [PROFILE["Condition"]]},
        "clinicalStatus": {"coding": [{"system": SYS_COND_CLINICAL, "code": "active"}]},
        "category": [{"coding": [{"system": SYS_COND_CATEGORY,
                                  "code": "encounter-diagnosis",
                                  "display": "Encounter Diagnosis"}]}],
        "code": code,
        "subject": ref("Patient", "patient-001"),
        "recordedDate": when,
    }


def build_observation(entity: dict, idx: int, when: str) -> dict:
    raw = entity.get("raw_text", "").strip()
    etype = entity["type"]  # VITAL_SIGN | LAB_VALUE | IMAGING
    rid = f"obs-{idx:02d}-{slug(raw, 28)}"
    obs = {
        "resourceType": "Observation",
        "id": rid,
        "meta": {"profile": [PROFILE["Observation"]]},
        "status": "final",
        "subject": ref("Patient", "patient-001"),
        "effectiveDateTime": when,
    }

    if etype == "IMAGING":
        modality = re.match(r"\s*([A-Za-z/]+)", raw)
        obs["category"] = [{"coding": [{"system": SYS_OBS_CATEGORY, "code": "imaging",
                                        "display": "Imaging"}]}]
        obs["code"] = {"text": (modality.group(1) if modality else "Imaging")}
        obs["valueString"] = raw
        return obs

    cat_code = "vital-signs" if etype == "VITAL_SIGN" else "laboratory"
    obs["category"] = [{"coding": [{"system": SYS_OBS_CATEGORY, "code": cat_code,
                                    "display": cat_code.replace("-", " ").title()}]}]

    # Blood pressure -> panel with systolic/diastolic components
    bp = re.search(r"BP\s*(\d+)\s*/\s*(\d+)", raw)
    if bp:
        obs["code"] = {"coding": [{"system": SYS_LOINC, "code": "85354-9",
                                   "display": "Blood pressure panel"}], "text": raw}
        obs["component"] = [
            {"code": {"coding": [{"system": SYS_LOINC, "code": "8480-6",
                                  "display": "Systolic blood pressure"}]},
             "valueQuantity": {"value": int(bp.group(1)), "unit": "mmHg",
                               "system": SYS_UCUM, "code": "mm[Hg]"}},
            {"code": {"coding": [{"system": SYS_LOINC, "code": "8462-4",
                                  "display": "Diastolic blood pressure"}]},
             "valueQuantity": {"value": int(bp.group(2)), "unit": "mmHg",
                               "system": SYS_UCUM, "code": "mm[Hg]"}},
        ]
        return obs

    # Mapped single-value vitals / labs
    for key, (loinc, disp, unit, ucum) in VITAL_MAP.items():
        if raw.upper().startswith(key.upper()):
            parsed = parse_value(raw)
            obs["code"] = {"coding": [{"system": SYS_LOINC, "code": loinc, "display": disp}],
                           "text": raw}
            if parsed:
                obs["valueQuantity"] = {"value": parsed[1], "unit": unit,
                                        "system": SYS_UCUM, "code": ucum}
            else:
                obs["valueString"] = raw
            return obs

    # Fallback (e.g. "V/S stable", "IVC max 0.9")
    obs["code"] = {"text": raw}
    obs["valueString"] = raw
    return obs


def build_medication_request(med_name: str, idx: int, when: str, raw_dose: str) -> dict:
    key = med_name.strip().lower()
    cat = MED_CATALOG.get(key, {"display": med_name.title(), "dose": raw_dose or "see order"})
    rid = f"medreq-{idx:02d}-{slug(med_name, 24)}"
    note = []
    if raw_dose:
        note.append({"text": f"OCR source (verify): {raw_dose.strip()[:180]}"})

    # B3: try a trusted SNOMED+EDQM doseForm from the TMT id. When it resolves we
    # emit a contained Medication (code=TMT, doseForm=resolved) and reference it;
    # otherwise keep the inline concept shape (a slug placeholder code, no doseForm).
    dose_form = None
    tmt_id = cat.get("tmt_id")
    if RESOLVE_DOSEFORM and tmt_id:
        try:
            from fhir_dose_form import resolve_dose_form
            dose_form = resolve_dose_form(tmt_id)
        except Exception as exc:  # never let enrichment break the offline transform
            print(f"  doseForm resolve skipped for {key}: {exc}", file=sys.stderr)

    medreq = {
        "resourceType": "MedicationRequest",
        "id": rid,
        "meta": {"profile": [PROFILE["MedicationRequest"]]},
        "status": "active",
        "intent": "order",
        "subject": ref("Patient", "patient-001"),
        "authoredOn": when,
        "dosageInstruction": [{"text": cat["dose"]}],
        **({"note": note} if note else {}),
    }
    if dose_form:
        med_id = f"med-{idx:02d}-{slug(med_name, 20)}"
        medreq["contained"] = [{
            "resourceType": "Medication",
            "id": med_id,
            "code": {"coding": [{"system": SYS_TMT, "code": tmt_id, "display": cat["display"]}],
                     "text": cat["display"]},
            "doseForm": dose_form,
        }]
        # R5 CodeableReference: reference the contained Medication carrying doseForm.
        medreq["medication"] = {"reference": {"reference": f"#{med_id}"}}
    else:
        # R5: medication is a CodeableReference (was medicationReference in R4)
        medreq["medication"] = {"concept": {
            "coding": [{"system": SYS_TMT, "code": tmt_id or slug(key, 20),
                        "display": cat["display"]}],
            "text": cat["display"],
        }}
    return medreq


def build_composition(doc_type: str, when: str, cond_ids, obs_ids, med_ids) -> dict:
    loinc, disp = DOC_TYPE_LOINC.get(doc_type, ("34109-9", "Note"))
    sections = []
    if cond_ids:
        sections.append({"title": "Diagnoses / การวินิจฉัย",
                         "code": {"coding": [{"system": SYS_LOINC, "code": "29548-5",
                                              "display": "Diagnosis"}]},
                         "entry": [ref("Condition", i) for i in cond_ids]})
    if med_ids:
        sections.append({"title": "Medications / รายการยา",
                         "code": {"coding": [{"system": SYS_LOINC, "code": "10160-0",
                                              "display": "History of medication use"}]},
                         "entry": [ref("MedicationRequest", i) for i in med_ids]})
    if obs_ids:
        sections.append({"title": "Vital Signs, Labs & Imaging / สัญญาณชีพ ผลแล็บ และภาพถ่าย",
                         "code": {"coding": [{"system": SYS_LOINC, "code": "8716-3",
                                              "display": "Vital signs"}]},
                         "entry": [ref("Observation", i) for i in obs_ids]})
    return {
        "resourceType": "Composition",
        "id": f"composition-{slug(doc_type, 24)}",
        "meta": {"profile": [PROFILE["Composition"]]},
        "status": "final",
        "type": {"coding": [{"system": SYS_LOINC, "code": loinc, "display": disp}]},
        "subject": ref("Patient", "patient-001"),
        "date": when,
        "author": [{"display": "Asgard / Syn OCR pipeline"}],
        "title": DOC_TYPE_TITLE_TH.get(doc_type, disp),
        "section": sections,
    }


def entry(resource: dict) -> dict:
    return {"fullUrl": f"{BASE}/{resource['resourceType']}/{resource['id']}",
            "resource": resource}


# --------------------------------------------------------------------------- #
# Per-document bundle
# --------------------------------------------------------------------------- #
def build_bundle(doc: dict) -> dict:
    doc_id = doc["document_id"]
    doc_type = doc["document_type"]
    when = doc.get("extracted_at")

    patient = build_patient()

    conditions, observations, med_requests = [], [], []
    seen_cond, seen_med = set(), set()

    for i, e in enumerate(doc.get("entities", [])):
        t = e["type"]
        if t == "DIAGNOSIS":
            key = (frozenset(c["code"] for c in e.get("icd_codes", [])),
                   e.get("normalized_text", e.get("raw_text", "")))
            if key in seen_cond:
                continue
            seen_cond.add(key)
            conditions.append(build_condition(e, len(conditions) + 1, when))
        elif t in ("VITAL_SIGN", "LAB_VALUE", "IMAGING"):
            observations.append(build_observation(e, len(observations) + 1, when))
        elif t == "MEDICATION":
            name = e.get("raw_text", "").strip()
            mkey = name.lower()
            if mkey in seen_med:
                continue
            seen_med.add(mkey)
            # find raw dosage from embedded fhir_resources if present
            raw_dose = ""
            for en in doc.get("fhir_resources", {}).get("entry", []):
                r = en["resource"]
                if r.get("resourceType") == "MedicationRequest" and mkey in json.dumps(r).lower():
                    di = r.get("dosageInstruction") or [{}]
                    raw_dose = di[0].get("text", "")
                    break
            med_requests.append(build_medication_request(name, len(med_requests) + 1, when, raw_dose))

    composition = build_composition(
        doc_type, when,
        [c["id"] for c in conditions],
        [o["id"] for o in observations],
        [m["id"] for m in med_requests],
    )

    entries = [entry(composition), entry(patient)]
    entries += [entry(c) for c in conditions]
    entries += [entry(m) for m in med_requests]
    entries += [entry(o) for o in observations]

    return {
        "resourceType": "Bundle",
        "id": f"doc-bundle-{doc_id}",
        "meta": {"profile": ["http://hl7.org/fhir/StructureDefinition/Bundle"]},
        "type": "document",
        "timestamp": when,
        "identifier": {"system": f"{BASE}/document-bundle", "value": f"DOC-{doc_id}"},
        "entry": entries,
    }


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    index_rows = []
    for n in range(1, 8):
        doc = json.loads((IN_DIR / f"extraction_{n}.json").read_text())
        bundle = build_bundle(doc)
        out = OUT_DIR / f"bundle_{n}.json"
        out.write_text(json.dumps(bundle, ensure_ascii=False, indent=2))
        counts = {}
        for en in bundle["entry"]:
            rt = en["resource"]["resourceType"]
            counts[rt] = counts.get(rt, 0) + 1
        index_rows.append((n, doc["document_type"], counts))
        summary = ", ".join(f"{v}×{k}" for k, v in counts.items())
        print(f"bundle_{n}.json  [{doc['document_type']:<20}]  {summary}")

    # Index / mapping summary
    lines = [
        "# FHIR R5 Document Bundles — Gathered Clinical Records",
        "",
        "Thai-profiled FHIR R5 (5.0.0) `Bundle` (type=document), one per source document.",
        "ICD-10-TM primary coding (+ ICD-9-CM equivalence); MedicationRequest.medication as R5 CodeableReference.",
        "When FHIR_RESOLVE_DOSEFORM=1, TMT-coded meds emit a contained Medication with a SNOMED+EDQM-coded doseForm (Sprint 58 dose link, needs_review=0 only).",
        "",
        "| # | Source document | FHIR resources |",
        "|---|-----------------|----------------|",
    ]
    for n, dtype, counts in index_rows:
        summary = ", ".join(f"{v}×{k}" for k, v in counts.items())
        lines.append(f"| {n} | {dtype} | {summary} |")
    (OUT_DIR / "INDEX.md").write_text("\n".join(lines) + "\n")
    print(f"\nWrote {len(index_rows)} bundles + INDEX.md to {OUT_DIR}")


if __name__ == "__main__":
    main()
