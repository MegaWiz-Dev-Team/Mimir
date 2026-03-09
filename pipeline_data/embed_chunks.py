#!/usr/bin/env python3
"""Track A: Embed source chunks into Qdrant via bge-m3 (stdlib only)."""

import json
import time
import urllib.request
import urllib.error

MIMIR_API     = "http://localhost:3000"
QDRANT_API    = "http://localhost:6333"
EMBEDDING_API = "http://localhost:8001"
COLLECTION    = "source_chunks"
DIM           = 1024
BATCH         = 10
USERNAME      = "megacare"
PASSWORD      = "admin123"
TID           = "127d37ee-2de2-4094-8993-f7cff046c0ec"


def api(url, data=None, headers=None, method=None):
    headers = headers or {}
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    else:
        body = None
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read())


def main():
    t0 = time.time()
    print("═══ Track A: Chunk Embedding Pipeline ═══\n")

    # 1. Login
    print("1️⃣  Login...")
    token = api(f"{MIMIR_API}/api/v1/auth/login",
                {"username": USERNAME, "password": PASSWORD})["token"]
    auth = {"Authorization": f"Bearer {token}", "X-Tenant-Id": TID}
    print("  ✅ OK")

    # 2. Fetch all chunks (paginated)
    print("2️⃣  Fetching chunks...")
    chunks, page = [], 1
    while True:
        d = api(f"{MIMIR_API}/api/v1/chunks?page={page}&limit=100", headers=auth)
        batch = d.get("chunks", [])
        chunks.extend(batch)
        total = d.get("total", 0)
        print(f"  Page {page}: {len(batch)} (total {len(chunks)}/{total})")
        if len(chunks) >= total or not batch:
            break
        page += 1
    print(f"  ✅ {len(chunks)} chunks")

    if not chunks:
        print("  ⚠️ No chunks!"); return

    # 3. Qdrant collection
    print("3️⃣  Qdrant collection...")
    try:
        info = api(f"{QDRANT_API}/collections/{COLLECTION}")
        cnt = info.get("result", {}).get("points_count", 0)
        print(f"  ✅ Exists ({cnt} pts)")
    except urllib.error.HTTPError:
        api(f"{QDRANT_API}/collections/{COLLECTION}",
            {"vectors": {"size": DIM, "distance": "Cosine"}}, method="PUT")
        print("  ✅ Created")

    # 4. Embed & upsert
    print(f"4️⃣  Embedding... (batch={BATCH})")
    done = 0
    nbatch = (len(chunks) + BATCH - 1) // BATCH

    for i in range(0, len(chunks), BATCH):
        b = chunks[i:i+BATCH]
        texts = [c.get("content", "")[:8000] for c in b]

        # Embed
        emb_resp = api(f"{EMBEDDING_API}/v1/embeddings",
                       {"model": "BAAI/bge-m3", "input": texts})
        embeddings = [x["embedding"] for x in emb_resp["data"]]

        # Build points
        points = []
        for c, vec in zip(b, embeddings):
            points.append({
                "id": abs(c["id"]),
                "vector": vec,
                "payload": {
                    "content": c.get("content", ""),
                    "source_id": c.get("source_id"),
                    "source_name": c.get("source_name", ""),
                    "chunk_order": c.get("chunk_order", 0),
                    "tenant_id": TID
                }
            })

        api(f"{QDRANT_API}/collections/{COLLECTION}/points",
            {"points": points}, method="PUT")
        done += len(b)
        bn = i // BATCH + 1
        print(f"  [{bn}/{nbatch}] {done}/{len(chunks)} ({done*100//len(chunks)}%)")

    elapsed = time.time() - t0
    print(f"  ✅ Done: {done} chunks in {elapsed:.1f}s")

    # 5. Verify
    print("5️⃣  Verify...")
    info = api(f"{QDRANT_API}/collections/{COLLECTION}")
    pts = info.get("result", {}).get("points_count", 0)
    print(f"  ✅ Collection: {pts} points")

    # 6. Test search
    print("6️⃣  Test search: 'sleep apnea CPAP treatment'")
    tv = api(f"{EMBEDDING_API}/v1/embeddings",
             {"model": "BAAI/bge-m3", "input": ["sleep apnea CPAP treatment"]})
    qvec = tv["data"][0]["embedding"]

    results = api(f"{QDRANT_API}/collections/{COLLECTION}/points/search",
                  {"vector": qvec, "limit": 3, "with_payload": True,
                   "filter": {"must": [{"key": "tenant_id", "match": {"value": TID}}]}})
    for j, r in enumerate(results.get("result", [])):
        s = r.get("score", 0)
        p = r.get("payload", {})
        print(f"  {j+1}. [{s:.4f}] {p.get('source_name','?')}")
        print(f"     {p.get('content','')[:80]}...")

    print(f"\n═══ Track A Complete ({elapsed:.1f}s) ═══")

if __name__ == "__main__":
    main()
