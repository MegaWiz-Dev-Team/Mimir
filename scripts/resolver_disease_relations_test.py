#!/usr/bin/env python3
"""Companion to resolver_bug_class_test.py — hits /disease_relations (the path
the Medical Knowledge Assistant actually uses via Bifrost MCP), which has its
OWN code path (llm_extract_disease → primekg_lookup_entity, NOT /resolve). Each
case must return found=True with the resolved name containing the expected
keyword."""
import json, subprocess, sys

CASES = [
    # eponymous WITH apostrophe (user types it; PrimeKG stores it WITHOUT)
    ("Coats' disease",        "coats"),
    ("Alzheimer's disease",   "alzheimer"),
    ("Parkinson's disease",   "parkinson"),
    ("Crohn's disease",       "crohn"),
    ("Huntington's disease",  "huntington"),
    # eponymous without apostrophe (the original Coats class)
    ("Coats disease",         "coats"),
    ("Alzheimer disease",     "alzheimer"),
    # typo with trailing s
    ("Alzheimers disease",    "alzheimer"),
    # hyphen-as-space
    ("Lesch Nyhan syndrome",  "lesch"),
    ("Coffin Siris syndrome", "coffin"),
    # regression — common diseases
    ("diabetes",              "diabetes"),
    ("hypertension",          "hypertens"),
    ("asthma",                "asthma"),
    # Thai routed via LLM extract
    ("ภาวะซึมเศร้า",          "depress"),
    ("โรคพาร์กินสัน",         "parkinson"),
]

POD = sys.argv[1] if len(sys.argv) > 1 else "deploy/mimir-api"


def disease_relations(query):
    body = json.dumps({"query": query}, ensure_ascii=False)
    cmd = [
        "kubectl", "-n", "asgard", "exec", "-i", POD, "--", "sh", "-c",
        "curl -s -m 25 http://localhost:8080/api/v1/knowledge/primekg/disease_relations "
        "-H 'Content-Type: application/json' -H 'X-Tenant-Id: asgard_medical' --data-binary @-"
    ]
    r = subprocess.run(cmd, input=body.encode("utf-8"), capture_output=True, timeout=40)
    try:
        return json.loads(r.stdout.decode("utf-8") or "{}")
    except Exception:
        return {"error": r.stdout[:200].decode("utf-8", "replace")}


passes = fails = 0
print(f"{'input':<28}{'expected':<14}{'resolved_disease':<28}{'seed.name':<32}{'found':<7}result")
print("-" * 120)

for query, kw in CASES:
    res = disease_relations(query)
    resolved = res.get("resolved_disease", "") or ""
    found = res.get("found", False)
    seed = (res.get("seed") or {}).get("name", "") or ""
    text = (resolved + " " + seed).lower()
    hit = found and kw.lower() in text
    ok = "✅" if hit else "❌"
    print(f"{query:<28}{kw:<14}{resolved[:26]:<28}{seed[:30]:<32}{str(found):<7}{ok}")
    if not hit:
        print(f"  ↳ note={res.get('note')!r}  count={res.get('count')}")
    if hit:
        passes += 1
    else:
        fails += 1

print("-" * 120)
print(f"\nTOTAL: {passes}/{passes+fails} passed  ({fails} failed)")
sys.exit(0 if fails == 0 else 1)
