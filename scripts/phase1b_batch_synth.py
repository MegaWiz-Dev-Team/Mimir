#!/usr/bin/env python3
"""
Sprint 39 Phase 1b — production-quality corpus synthesis via Gemini Batch API.

Replaces local-gemma synthesis (Phase 1a MVP) with cloud Gemini 3 Flash batched.
Cost: ~$6 for 5,000 pairs (50% batch discount on $0.50/$3 per 1M tokens).

Workflow:
  1. Build JSONL of 1,000 batched calls (5 pairs each → 5K total pairs)
  2. Upload to Gemini Files API (input file)
  3. Submit batch job → returns batch_name
  4. Poll status until COMPLETED (typical: minutes to hours)
  5. Download results JSONL
  6. Parse Q-A pairs (regex on ===Q1===/===A1=== delimiters, same as MVP)
  7. Bulk-import to Curator dataset

Cost ceiling: $6 batch (50% off $12 standard at 4.5M tokens). User-approved.
"""

from __future__ import annotations
import argparse
import json
import os
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import requests

GEMINI_API_KEY = os.environ.get("GEMINI_API_KEY", "")
GEMINI_BASE = "https://generativelanguage.googleapis.com/v1beta"
MIMIR_API = os.environ.get("MIMIR_API_URL", "http://localhost:30000/api/v1")
TENANT = os.environ.get("MIMIR_TENANT", "asgard_medical")
MODEL = os.environ.get("PHASE1B_MODEL", "models/gemini-3-flash-preview")


@dataclass
class TopicSeed:
    specialty: str
    topic: str
    n_pairs: int
    suggested_tags: list[str]


# Same seed list as Phase 1a MVP, scaled up: each topic produces N pairs.
# For 5K pairs target with ~50 distinct topics × 100 each.
SEEDS: list[TopicSeed] = [
    # ─── Cardiology (target ~600 pairs) ─────────────────────────────────────
    TopicSeed("cardiology", "atrial fibrillation rate vs rhythm control + anticoagulation", 5, ["pharmacy", "anticoagulation"]),
    TopicSeed("cardiology", "heart failure with reduced ejection fraction (HFrEF) — guideline-directed therapy + GDMT progression", 5, ["pharmacy", "monitoring"]),
    TopicSeed("cardiology", "STEMI / NSTEMI / unstable angina — recognition, ECG patterns, immediate management", 5, ["emergency", "urgent", "imaging"]),
    TopicSeed("cardiology", "hypertensive emergency vs urgency — recognition + IV vs oral therapy choices", 5, ["emergency", "pharmacy"]),
    TopicSeed("cardiology", "valvular heart disease — aortic stenosis, mitral regurgitation, indications for surgery vs TAVR", 5, ["imaging"]),
    TopicSeed("cardiology", "supraventricular tachycardias (SVT, AVNRT, atrial flutter)", 5, ["pharmacy", "emergency"]),
    TopicSeed("cardiology", "lipid management + statin intensity selection", 5, ["pharmacy", "screening"]),
    TopicSeed("cardiology", "pericarditis vs myocarditis vs pericardial effusion + tamponade", 4, ["differential-dx", "imaging", "urgent"]),
    TopicSeed("cardiology", "pulmonary hypertension — WHO classification, workup, treatment", 4, ["differential-dx"]),
    TopicSeed("cardiology", "preoperative cardiac risk stratification (RCRI, ACC/AHA)", 4, ["preoperative", "screening"]),
    TopicSeed("cardiology", "endocarditis — Duke criteria, antibiotic choices, surgical indications", 4, ["antibiotic", "differential-dx"]),
    TopicSeed("cardiology", "DVT and pulmonary embolism — workup, scoring (Wells, PERC), anticoagulation duration", 5, ["urgent", "anticoagulation", "imaging"]),
    TopicSeed("cardiology", "syncope — neurocardiogenic vs orthostatic vs cardiac, when to admit", 4, ["differential-dx", "urgent"]),
    TopicSeed("cardiology", "aortic dissection — Stanford classification, imaging choice, BP control", 4, ["urgent", "red-flag", "imaging"]),

    # ─── Endocrinology (target ~600 pairs) ──────────────────────────────────
    TopicSeed("endocrinology", "T2DM stepwise pharmacotherapy + agent selection by comorbidity (CKD, ASCVD, HF)", 5, ["pharmacy", "monitoring"]),
    TopicSeed("endocrinology", "DKA and HHS — diagnostic criteria, insulin protocol, electrolyte management", 5, ["emergency", "pharmacy", "icu"]),
    TopicSeed("endocrinology", "thyroid disorders — hyperthyroidism (Graves, toxic nodule), workup + RAI vs surgery", 5, ["pharmacy"]),
    TopicSeed("endocrinology", "hypothyroidism + subclinical hypothyroidism + pregnancy considerations", 4, ["pregnancy", "pharmacy"]),
    TopicSeed("endocrinology", "thyroid nodules — TIRADS, FNA indications", 4, ["imaging", "screening"]),
    TopicSeed("endocrinology", "Cushing syndrome — ACTH-dependent vs independent, IPSS, treatment options", 4, ["differential-dx", "imaging"]),
    TopicSeed("endocrinology", "primary aldosteronism — screening, confirmatory testing, treatment", 4, ["screening", "lab-interpretation"]),
    TopicSeed("endocrinology", "adrenal insufficiency — primary vs secondary, stress dosing", 4, ["pharmacy", "preoperative"]),
    TopicSeed("endocrinology", "hypercalcemia and hyperparathyroidism — workup, surgical indications", 4, ["differential-dx", "lab-interpretation"]),
    TopicSeed("endocrinology", "osteoporosis — DEXA interpretation, FRAX, bisphosphonate vs denosumab", 4, ["pharmacy", "screening"]),
    TopicSeed("endocrinology", "diabetic complications — retinopathy, nephropathy, neuropathy screening + management", 5, ["screening", "monitoring"]),
    TopicSeed("endocrinology", "pituitary disorders — adenomas, hypopituitarism, prolactinoma management", 4, ["pharmacy", "imaging"]),

    # ─── ENT (target ~400 pairs) ────────────────────────────────────────────
    TopicSeed("ent", "acute otitis media in adults + children — antibiotic indications, watchful waiting", 4, ["pediatric", "antibiotic"]),
    TopicSeed("ent", "chronic otitis media + cholesteatoma + tympanostomy tubes", 4, ["pediatric"]),
    TopicSeed("ent", "rhinitis — allergic vs non-allergic, intranasal corticosteroid use, antihistamines", 4, ["pharmacy", "outpatient"]),
    TopicSeed("ent", "acute bacterial vs viral sinusitis + chronic rhinosinusitis", 4, ["antibiotic", "differential-dx"]),
    TopicSeed("ent", "vertigo — BPPV vs Meniere vs vestibular neuritis, Dix-Hallpike, Epley maneuver", 4, ["differential-dx", "outpatient"]),
    TopicSeed("ent", "sudden sensorineural hearing loss — emergency steroid, MRI", 4, ["emergency", "urgent", "pharmacy"]),
    TopicSeed("ent", "epistaxis — anterior vs posterior, packing, cautery, when to refer", 4, ["emergency", "outpatient"]),
    TopicSeed("ent", "tonsillitis + peritonsillar abscess + cellulitis differentiation", 4, ["antibiotic", "differential-dx"]),
    TopicSeed("ent", "laryngitis + voice disorders + vocal cord lesions", 3, ["differential-dx"]),
    TopicSeed("ent", "OSA — diagnosis (polysomnography), CPAP titration, surgical alternatives", 4, ["postoperative", "monitoring"]),

    # ─── Pediatrics (target ~500 pairs) ─────────────────────────────────────
    TopicSeed("pediatrics", "pediatric asthma — diagnosis, severity, controller therapy, exacerbation", 5, ["pediatric", "pharmacy", "dosing"]),
    TopicSeed("pediatrics", "pediatric UTI — diagnosis, imaging (VCUG/RBUS), antibiotic choices", 4, ["pediatric", "antibiotic"]),
    TopicSeed("pediatrics", "pediatric pneumonia — bacterial vs viral, antibiotics, RSV bronchiolitis management", 5, ["pediatric", "antibiotic", "imaging"]),
    TopicSeed("pediatrics", "growth and developmental milestones — failure to thrive workup", 4, ["pediatric", "screening"]),
    TopicSeed("pediatrics", "vaccinations + catch-up schedules + contraindications", 4, ["pediatric", "screening"]),
    TopicSeed("pediatrics", "Kawasaki disease — diagnostic criteria, IVIG + aspirin", 4, ["urgent", "pharmacy", "differential-dx"]),
    TopicSeed("pediatrics", "neonatal jaundice — physiologic vs pathologic, phototherapy thresholds", 4, ["pediatric", "monitoring"]),
    TopicSeed("pediatrics", "pediatric fever workup by age — neonate, infant, toddler", 4, ["pediatric", "differential-dx"]),
    TopicSeed("pediatrics", "ADHD diagnosis + first-line stimulants + non-stimulant alternatives", 4, ["pediatric", "pharmacy", "outpatient"]),
    TopicSeed("pediatrics", "pediatric seizures + first unprovoked seizure workup", 4, ["pediatric", "imaging", "urgent"]),

    # ─── Emergency medicine (target ~500 pairs) ─────────────────────────────
    TopicSeed("emergency-medicine", "sepsis recognition (Sepsis-3, qSOFA, lactate) + bundle (3-hour, 6-hour)", 5, ["urgent", "icu", "antibiotic"]),
    TopicSeed("emergency-medicine", "stroke — last-known-well, NIH stroke scale, tPA window, thrombectomy", 5, ["urgent", "imaging", "red-flag"]),
    TopicSeed("emergency-medicine", "anaphylaxis — recognition, IM epinephrine, biphasic risk", 4, ["pharmacy", "urgent", "allergy"]),
    TopicSeed("emergency-medicine", "trauma triage — primary/secondary survey, FAST, transfusion criteria", 4, ["urgent", "imaging"]),
    TopicSeed("emergency-medicine", "acute kidney injury — pre-renal vs renal vs post-renal, contrast nephropathy", 4, ["differential-dx", "lab-interpretation", "nephrology"]),
    TopicSeed("emergency-medicine", "acute abdomen — appendicitis, cholecystitis, diverticulitis, pancreatitis differentiation", 5, ["differential-dx", "imaging", "urgent"]),
    TopicSeed("emergency-medicine", "GI bleed — UGIB vs LGIB, Glasgow-Blatchford, endoscopy timing", 4, ["urgent", "monitoring"]),
    TopicSeed("emergency-medicine", "respiratory failure — type I vs II, NIV indications, intubation criteria", 4, ["urgent", "icu", "monitoring"]),
    TopicSeed("emergency-medicine", "burn management — Parkland formula, depth assessment, transfer criteria", 4, ["urgent", "dosing"]),
    TopicSeed("emergency-medicine", "toxicology — common overdoses (acetaminophen, opioids, salicylates), antidotes", 4, ["pharmacy", "urgent", "antidote"]),

    # ─── Psychiatry (target ~400 pairs) ─────────────────────────────────────
    TopicSeed("psychiatry", "MDD — diagnosis, PHQ-9, first-line SSRI + augmentation strategies", 4, ["pharmacy", "screening", "outpatient"]),
    TopicSeed("psychiatry", "anxiety disorders (GAD, panic, social, PTSD) — diagnosis + treatment", 4, ["pharmacy", "outpatient"]),
    TopicSeed("psychiatry", "bipolar I vs II — mania, hypomania, mood stabilizers, lithium monitoring", 4, ["pharmacy", "monitoring"]),
    TopicSeed("psychiatry", "schizophrenia and psychosis — antipsychotic selection, EPS, metabolic monitoring", 4, ["pharmacy", "monitoring"]),
    TopicSeed("psychiatry", "suicide risk assessment — C-SSRS, acute intervention, hospitalization criteria", 4, ["urgent", "red-flag", "screening"]),
    TopicSeed("psychiatry", "substance use disorders — alcohol withdrawal CIWA, opioid use disorder, MAT", 4, ["pharmacy", "urgent", "outpatient"]),
    TopicSeed("psychiatry", "dementia vs delirium vs depression in elderly — differentiation + workup", 4, ["geriatric", "differential-dx"]),
    TopicSeed("psychiatry", "eating disorders — anorexia, bulimia, refeeding syndrome", 4, ["monitoring", "differential-dx"]),
    TopicSeed("psychiatry", "perinatal depression + medication safety in pregnancy/breastfeeding", 3, ["pregnancy", "pharmacy", "obgyn"]),
    TopicSeed("psychiatry", "ADHD adult — diagnosis, stimulant + non-stimulant, comorbidities", 3, ["pharmacy", "outpatient"]),

    # ─── Pharmacy (target ~500 pairs) ───────────────────────────────────────
    TopicSeed("pharmacy", "warfarin interactions + INR management in polypharmacy", 5, ["interaction", "anticoagulation", "geriatric", "monitoring"]),
    TopicSeed("pharmacy", "DOAC selection (apixaban, rivaroxaban, dabigatran) by indication + renal function", 5, ["anticoagulation", "nephrology"]),
    TopicSeed("pharmacy", "renal dose adjustments for common antibiotics (vancomycin, aminoglycosides, beta-lactams)", 5, ["dosing", "nephrology", "antibiotic"]),
    TopicSeed("pharmacy", "hepatic dose adjustments + Child-Pugh-based dosing", 4, ["dosing", "monitoring"]),
    TopicSeed("pharmacy", "medication safety in pregnancy + lactation (FDA + LactMed)", 4, ["pregnancy", "obgyn", "contraindication"]),
    TopicSeed("pharmacy", "geriatric prescribing — Beers criteria, polypharmacy review, deprescribing", 4, ["geriatric", "interaction"]),
    TopicSeed("pharmacy", "adverse drug reactions — common patterns, type A vs B, reporting", 4, ["monitoring", "interaction"]),
    TopicSeed("pharmacy", "antibiotic stewardship — empiric vs targeted, narrow-spectrum, duration", 4, ["antibiotic", "monitoring"]),
    TopicSeed("pharmacy", "pain management — WHO ladder, opioid conversion, neuropathic pain", 4, ["pharmacy", "outpatient"]),
    TopicSeed("pharmacy", "vaccination during immunosuppression / chemotherapy / HIV", 3, ["pharmacy", "screening"]),

    # ─── General medicine (target ~500 pairs) ───────────────────────────────
    TopicSeed("general-medicine", "COPD — GOLD classification, inhaler stepwise therapy, exacerbation management", 5, ["pharmacy", "monitoring", "outpatient"]),
    TopicSeed("general-medicine", "GERD + peptic ulcer + H. pylori eradication", 4, ["pharmacy", "outpatient"]),
    TopicSeed("general-medicine", "anemia workup — micro/normo/macrocytic, iron studies, B12/folate", 5, ["differential-dx", "lab-interpretation"]),
    TopicSeed("general-medicine", "CKD — staging, ACR, progression monitoring, RAAS blockade", 5, ["nephrology", "monitoring", "pharmacy"]),
    TopicSeed("general-medicine", "thrombocytopenia workup — ITP vs TTP vs HIT vs DIC", 4, ["differential-dx", "urgent", "lab-interpretation"]),
    TopicSeed("general-medicine", "IBD — Crohn vs UC, induction vs maintenance, biologics", 4, ["pharmacy", "monitoring", "differential-dx"]),
    TopicSeed("general-medicine", "rheumatoid arthritis vs OA + early DMARD therapy", 4, ["pharmacy", "monitoring", "rheumatology"]),
    TopicSeed("general-medicine", "lupus + APS — diagnostic criteria, organ involvement, monitoring", 4, ["differential-dx", "monitoring", "rheumatology"]),
    TopicSeed("general-medicine", "preventive care + USPSTF screening recommendations by age/sex", 5, ["screening", "outpatient"]),
    TopicSeed("general-medicine", "hyperlipidemia + cardiovascular risk + ASCVD calculator", 4, ["screening", "pharmacy"]),
]


def synth_prompt(specialty: str, topic: str, n_pairs: int) -> str:
    return f"""You are a medical-education content writer. Generate exactly {n_pairs} realistic clinical question-answer pairs about: **{topic}** (specialty: {specialty}).

Each QUESTION: 1-3 sentences — a realistic question a junior physician would ask a senior colleague (vignette OR direct form). Vary structure across the {n_pairs} pairs.

Each ANSWER: 3-7 sentences of medically accurate, guideline-aligned content. Include specific numbers (doses, thresholds, scoring criteria). Note when professional consultation is required.

Use this EXACT delimiter format (no JSON, no markdown fences, no preamble):

===Q1===
<question text>
===A1===
<answer text>

===Q2===
<question text>
===A2===
<answer text>

(continue through ===Q{n_pairs}=== / ===A{n_pairs}===)

Start your response with ===Q1===. Do not add any commentary before or after."""


def build_batch_jsonl(seeds: list[TopicSeed], scale: int) -> list[dict]:
    """Build batch request JSONL. Each row = one Gemini API call.

    `scale` = pairs-per-topic multiplier. e.g. seed n=5 with scale=2 → ask for 10.
    Limit individual calls to <= 8 pairs each (Gemini batch quality drop above).

    Keys use stable MD5 (not Python's randomized hash) so the same input across
    runs produces the same keys — essential for resuming via --batch-name.
    """
    import hashlib
    requests_list = []
    for seed in seeds:
        per_call = min(seed.n_pairs * scale, 8)
        n_calls = max(1, (seed.n_pairs * scale + per_call - 1) // per_call)
        topic_hash = hashlib.md5(seed.topic.encode()).hexdigest()[:8]
        for i in range(n_calls):
            req = {
                "key": f"{seed.specialty}__{topic_hash}__{i}",
                "request": {
                    "contents": [
                        {"role": "user", "parts": [{"text": synth_prompt(seed.specialty, seed.topic, per_call)}]},
                        {"role": "model", "parts": [{"text": "===Q1==="}]},
                    ],
                    "generationConfig": {
                        "temperature": 0.6,
                        "maxOutputTokens": 4096,
                    },
                },
                # Capture metadata so we can reattach specialty + tags after batch returns.
                "metadata": {
                    "specialty": seed.specialty,
                    "topic": seed.topic,
                    "tags": seed.suggested_tags,
                },
            }
            requests_list.append(req)
    return requests_list


_QA_RE = re.compile(
    r"===Q(\d+)===\s*(.*?)\s*===A\1===\s*(.*?)(?=\s*===Q\d+===|\s*$)",
    re.DOTALL,
)


def parse_pairs(text: str) -> list[dict]:
    if not text:
        return []
    if "===Q1===" not in text:
        text = "===Q1===\n" + text
    pairs = []
    for m in _QA_RE.finditer(text):
        q = m.group(2).strip().strip("`").strip()
        a = m.group(3).strip().strip("`").strip()
        if q and a and len(q) > 20 and len(a) > 50:
            pairs.append({"question": q, "ai_answer": a})
    return pairs


# ─── Gemini Batch API client ─────────────────────────────────────────────────

def upload_input_file(jsonl_path: Path) -> str:
    """Upload JSONL via Files API resumable upload. Returns file_name like 'files/abc123'.

    Correct endpoint: /upload/v1beta/files (note the /upload/ prefix).
    """
    upload_base = "https://generativelanguage.googleapis.com/upload/v1beta/files"
    file_size = jsonl_path.stat().st_size
    # Step 1: start resumable upload session
    headers = {
        "X-Goog-Upload-Protocol": "resumable",
        "X-Goog-Upload-Command": "start",
        "X-Goog-Upload-Header-Content-Length": str(file_size),
        "X-Goog-Upload-Header-Content-Type": "application/jsonl",
        "Content-Type": "application/json",
    }
    meta = {"file": {"display_name": "phase1b-batch-input"}}
    url = f"{upload_base}?key={GEMINI_API_KEY}"
    r = requests.post(url, headers=headers, json=meta, timeout=30)
    r.raise_for_status()
    upload_url = r.headers.get("X-Goog-Upload-URL") or r.headers.get("x-goog-upload-url")
    if not upload_url:
        raise RuntimeError(
            f"no upload URL in response (status={r.status_code}). "
            f"Headers: {dict(r.headers)}. Body: {r.text[:300]}"
        )
    # Step 2: upload bytes + finalize
    with open(jsonl_path, "rb") as f:
        data = f.read()
    headers2 = {
        "Content-Length": str(len(data)),
        "X-Goog-Upload-Offset": "0",
        "X-Goog-Upload-Command": "upload, finalize",
    }
    r2 = requests.post(upload_url, headers=headers2, data=data, timeout=300)
    r2.raise_for_status()
    return r2.json()["file"]["name"]


def submit_batch(input_file: str, model: str, display_name: str) -> str:
    """Submit batch generation job. Returns batch_name like 'batches/xyz'."""
    payload = {
        "batch": {
            "displayName": display_name,
            "inputConfig": {
                "fileName": input_file,
            },
        }
    }
    url = f"{GEMINI_BASE}/{model}:batchGenerateContent?key={GEMINI_API_KEY}"
    r = requests.post(url, json=payload, timeout=30)
    if not r.ok:
        print(f"batch submit failed: {r.status_code} {r.text}", file=sys.stderr)
        r.raise_for_status()
    return r.json()["name"]


SUCCESS_STATES = {
    "COMPLETED", "SUCCEEDED",
    "JOB_STATE_SUCCEEDED", "JOB_STATE_COMPLETED",
    "BATCH_STATE_SUCCEEDED", "BATCH_STATE_COMPLETED",
}
FAILURE_STATES = {
    "FAILED", "CANCELLED", "EXPIRED",
    "JOB_STATE_FAILED", "JOB_STATE_CANCELLED",
    "BATCH_STATE_FAILED", "BATCH_STATE_CANCELLED", "BATCH_STATE_EXPIRED",
}


def poll_batch(batch_name: str, max_min: int = 120) -> dict | None:
    """Poll batch until terminal state. Returns batch resource dict."""
    url = f"{GEMINI_BASE}/{batch_name}?key={GEMINI_API_KEY}"
    start = time.time()
    while True:
        r = requests.get(url, timeout=30)
        r.raise_for_status()
        info = r.json()
        state = info.get("metadata", {}).get("state") or info.get("state")
        elapsed = int(time.time() - start)
        print(f"[{elapsed:5d}s] batch state={state}")
        if state in SUCCESS_STATES:
            return info
        if state in FAILURE_STATES:
            print(f"batch failed: {json.dumps(info, indent=2)[:1000]}", file=sys.stderr)
            return None
        if elapsed > max_min * 60:
            print(f"timeout waiting batch {max_min}min", file=sys.stderr)
            return None
        time.sleep(60)


def download_results(batch_info: dict) -> list[dict]:
    """Download batch result JSONL via Files API.

    Output path lives at metadata.output.responsesFile in the v1main API shape.
    """
    md = batch_info.get("metadata", {})
    out_file = (
        md.get("output", {}).get("responsesFile")
        or md.get("outputFile")
        or batch_info.get("response", {}).get("outputConfig", {}).get("file")
    )
    if not out_file:
        raise RuntimeError(f"no output file in batch info: {json.dumps(batch_info, indent=2)[:500]}")
    # Files download endpoint
    url = f"{GEMINI_BASE}/{out_file}:download?key={GEMINI_API_KEY}&alt=media"
    r = requests.get(url, timeout=300)
    if not r.ok:
        # Try alternate download endpoint
        alt_url = f"https://generativelanguage.googleapis.com/download/v1beta/{out_file}?alt=media&key={GEMINI_API_KEY}"
        r = requests.get(alt_url, timeout=300)
        r.raise_for_status()
    rows = [json.loads(ln) for ln in r.text.split("\n") if ln.strip()]
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-pairs", type=int, default=5000)
    ap.add_argument("--scale", type=int, default=1, help="pairs-per-topic multiplier (1=base, 2=double)")
    ap.add_argument("--dataset-name", default="Sprint 39 Phase 1b corpus (gemini-3-flash batch)")
    ap.add_argument("--dataset-id", help="Reuse existing dataset")
    ap.add_argument("--prepare-only", action="store_true",
                    help="Build JSONL + show plan, do NOT submit batch")
    ap.add_argument("--input-file", help="Reuse existing files/<id> instead of re-upload")
    ap.add_argument("--batch-name", help="Reuse existing batches/<id> (skip submit, just poll)")
    ap.add_argument("--scale-down-to", type=int, default=None,
                    help="Cap total requests for cost control / dry-run")
    args = ap.parse_args()

    if not GEMINI_API_KEY:
        print("GEMINI_API_KEY missing. Set env var.", file=sys.stderr)
        sys.exit(1)

    print(f"╔══════════════════════════════════════════════════════════════╗")
    print(f"║ Sprint 39 Phase 1b — Gemini Batch synth                      ║")
    print(f"╚══════════════════════════════════════════════════════════════╝")
    print(f"Model:        {MODEL}")
    print(f"Target:       {args.max_pairs} pairs (scale={args.scale})")
    print()

    # Build batch requests
    batch_requests = build_batch_jsonl(SEEDS, args.scale)
    if args.scale_down_to:
        batch_requests = batch_requests[:args.scale_down_to]
    expected_pairs = sum(
        min(req["request"]["contents"][0]["parts"][0]["text"].count("===Q") - 1, 8) or 5
        for req in batch_requests
    )  # rough — based on per-call request size from prompt
    print(f"Built {len(batch_requests)} batch requests (~{expected_pairs} expected pairs)")
    print(f"Specialties: {len(set(r['metadata']['specialty'] for r in batch_requests))}")

    if args.prepare_only:
        print("(--prepare-only: stopping)")
        return

    # Write JSONL
    work = Path("/tmp/phase1b_batch")
    work.mkdir(parents=True, exist_ok=True)
    input_path = work / "batch_input.jsonl"
    with open(input_path, "w") as f:
        for r in batch_requests:
            # Strip metadata before submitting (keep our own side-table for tags)
            submit_obj = {"key": r["key"], "request": r["request"]}
            f.write(json.dumps(submit_obj) + "\n")
    print(f"JSONL written: {input_path} ({input_path.stat().st_size} bytes)")

    # Save metadata side-table for reattach later
    meta_path = work / "batch_metadata.json"
    meta_table = {r["key"]: r["metadata"] for r in batch_requests}
    meta_path.write_text(json.dumps(meta_table, indent=2))

    # Upload + submit
    if args.input_file:
        file_name = args.input_file
        print(f"Reusing input file: {file_name}")
    else:
        print(f"Uploading input file...")
        file_name = upload_input_file(input_path)
        print(f"  uploaded: {file_name}")

    if args.batch_name:
        batch_name = args.batch_name
        print(f"Reusing batch: {batch_name}")
    else:
        print(f"Submitting batch job...")
        batch_name = submit_batch(file_name, MODEL,
                                   f"phase1b-{int(time.time())}")
        print(f"  batch: {batch_name}")

    # Poll
    print(f"Polling for completion (typical: minutes to hours)...")
    info = poll_batch(batch_name, max_min=180)
    if not info:
        print("❌ batch did not complete cleanly")
        sys.exit(1)

    # Download
    print(f"Downloading results...")
    rows = download_results(info)
    print(f"  got {len(rows)} response rows")

    # Save raw results
    raw_path = work / "batch_results.jsonl"
    raw_path.write_text("\n".join(json.dumps(r) for r in rows))

    # Parse pairs + reattach metadata
    items_for_curator = []
    for row in rows:
        key = row.get("key", "")
        meta = meta_table.get(key, {})
        # Each response should have candidates → content → parts[].text
        try:
            text = row["response"]["candidates"][0]["content"]["parts"][0]["text"]
        except (KeyError, IndexError, TypeError):
            print(f"  skip {key}: no text", file=sys.stderr)
            continue
        pairs = parse_pairs(text)
        for p in pairs:
            items_for_curator.append({
                "question": p["question"],
                "ai_answer": p["ai_answer"],
                "specialty": meta.get("specialty"),
                "tags": meta.get("tags", []),
            })
    print(f"  parsed {len(items_for_curator)} valid pairs")

    # Save parsed JSONL
    parsed_path = work / "parsed_pairs.jsonl"
    parsed_path.write_text("\n".join(json.dumps(p) for p in items_for_curator) + "\n")

    # Import to Curator
    if args.dataset_id:
        ds_id = args.dataset_id
        print(f"Reusing dataset: {ds_id}")
    else:
        # Create dataset
        headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
        payload = {
            "name": args.dataset_name,
            "description": f"Phase 1b corpus from Gemini batch synth ({MODEL}). {len(items_for_curator)} pairs across specialties.",
            "source": MODEL,
        }
        r = requests.post(f"{MIMIR_API}/training/datasets", json=payload, headers=headers, timeout=10)
        r.raise_for_status()
        ds_id = r.json()["id"]
        print(f"Created dataset: {ds_id}")

    # Bulk import in chunks of 500 (server side handles large)
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    chunk = 500
    imported = 0
    for i in range(0, len(items_for_curator), chunk):
        slc = items_for_curator[i:i + chunk]
        r = requests.post(
            f"{MIMIR_API}/training/datasets/{ds_id}/items",
            json={"items": slc}, headers=headers, timeout=60,
        )
        r.raise_for_status()
        imported += r.json().get("imported", 0)
        print(f"  imported chunk {i//chunk + 1}: total={imported}")
    print(f"\n✅ Phase 1b synth complete: {imported} pairs in dataset {ds_id}")


if __name__ == "__main__":
    main()
