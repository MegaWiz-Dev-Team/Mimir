#!/usr/bin/env python3 -u
"""Track B+C: KG + QA via Gemini Flash (unbuffered, with retry)."""

import json, time, urllib.request, urllib.error, sys, os

sys.stdout.reconfigure(line_buffering=True)  # Force line buffering

MIMIR     = "http://localhost:3000"
QDRANT    = "http://localhost:6333"
GEMINI_KEY = os.environ.get("GEMINI_API_KEY", "")
if not GEMINI_KEY:
    sys.stderr.write("Error: GEMINI_API_KEY environment variable not set.\n")
    sys.exit(1)
GEMINI_URL = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={GEMINI_KEY}"
TID       = "127d37ee-2de2-4094-8993-f7cff046c0ec"

def api(url, data=None, headers=None, method=None):
    headers = headers or {}
    body = json.dumps(data).encode() if data else None
    if body: headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())

def gemini_call(prompt, system, max_retries=3):
    """Call Gemini with retry."""
    contents = [
        {"role": "user", "parts": [{"text": system}]},
        {"role": "model", "parts": [{"text": "OK."}]},
        {"role": "user", "parts": [{"text": prompt}]}
    ]
    data = {"contents": contents, "generationConfig": {"maxOutputTokens": 1024, "temperature": 0.1}}
    
    for attempt in range(max_retries):
        try:
            r = api(GEMINI_URL, data)
            if "candidates" in r:
                return r["candidates"][0]["content"]["parts"][0]["text"]
            return None
        except Exception as e:
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt)
            else:
                return None

def parse_json(text):
    if not text: return None
    text = text.strip()
    if text.startswith("```"):
        lines = text.split("\n")[1:]
        if lines and lines[-1].strip() == "```": lines = lines[:-1]
        text = "\n".join(lines)
    try: return json.loads(text)
    except:
        for ch in ["{", "["]:
            i = text.find(ch)
            if i >= 0:
                try: return json.loads(text[i:])
                except: pass
    return None

KG_SYS = """Extract medical entities and relations as JSON:
{"entities":[{"name":"...","type":"disease|symptom|drug|treatment|device|anatomy"}],
 "relations":[{"from":"...","to":"...","type":"treats|causes|symptom_of|used_for|diagnoses"}]}
Max 15 entities, 15 relations. Concise names."""

QA_SYS = """Generate 3-5 medical Q&A pairs as JSON array:
[{"question":"...","answer":"..."}]
Match the source language. Focus on clinical knowledge."""

def main():
    t0 = time.time()
    print("═══ Track B+C: KG + QA Pipeline ═══")
    print(f"Model: gemini-2.0-flash")

    # Login
    token = api(f"{MIMIR}/api/v1/auth/login", {"username":"megacare","password":"admin123"})["token"]
    auth = {"Authorization": f"Bearer {token}", "X-Tenant-Id": TID}
    print("✅ Logged in")

    # Fetch chunks
    chunks, page = [], 1
    while True:
        d = api(f"{MIMIR}/api/v1/chunks?page={page}&limit=100", headers=auth)
        chunks.extend(d.get("chunks", []))
        if len(chunks) >= d.get("total", 0) or not d.get("chunks"): break
        page += 1
    print(f"✅ {len(chunks)} chunks fetched")

    # Process
    stats = {"entities": 0, "relations": 0, "qa": 0, "errors": 0}
    all_kg = []
    all_qa = []

    for i, c in enumerate(chunks):
        content = c.get("content", "")
        if len(content.strip()) < 30: continue
        sname = c.get("source_name", "")
        prompt = f"Source: {sname}\n\n{content[:2500]}"

        # KG
        kg = parse_json(gemini_call(prompt, KG_SYS))
        if kg:
            ents = kg.get("entities", [])
            rels = kg.get("relations", [])
            stats["entities"] += len(ents)
            stats["relations"] += len(rels)
            all_kg.append({"chunk_id": c["id"], "source_id": c.get("source_id"), "entities": ents, "relations": rels})
        else:
            stats["errors"] += 1

        # QA
        qa = parse_json(gemini_call(prompt, QA_SYS))
        if qa and isinstance(qa, list):
            stats["qa"] += len(qa)
            all_qa.append({"chunk_id": c["id"], "source_id": c.get("source_id"), "qa": qa})
        else:
            stats["errors"] += 1

        elapsed = time.time() - t0
        eta = elapsed / (i+1) * (len(chunks) - i - 1)
        print(f"[{i+1}/{len(chunks)}] E={stats['entities']} R={stats['relations']} Q={stats['qa']} err={stats['errors']} ETA={eta:.0f}s")

        time.sleep(0.3)  # Rate limit

    print(f"\n✅ Extraction done in {time.time()-t0:.0f}s")
    print(f"   Entities: {stats['entities']}, Relations: {stats['relations']}, QA: {stats['qa']}")

    # Save results
    with open("/tmp/pipeline_results.json", "w") as f:
        json.dump({"kg": all_kg, "qa": all_qa, "stats": stats}, f, ensure_ascii=False, indent=2)
    print(f"📁 Saved to /tmp/pipeline_results.json")

    # Store in MariaDB via direct SQL query API or save for later bulk import
    print(f"\n═══ Complete ({time.time()-t0:.0f}s) ═══")

if __name__ == "__main__":
    main()
