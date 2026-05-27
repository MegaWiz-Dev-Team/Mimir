#!/usr/bin/env python3
"""Coverage test for the 6 PrimeKG endpoints not exercised by the bug-class +
disease_relations suites: /entity, /neighbors, /drug_interactions,
/disease_drugs, /symptom_to_disease, /path."""
import json, subprocess, sys

POD = sys.argv[1] if len(sys.argv) > 1 else "deploy/mimir-api"
BASE = "http://localhost:8080/api/v1/knowledge/primekg"


def call(path, body):
    cmd = ["kubectl", "-n", "asgard", "exec", "-i", POD, "--", "sh", "-c",
           f"curl -s -m 25 {BASE}{path} -H 'Content-Type: application/json' "
           f"-H 'X-Tenant-Id: asgard_medical' --data-binary @-"]
    r = subprocess.run(cmd, input=json.dumps(body, ensure_ascii=False).encode(),
                       capture_output=True, timeout=40)
    try:
        return json.loads(r.stdout.decode("utf-8") or "{}")
    except Exception:
        return {"error": (r.stdout.decode("utf-8", "replace")[:200] if r.stdout else "(empty)")}


def lookup_one(name, type_filter=None):
    body = {"name": name, "limit": 1}
    if type_filter:
        body["type"] = type_filter
    items = call("/entity", body).get("items", [])
    return items[0] if items else None


passes = fails = 0


def case(g, label, ok, detail=""):
    global passes, fails
    print(f"  [{('PASS' if ok else 'FAIL')}] {g:<3} {label:<48} {detail}")
    if ok:
        passes += 1
    else:
        fails += 1


# A. /entity
print("\n====== A. /entity ======")
for name, kw in [("diabetes", "diabetes"), ("Coats disease", "coats"),
                 ("metformin", "metformin"), ("hypertension", "hypertens"),
                 ("xyznonexistentdz", None)]:
    items = call("/entity", {"name": name, "limit": 3}).get("items", [])
    if kw is None:
        case("A", f"'{name}' negative", len(items) == 0, f"count={len(items)}")
    else:
        top = (items[0].get("name", "") if items else "").lower()
        case("A", f"'{name}'", len(items) > 0 and kw in top, f"top={top[:38]!r}")

DM = lookup_one("diabetes mellitus", "disease")
HTN = lookup_one("hypertension", "disease")
ASTH = lookup_one("asthma", "disease")
MET = lookup_one("metformin", "drug")
WARF = lookup_one("warfarin", "drug")

# B. /neighbors
print("\n====== B. /neighbors ======")
if DM:
    n1 = call("/neighbors", {"entity_index": DM["entity_index"], "hops": 1, "limit": 25})
    case("B", "DM 1-hop > 0", n1.get("count", 0) > 0, f"count={n1.get('count')}")
    n2 = call("/neighbors", {"entity_index": DM["entity_index"], "hops": 2, "limit": 25})
    case("B", "DM 2-hop >= 1-hop", n2.get("count", 0) >= n1.get("count", 0),
         f"2hop={n2.get('count')} 1hop={n1.get('count')}")
    # Cypher rel types are stored UPPERCASE in PrimeKG; pass them that way.
    nf = call("/neighbors", {"entity_index": DM["entity_index"],
                              "relation_types": ["INDICATION"], "limit": 25})
    # `indication` edges connect a drug to a disease, so counterparts are drugs.
    # PrimeKG drug nodes use DrugBank ids (DB...) — type field isn't always set.
    is_drugish = lambda n: (n.get("type") == "drug"
                             or str(n.get("entity_id", "")).startswith("DB"))
    all_drugs = (nf.get("count", 0) > 0
                 and all(is_drugish(i) for i in nf.get("items", [])))
    case("B", "DM filter=INDICATION -> drugs only", all_drugs, f"count={nf.get('count')}")
else:
    case("B", "DM seed missing", False, "")

# C. /drug_interactions
print("\n====== C. /drug_interactions ======")
for drug, label in [(WARF, "warfarin"), (MET, "metformin")]:
    if drug:
        r = call("/drug_interactions", {"drug_index": drug["entity_index"], "limit": 25})
        items = r.get("items", [])
        case("C", f"{label} > 0", r.get("count", 0) > 0, f"count={r.get('count')}")
        # Drug-drug edges in PrimeKG → counterparts identified by DrugBank id (DB...)
        all_drugs = bool(items) and all(str(i.get("entity_id", "")).startswith("DB")
                                         for i in items)
        case("C", f"{label} all counterparts DrugBank ids", all_drugs, "")
        case("C", f"{label} severity_filter_supported=false",
             r.get("severity_filter_supported") is False, "")
    else:
        case("C", f"{label} seed missing", False, "")

# D. /disease_drugs
print("\n====== D. /disease_drugs ======")
for d, label in [(HTN, "hypertension"), (DM, "diabetes mellitus"), (ASTH, "asthma")]:
    if d:
        r = call("/disease_drugs", {"disease_index": d["entity_index"], "limit_per_relation": 5})
        groups = r if isinstance(r, dict) and "error" not in r else {}
        non_empty = {k: v for k, v in groups.items() if isinstance(v, list) and v}
        case("D", f"{label} >=1 non-empty group", len(non_empty) > 0,
             f"groups={list(non_empty.keys())[:3]}")
        # Items in indication/contraindication/off_label_use groups are drugs
        # identified by DrugBank id (DB…). `type` field isn't set on these.
        has_drug = any(any(str(it.get("entity_id", "")).startswith("DB") for it in v)
                       for v in non_empty.values())
        case("D", f"{label} groups contain DrugBank ids", has_drug, "")
    else:
        case("D", f"{label} seed missing", False, "")

# E. /symptom_to_disease
print("\n====== E. /symptom_to_disease ======")
for phens, label in [(["Fever"], "Fever"), (["Headache"], "Headache"),
                     (["Fever", "Cough"], "Fever+Cough min=2")]:
    body = {"phenotype_names": phens, "min_match": len(phens), "limit": 10}
    r = call("/symptom_to_disease", body)
    top = (r.get("items") or [{}])[0].get("name", "")[:35]
    case("E", label, r.get("count", 0) > 0, f"count={r.get('count')} top={top!r}")
r = call("/symptom_to_disease", {"phenotype_names": [], "limit": 5})
case("E", "empty phenotype_names -> error", "error" in r, str(r)[:60])

# F. /path
print("\n====== F. /path ======")
if MET and DM:
    r = call("/path", {"from_index": MET["entity_index"], "to_index": DM["entity_index"],
                       "max_hops": 3, "limit_paths": 3})
    case("F", "metformin -> diabetes path", r.get("count", 0) > 0,
         f"count={r.get('count')}")
if WARF and HTN:
    r = call("/path", {"from_index": WARF["entity_index"], "to_index": HTN["entity_index"],
                       "max_hops": 4, "limit_paths": 3})
    # warfarin <-> HTN may legitimately not connect; endpoint must just respond cleanly
    case("F", "warfarin -> HTN responds cleanly", "error" not in r,
         f"count={r.get('count', 'n/a')}")

print(f"\nTOTAL: {passes}/{passes+fails} passed  ({fails} failed)")
sys.exit(0 if fails == 0 else 1)
