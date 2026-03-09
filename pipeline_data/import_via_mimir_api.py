#!/usr/bin/env python3 -u
"""Import KG data from pipeline_results.json via Mimir bulk API."""

import json, time, urllib.request, urllib.error, sys

sys.stdout.reconfigure(line_buffering=True)

MIMIR = "http://localhost:3000"
TID   = "127d37ee-2de2-4094-8993-f7cff046c0ec"

def api(url, data=None, headers=None, method=None):
    headers = headers or {}
    body = json.dumps(data).encode() if data else None
    if body: headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        err = e.read().decode()[:300]
        return {"error": f"HTTP {e.code}: {err}"}
    except Exception as e:
        return {"error": str(e)}

def main():
    t0 = time.time()
    print("═══ Import KG via Mimir API ═══\n")

    # Login
    token = api(f"{MIMIR}/api/v1/auth/login", {"username":"megacare","password":"admin123"})["token"]
    auth = {"Authorization": f"Bearer {token}", "X-Tenant-Id": TID}
    print("✅ Logged in")

    # Load pipeline results
    with open("/Users/mimir/Developer/Mimir/pipeline_data/pipeline_results.json") as f:
        data = json.load(f)
    kg_data = data.get("kg", [])
    qa_data = data.get("qa", [])
    print(f"📁 Loaded: {len(kg_data)} KG chunks, {len(qa_data)} QA chunks")

    # Check graph stats before
    stats = api(f"{MIMIR}/api/v1/graph/stats", headers=auth)
    print(f"\n📊 Before: entities={stats.get('total_entities',0)}, relations={stats.get('total_relations',0)}")

    # Import entities per source_id
    print("\n1️⃣  Importing entities via POST /graph/entities/bulk...")
    by_source = {}
    for item in kg_data:
        sid = item.get("source_id")
        if sid not in by_source:
            by_source[sid] = {"entities": [], "relations": []}
        for ent in item.get("entities", []):
            by_source[sid]["entities"].append({
                "name": ent["name"],
                "entity_type": ent.get("type", "concept"),
                "chunk_id": item.get("chunk_id"),
            })
        for rel in item.get("relations", []):
            by_source[sid]["relations"].append({
                "from_entity": rel.get("from", ""),
                "to_entity": rel.get("to", ""),
                "relation_type": rel.get("type", "related_to"),
            })

    total_ent_inserted = 0
    total_ent_skipped = 0
    total_rel_inserted = 0
    total_rel_skipped = 0

    for sid, items in sorted(by_source.items()):
        # Entities
        r = api(f"{MIMIR}/api/v1/graph/entities/bulk",
                {"entities": items["entities"], "source_id": sid},
                headers=auth, method="POST")
        ins = r.get("inserted", 0)
        skip = r.get("skipped", 0)
        total_ent_inserted += ins
        total_ent_skipped += skip
        print(f"  Source #{sid}: {ins} entities inserted, {skip} skipped")

    print(f"  Total: {total_ent_inserted} inserted, {total_ent_skipped} skipped")

    # Import relations per source_id
    print("\n2️⃣  Importing relations via POST /graph/relations/bulk...")
    for sid, items in sorted(by_source.items()):
        r = api(f"{MIMIR}/api/v1/graph/relations/bulk",
                {"relations": items["relations"], "source_id": sid},
                headers=auth, method="POST")
        ins = r.get("inserted", 0)
        skip = r.get("skipped", 0)
        total_rel_inserted += ins
        total_rel_skipped += skip
        print(f"  Source #{sid}: {ins} relations inserted, {skip} skipped")

    print(f"  Total: {total_rel_inserted} inserted, {total_rel_skipped} skipped")

    # Check graph stats after
    stats = api(f"{MIMIR}/api/v1/graph/stats", headers=auth)
    print(f"\n📊 After: entities={stats.get('total_entities',0)}, relations={stats.get('total_relations',0)}")
    
    # Show entity types
    for et in stats.get("entities_by_type", []):
        print(f"  {et['type']}: {et['count']}")

    # Test trigger_extraction (SQL fix)
    print("\n3️⃣  Test trigger_extraction (SQL fix)...")
    r = api(f"{MIMIR}/api/v1/graph/extract",
            {"source_id": 10},
            headers=auth, method="POST")
    print(f"  status={r.get('status','?')}, run_id={r.get('run_id','?')}")

    # Test vector search (TenantContext fix)
    print("\n4️⃣  Test vector search (TenantContext fix)...")
    r = api(f"{MIMIR}/api/v1/vector/search",
            {"query": "sleep apnea CPAP treatment", "limit": 3},
            headers=auth, method="POST")
    if "error" in r:
        print(f"  ⚠️ {json.dumps(r)[:200]}")
    elif isinstance(r, list):
        print(f"  ✅ {len(r)} results")
        for i, item in enumerate(r[:3]):
            print(f"    {i+1}. score={item.get('score','?'):.4f}")
    else:
        print(f"  {json.dumps(r)[:200]}")

    # Coverage
    print("\n5️⃣  Coverage check...")
    cov = api(f"{MIMIR}/api/v1/coverage/overview", headers=auth)
    print(f"  Overall score: {cov.get('overall_score', '?')}")
    stages = cov.get("pipeline_stages", {})
    for k, v in stages.items():
        print(f"    {k}: {v}")

    # Graph visualization (quick check)
    print("\n6️⃣  Graph visualization check...")
    viz = api(f"{MIMIR}/api/v1/graph/visualization?limit=50", headers=auth)
    print(f"  Nodes: {viz.get('total_nodes',0)}, Edges: {viz.get('total_edges',0)}")

    # Graph path search
    print("\n7️⃣  Graph path search (OSA → CPAP)...")
    paths = api(f"{MIMIR}/api/v1/graph/paths?from=OSA&to=CPAP", headers=auth)
    print(f"  Found: {paths.get('found', False)}")
    for p in paths.get("paths", [])[:2]:
        steps = p.get("steps", [])
        chain = " → ".join(f"{s['from']} --[{s['relation_type']}]--> {s['to']}" for s in steps)
        print(f"    {chain}")

    elapsed = time.time() - t0
    print(f"\n═══ Done ({elapsed:.0f}s) ═══")

if __name__ == "__main__":
    main()
