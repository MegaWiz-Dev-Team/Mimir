#!/usr/bin/env python3
"""
Sprint 39 Phase 1a MVP — synthesize medical Q-A pairs via local gemma-4-26b.

Generates 100-200 pairs across 8 specialties (cardio/endo/ent/peds/emergency/
psych/pharmacy/general-medicine) and bulk-imports them to Mimir Curator.

Run AFTER: champion holds at gemma-4-26b, no eval competing for MLX cycles.
Cost: $0 (local model). Time: ~15-25 min.
"""

from __future__ import annotations
import argparse
import json
import os
import sys
import time
from dataclasses import dataclass

import requests

HEIMDALL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1")
HEIMDALL_KEY = os.environ.get(
    "HEIMDALL_API_KEY",
    "hml-REDACTED",
)
MIMIR_API = os.environ.get("MIMIR_API_URL", "http://localhost:30000/api/v1")
TENANT = os.environ.get("MIMIR_TENANT", "asgard_medical")
MODEL = os.environ.get("SYNTH_MODEL", "mlx-community/gemma-4-26b-a4b-it-4bit")


@dataclass
class TopicSeed:
    specialty: str
    topic: str
    n_pairs: int  # how many pairs to ask gemma to generate
    suggested_tags: list[str]


# ─── Topic seeds ─────────────────────────────────────────────────────────────
SEEDS: list[TopicSeed] = [
    # Cardiology
    TopicSeed("cardiology", "atrial fibrillation management (rate vs rhythm, anticoagulation)", 5, ["pharmacy", "anticoagulation"]),
    TopicSeed("cardiology", "heart failure with reduced ejection fraction (HFrEF) — guideline-directed therapy", 5, ["pharmacy", "monitoring"]),
    TopicSeed("cardiology", "acute coronary syndromes (STEMI, NSTEMI, unstable angina)", 5, ["emergency", "urgent", "imaging"]),
    TopicSeed("cardiology", "hypertensive emergency vs urgency", 4, ["emergency", "pharmacy"]),
    # Endocrinology
    TopicSeed("endocrinology", "type 2 diabetes mellitus — pharmacotherapy progression and complications", 5, ["pharmacy", "monitoring"]),
    TopicSeed("endocrinology", "thyroid disorders (hyper, hypo, nodules)", 4, ["lab-interpretation"]),
    TopicSeed("endocrinology", "adrenal insufficiency and Cushing syndrome", 4, ["differential-dx", "pharmacy"]),
    # ENT
    TopicSeed("ent", "acute and chronic otitis media (adult + pediatric)", 4, ["pediatric", "antibiotic"]),
    TopicSeed("ent", "allergic and non-allergic rhinitis", 3, ["pharmacy", "outpatient"]),
    TopicSeed("ent", "sinusitis (acute bacterial vs viral, chronic)", 3, ["antibiotic", "differential-dx"]),
    # Pediatrics
    TopicSeed("pediatrics", "pediatric asthma — diagnosis, severity classification, controller therapy", 4, ["pediatric", "pharmacy", "dosing"]),
    TopicSeed("pediatrics", "common pediatric infections (UTI, pneumonia, bronchiolitis)", 4, ["pediatric", "antibiotic", "dosing"]),
    TopicSeed("pediatrics", "growth and developmental milestones, failure to thrive", 3, ["pediatric", "screening"]),
    # Emergency medicine
    TopicSeed("emergency-medicine", "sepsis recognition and early management (Sepsis-3, qSOFA, bundle)", 4, ["urgent", "icu", "antibiotic"]),
    TopicSeed("emergency-medicine", "acute stroke evaluation (last-known-well, imaging, tPA window)", 4, ["urgent", "imaging", "red-flag"]),
    TopicSeed("emergency-medicine", "anaphylaxis and severe allergic reactions", 3, ["pharmacy", "urgent", "allergy"]),
    # Psychiatry
    TopicSeed("psychiatry", "major depressive disorder diagnosis + first-line pharmacotherapy", 4, ["pharmacy", "screening", "outpatient"]),
    TopicSeed("psychiatry", "anxiety disorders (GAD, panic, PTSD) — diagnosis and treatment", 3, ["pharmacy", "outpatient"]),
    TopicSeed("psychiatry", "bipolar disorder — mania vs hypomania, mood stabilizers", 3, ["pharmacy", "monitoring"]),
    TopicSeed("psychiatry", "suicide risk assessment and acute management", 3, ["urgent", "red-flag", "screening"]),
    # Pharmacy
    TopicSeed("pharmacy", "drug-drug interactions in chronic disease (warfarin, polypharmacy)", 4, ["interaction", "anticoagulation", "geriatric"]),
    TopicSeed("pharmacy", "renal and hepatic dose adjustments", 4, ["dosing", "nephrology", "monitoring"]),
    TopicSeed("pharmacy", "medication safety in pregnancy and breastfeeding", 3, ["pregnancy", "obgyn", "contraindication"]),
    # General medicine
    TopicSeed("general-medicine", "COPD — diagnosis, GOLD classification, and stepwise pharmacotherapy", 4, ["pharmacy", "monitoring", "outpatient"]),
    TopicSeed("general-medicine", "GERD and peptic ulcer disease", 3, ["pharmacy", "outpatient"]),
    TopicSeed("general-medicine", "anemia workup (microcytic, macrocytic, normocytic)", 3, ["differential-dx", "lab-interpretation"]),
    TopicSeed("general-medicine", "chronic kidney disease — staging, monitoring, complications", 4, ["nephrology", "monitoring", "pharmacy"]),
]


def synth_prompt(specialty: str, topic: str, n_pairs: int) -> str:
    """Build synthesis prompt for batched Q-A generation.

    Uses a text-delimiter format (not JSON) because thinking-mode models
    (gemma-4-26b) frequently bleed reasoning into JSON output, breaking
    parse. Regex-based delimiter parsing recovers cleanly even when the
    model adds preamble or post-amble text.
    """
    return f"""You are a medical-education content writer. Generate exactly {n_pairs} realistic clinical question-answer pairs about: **{topic}** (specialty: {specialty}).

Each QUESTION: 1-3 sentences — a realistic question a junior physician would ask a senior colleague (vignette OR direct form). Vary structure across the {n_pairs} pairs.

Each ANSWER: 3-6 sentences of medically accurate, guideline-aligned content. Include specific numbers (doses, thresholds, scoring criteria). Note when professional consultation is required.

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

Start your response with ===Q1=== . Do not add any commentary before or after."""


SYSTEM_MSG = (
    "You are a medical-education content writer. "
    "Output ONLY clinical Q-A pairs in the requested delimiter format. "
    "Do NOT echo the requirements. Do NOT add preamble. Start directly with ===Q1===. "
    "No bullet-point summaries of the task. Just the pairs."
)


def call_gemma(prompt: str, timeout: int = 240) -> str | None:
    """Call gemma via Heimdall. Returns content text or None on error.

    Strategy for gemma-4-26b thinking-mode:
      1. System message tells the model NOT to echo task requirements
      2. Assistant prefill seeds the response with "===Q1===\\n" so the model
         starts producing the question immediately rather than meta-talk
      3. Concatenate prefill + completion when reading
      4. Larger max_tokens (4096) leaves room even if model still does some
         thinking. Prefer reasoning field as fallback if content empty.
    """
    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {HEIMDALL_KEY}"}
    prefill = "===Q1===\n"
    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": SYSTEM_MSG},
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": prefill},
        ],
        "temperature": 0.6,
        "max_tokens": 4096,
    }
    try:
        r = requests.post(f"{HEIMDALL}/chat/completions", json=payload, headers=headers, timeout=timeout)
        r.raise_for_status()
        data = r.json()
        msg = data["choices"][0]["message"]
        content = (msg.get("content") or "").strip()
        if not content:
            content = (msg.get("reasoning") or "").strip()
        if not content:
            return None
        # If the response doesn't already start with the prefill, prepend it
        # so the parser finds ===Q1=== anchor.
        if "===Q1===" not in content:
            content = prefill + content
        return content
    except Exception as e:
        print(f"  ✗ gemma call error: {e}", file=sys.stderr)
        return None


_QA_RE = __import__("re").compile(
    r"===Q(\d+)===\s*(.*?)\s*===A\1===\s*(.*?)(?=\s*===Q\d+===|\s*$)",
    __import__("re").DOTALL,
)


def parse_pairs(text: str) -> list[dict]:
    """Extract Q-A pairs from gemma's response using ===Q1===/===A1=== delimiters.

    Robust against:
      - Preamble/post-amble (regex anchors find Q/A blocks anywhere)
      - Thinking-mode bleed (model meta-talk before pairs is ignored)
      - Markdown fences (we don't care about them — delimiters dominate)
      - Mismatched Q/A indices (we use \\1 backreference for tight pairing)
    """
    if not text:
        return []
    valid = []
    for match in _QA_RE.finditer(text):
        q = match.group(2).strip()
        a = match.group(3).strip()
        # Strip any trailing fence/punctuation noise
        q = q.strip("`").strip()
        a = a.strip("`").strip()
        if q and a and len(q) > 20 and len(a) > 50:
            valid.append({"question": q, "ai_answer": a})
    if not valid:
        print(f"  ✗ no valid pairs parsed", file=sys.stderr)
        print(f"  raw (first 300): {text[:300]}", file=sys.stderr)
    return valid


def import_items(dataset_id: str, items: list[dict]) -> bool:
    """Bulk-import items to Curator dataset."""
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    payload = {"items": items}
    try:
        r = requests.post(
            f"{MIMIR_API}/training/datasets/{dataset_id}/items",
            json=payload,
            headers=headers,
            timeout=30,
        )
        r.raise_for_status()
        result = r.json()
        return result.get("imported", 0) > 0
    except Exception as e:
        print(f"  ✗ import error: {e}", file=sys.stderr)
        return False


def create_dataset(name: str, description: str) -> str:
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    payload = {"name": name, "description": description, "source": MODEL}
    r = requests.post(f"{MIMIR_API}/training/datasets", json=payload, headers=headers, timeout=10)
    r.raise_for_status()
    return r.json()["id"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset-name", default="Sprint 39 MVP corpus v2 (gemma-synth)")
    ap.add_argument("--dataset-id", help="Reuse an existing dataset_id (skip create)")
    ap.add_argument("--max-pairs", type=int, default=200)
    ap.add_argument("--save-only", action="store_true", help="Save to JSONL but don't import")
    ap.add_argument("--out", default="/tmp/mvp_synth_pairs.jsonl")
    args = ap.parse_args()

    if args.dataset_id:
        ds_id = args.dataset_id
        print(f"Reusing dataset: {ds_id}")
    else:
        ds_id = create_dataset(
            args.dataset_name,
            "MVP corpus synthesized via local gemma-4-26b. Sprint 39 Phase 1a — validates pipeline before paid Gemini synthesis (Phase 1b).",
        )
        print(f"Created dataset: {ds_id}")

    out = open(args.out, "a")
    accumulated = 0
    seed_idx = 0
    fail_streak = 0

    while accumulated < args.max_pairs and seed_idx < len(SEEDS):
        seed = SEEDS[seed_idx]
        seed_idx += 1
        n = min(seed.n_pairs, args.max_pairs - accumulated)
        prompt = synth_prompt(seed.specialty, seed.topic, n)
        t0 = time.time()
        print(f"[{seed_idx:02d}/{len(SEEDS)}] {seed.specialty:20s} | {seed.topic[:50]:50s} | n={n} ... ", end="", flush=True)
        text = call_gemma(prompt)
        elapsed = time.time() - t0
        if not text:
            print(f"FAIL ({elapsed:.0f}s)")
            fail_streak += 1
            if fail_streak >= 3:
                print("Too many consecutive failures — stopping.")
                break
            continue
        pairs = parse_pairs(text)
        if not pairs:
            print(f"PARSE-FAIL ({elapsed:.0f}s)")
            fail_streak += 1
            continue
        fail_streak = 0
        # Tag with specialty + suggested cross-cutting tags
        items = [
            {
                "question": p["question"],
                "ai_answer": p["ai_answer"],
                "specialty": seed.specialty,
                "tags": seed.suggested_tags,
            }
            for p in pairs
        ]
        # Save to JSONL (always)
        for it in items:
            out.write(json.dumps(it) + "\n")
        out.flush()
        # Import (unless --save-only)
        if not args.save_only:
            ok = import_items(ds_id, items)
            print(f"OK ({elapsed:.0f}s) parsed={len(pairs)} imported={'✓' if ok else '✗'}")
        else:
            print(f"OK ({elapsed:.0f}s) parsed={len(pairs)} (save-only)")
        accumulated += len(pairs)

    out.close()
    print(f"\n{'═' * 60}")
    print(f"  Total pairs accumulated: {accumulated}")
    print(f"  Dataset id: {ds_id}")
    print(f"  Saved JSONL: {args.out}")
    print(f"{'═' * 60}")


if __name__ == "__main__":
    main()
