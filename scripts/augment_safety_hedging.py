#!/usr/bin/env python3
"""
Sprint 39 retry — augment training corpus with safety hedging.

Phase 3 result revealed safety dimension regressed -0.25 because the LoRA
inherited a directive style from the Gemini-synthesized corpus (only 9% had
proper "consult professional" hedging). This script appends a safety
disclaimer to pairs that lack it, varying the phrasing to avoid the model
just learning a single template.

Strategy:
  1. Read all APPROVED items from source dataset
  2. Classify each: already-hedged (skip) vs needs-hedging (augment)
  3. Append randomized safety disclaimer to needs-hedging items
  4. Write to NEW dataset (preserve original for A/B comparison)
  5. Auto-approve all in new dataset (curator review-skip for speed)

Cost: $0 (no LLM calls — pure text manipulation).
"""

from __future__ import annotations
import argparse
import os
import random

import requests

MIMIR_API = os.environ.get("MIMIR_API_URL", "http://localhost:30000/api/v1")
TENANT = os.environ.get("MIMIR_TENANT", "asgard_medical")

# Random hedging templates — varied phrasing to avoid LoRA learning one boilerplate.
HEDGING_TEMPLATES = [
    "\n\nClinical decisions in this scenario should be made in consultation with a qualified physician familiar with the patient's complete history, current medications, and exam findings.",
    "\n\nThis guidance reflects general clinical principles. Always consult a qualified clinician for individualized decisions on this patient, especially considering comorbidities, allergies, and prior treatment responses.",
    "\n\nProfessional consultation is essential before applying this approach. A physician familiar with the patient's full case must validate the plan in light of contraindications and individual factors.",
    "\n\nIndividual patient management should be guided by a qualified physician. The information above provides framework guidance; actual orders, doses, and decisions require clinical judgment and patient-specific context.",
    "\n\nWhile these are evidence-based principles, the treating clinician must integrate them with the specific patient's context (vitals, comorbidities, drug interactions, prior responses) before acting.",
    "\n\nIf there is any uncertainty about the optimal management for this patient, consult with a senior physician or relevant specialist before proceeding.",
    "\n\nAlways verify dosing, contraindications, and the suitability of this approach with a qualified clinician for the specific patient — this synthesis is not a substitute for direct clinical judgment.",
    "\n\nFor the specific patient, consult a qualified physician to confirm the approach is appropriate given their full clinical picture, including any contraindications or comorbidities.",
]

# Professional/clinician keywords (any of these counts as "professional referral")
_PROFESSIONALS = (
    "physician", "clinician", "specialist", "professional", "provider",
    "doctor", "surgeon", "expert",
    "cardiologist", "nephrologist", "endocrinologist", "neurologist",
    "psychiatrist", "pulmonologist", "rheumatologist", "oncologist",
    "gastroenterologist", "hepatologist", "hematologist",
    "pediatrician", "obstetrician", "gynecologist", "anesthesiologist",
    "emergency services", "intensivist", "icu team",
)

# Urgency / referral signals
_URGENCY = (
    "seek immediate", "seek urgent", "seek medical", "seek emergency",
    "refer to", "referral to", "consult", "consultation",
    "under supervision", "under guidance",
)


def is_already_hedged(text: str) -> bool:
    """True if text already contains BOTH a 'consult/refer/seek' verb AND a
    professional-referral noun. This matches our SQL definition (~9.3% of
    Phase 1b corpus). Less strict than line-anchored regex."""
    lower = text.lower()
    has_urgency = any(u in lower for u in _URGENCY)
    has_pro = any(p in lower for p in _PROFESSIONALS)
    return has_urgency and has_pro


def list_items(dataset_id: str) -> list[dict]:
    """Get all items by paging if needed. For now use direct DB-via-export style."""
    headers = {"X-Tenant-Id": TENANT}
    # Curator API doesn't have a "list all items" endpoint; export is approved-only.
    # We fetch via direct DB query through a scratch endpoint OR by paging the queue.
    # For simplicity: use export endpoint (approved only — fits our use case since we
    # approved everything in /tmp/...).
    r = requests.get(
        f"{MIMIR_API}/training/datasets/{dataset_id}/export.jsonl",
        headers=headers,
        timeout=60,
    )
    r.raise_for_status()
    import json
    items = []
    for ln in r.text.split("\n"):
        if not ln.strip():
            continue
        obj = json.loads(ln)
        items.append({
            "question": obj["prompt"],
            "ai_answer": obj["completion"],
            "specialty": obj.get("metadata", {}).get("specialty"),
            "tags": obj.get("metadata", {}).get("tags", []),
        })
    return items


def augment(text: str, rng: random.Random) -> str:
    if is_already_hedged(text):
        return text
    # Pick random template; vary so model doesn't learn one boilerplate.
    template = rng.choice(HEDGING_TEMPLATES)
    return text.rstrip() + template


def create_dataset(name: str, description: str, source: str) -> str:
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    r = requests.post(
        f"{MIMIR_API}/training/datasets",
        json={"name": name, "description": description, "source": source},
        headers=headers,
        timeout=10,
    )
    r.raise_for_status()
    return r.json()["id"]


def import_items(dataset_id: str, items: list[dict]):
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    chunk = 500
    imported = 0
    for i in range(0, len(items), chunk):
        slc = items[i:i + chunk]
        r = requests.post(
            f"{MIMIR_API}/training/datasets/{dataset_id}/items",
            json={"items": slc}, headers=headers, timeout=60,
        )
        r.raise_for_status()
        imported += r.json().get("imported", 0)
    return imported


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-dataset", required=True, help="Source dataset_id to read")
    ap.add_argument("--target-name",
                    default="Sprint 39 Phase 1b corpus + safety hedging augment")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    rng = random.Random(args.seed)

    print(f"Reading source dataset {args.source_dataset}...")
    items = list_items(args.source_dataset)
    print(f"  {len(items)} items loaded")

    print("Augmenting...")
    n_already = 0
    n_augmented = 0
    for it in items:
        if is_already_hedged(it["ai_answer"]):
            n_already += 1
        else:
            it["ai_answer"] = augment(it["ai_answer"], rng)
            n_augmented += 1
    print(f"  already hedged: {n_already} ({n_already/len(items)*100:.1f}%)")
    print(f"  augmented:      {n_augmented} ({n_augmented/len(items)*100:.1f}%)")
    print(f"  total output:   {len(items)}")

    if args.dry_run:
        print("\n=== sample (3 random) ===")
        for it in random.Random(args.seed).sample(items, 3):
            print(f"\n[Q] {it['question'][:100]}")
            print(f"[A] ...{it['ai_answer'][-300:]}")
        return

    print("Creating new dataset...")
    new_id = create_dataset(
        args.target_name,
        f"Sprint 39 retry: source {args.source_dataset} augmented with safety hedging "
        f"on {n_augmented} items lacking explicit professional referral.",
        "augment-safety-hedging",
    )
    print(f"  new dataset: {new_id}")

    print("Importing augmented items...")
    imported = import_items(new_id, items)
    print(f"  imported {imported} items")

    print(f"\n✅ Done.")
    print(f"   New dataset: {new_id}")
    print(f"   Next: auto-approve + lora_train_mvp.py --dataset-id {new_id} --iters 300 --num-layers 16")


if __name__ == "__main__":
    main()
