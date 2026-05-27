#!/usr/bin/env python3
"""Regression test for the Coats-class resolver bug (apostrophe / hyphen /
punctuation in SNOMED FSN that user typically OMITS in input). Each case has
the exact user input + an expected keyword that MUST appear in the resolved
name (case-insensitive). Pass = at least one resolved entity matches; otherwise
print the actual (false) match so the failure mode is visible."""
import json
import subprocess
import sys

# (input_text, expected_keyword_in_result_name, class)
CASES = [
    # --- BUG CLASS A: eponymous, user omits the apostrophe ---
    ("Coats disease",         "coats",        "A apostrophe-stripped"),
    ("Alzheimer disease",     "alzheimer",    "A apostrophe-stripped"),
    ("Alzheimers disease",    "alzheimer",    "A apostrophe-stripped"),
    ("Parkinson disease",     "parkinson",    "A apostrophe-stripped"),
    ("Crohn disease",         "crohn",        "A apostrophe-stripped"),
    ("Hodgkin disease",       "hodgkin",      "A apostrophe-stripped"),
    ("Huntington disease",    "huntington",   "A apostrophe-stripped"),
    ("Sezary disease",        "sezary",       "A apostrophe-stripped"),  # accent stripped too
    ("Gaucher disease",       "gaucher",      "A apostrophe-stripped"),

    # --- BUG CLASS B: hyphenated names, user uses a space instead ---
    ("Lesch Nyhan syndrome",  "lesch",        "B hyphen-as-space"),
    ("Coffin Siris syndrome", "coffin",       "B hyphen-as-space"),
    ("Kleine Levin syndrome", "kleine",       "B hyphen-as-space"),

    # --- BUG CLASS C: with proper punctuation (positive controls) ---
    ("Coats' disease",        "coats",        "C punct-preserved (positive)"),
    ("Alzheimer's disease",   "alzheimer",    "C punct-preserved (positive)"),
    ("Lesch-Nyhan syndrome",  "lesch",        "C punct-preserved (positive)"),

    # --- REGRESSION: common diseases that always worked ---
    ("diabetes",              "diabetes",     "R regression"),
    ("hypertension",          "hypertens",    "R regression"),
    ("asthma",                "asthma",       "R regression"),

    # --- THAI path: LLM normalize -> resolver (previous bug class) ---
    ("ไข้เลือดออก",            "dengue",       "T thai"),
    ("ภาวะซึมเศร้า",            "depress",      "T thai"),
    ("สิว",                    "acne",         "T thai"),
    ("โรคพาร์กินสัน",          "parkinson",    "T thai"),

    # --- ACRONYM: expand expected via LLM/dict ---
    ("T2DM",                   "diabetes",    "X acronym"),
    ("COPD",                   "chronic obstructive", "X acronym"),
]

POD = sys.argv[1] if len(sys.argv) > 1 else "deploy/mimir-api"
TENANT = "asgard_medical"


def resolve(text):
    body = json.dumps({"text": text}, ensure_ascii=False)
    cmd = [
        "kubectl", "-n", "asgard", "exec", "-i", POD, "--",
        "sh", "-c",
        f"curl -s -m 20 http://localhost:8080/api/v1/knowledge/primekg/resolve "
        f"-H 'Content-Type: application/json' -H 'X-Tenant-Id: {TENANT}' --data-binary @-"
    ]
    r = subprocess.run(cmd, input=body.encode("utf-8"), capture_output=True, timeout=30)
    try:
        return json.loads(r.stdout.decode("utf-8") or "{}")
    except Exception:
        return {"error": r.stdout[:200].decode("utf-8", "replace")}


passes = 0
fails = 0
by_class = {}

print(f"{'class':<32}{'input':<30}{'expected':<20}{'got':<40}result")
print("-" * 130)

for text, kw, klass in CASES:
    res = resolve(text)
    entities = res.get("resolved", [])
    names = [e.get("name", "") for e in entities]
    grouped = [e.get("grouped_name", "") for e in entities]
    snomed = res.get("snomed_fsn", "") or ""
    got = names[0] if names else "(none)"
    # STRICT: top result must contain the keyword (in name OR grouped_name)
    top_text = (names[0] if names else "") + " " + (grouped[0] if grouped else "")
    hit = bool(names) and kw.lower() in top_text.lower()
    ok = "✅" if hit else "❌"
    print(f"{klass:<32}{text:<30}{kw:<20}{got[:38]:<40}{ok}")
    if not hit:
        print(f"{'':32}{'':30}{'':20}↳ snomed_fsn={snomed!r}, top_grouped={grouped[0] if grouped else None!r}, all={names[:3]}")
    by_class.setdefault(klass, [0, 0])
    if hit:
        by_class[klass][0] += 1
        passes += 1
    else:
        by_class[klass][1] += 1
        fails += 1

print("-" * 130)
print(f"\nby class (pass/total):")
for klass, (p, f) in sorted(by_class.items()):
    total = p + f
    print(f"  {klass:<32} {p}/{total}")
print(f"\nTOTAL: {passes}/{passes+fails} passed  ({fails} failed)")
sys.exit(0 if fails == 0 else 1)
