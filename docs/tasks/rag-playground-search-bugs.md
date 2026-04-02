# RAG Ensemble Playground: Search Bugs Analysis & Fix Plan

**Status:** 🔴 Pending Fixes (Analysis Completed)
**Components:** Vector (Qdrant), Graph (MariaDB/sqlx), Tree (PageIndex), Indexing (Background Tasks)

---

## 1. Vector Search Silently Fails (0 Results)
**Symptom:** RAG Playground returns 0 vector results for `AHI`, while the Vector Management dashboard successfully finds 517 chunks.
**Root Cause:**
- The database `source_chunks` uses the modern **"Named Vector"** configuration (named `"dense"`, 1024 dims).
- The dashboard hits `POST /api/vector/search` which correctly utilizes the newer `qdrant.search_hybrid()` backend function formatted for named vectors.
- However, RAG Playground (`POST /api/search`) uses the legacy `QdrantRetriever::search()`. This function sends an **"Unnamed Vector"** request (a flat array).
- Qdrant rejects this request with a `400 Bad Request` (`Not existing vector name error`), causing Playground to silently log a warning and return 0 vector hits.
**Fix required in:** `ro-ai-bridge/src/retrieval/qdrant.rs`
- Modify `VectorRetriever::search` behavior to correctly wrap the payload as `{"name": "dense", "vector": [...]}`.

---

## 2. Graph Search Type Decoding Crash
**Symptom:** Graph UI shows "AHI" nodes and edges natively, but RAG Playground drops Graph results.
**Root Cause:**
- The Graph SQL query matching `AHI` works and correctly retrieves 5 entities from MariaDB.
- However, `sqlx::query_as` crashes during data mapping because the `properties` column in `kg_entities` is stored as `JSON/BLOB` type in MariaDB.
- The Rust struct `GraphRetriever::search(query)` restricts the tuple's fourth column to `Option<String>`. 
- This type mismatch (`Rust type Option<String> is not compatible with SQL type BLOB`) causes `sqlx` to throw an Err, completely aborting graph processing.
**Fix required in:** `ro-ai-bridge/src/retrieval/graph.rs`
- Change the `entities` tuple decoding for the properties column to accept `Option<serde_json::Value>` or `Option<Vec<u8>>` and serialize it to string safely.

---

## 3. Tree Search (PageIndex) Deprecated Table Query
**Symptom:** Tree search returns 0 results.
**Root Cause:**
- The Tree fetcher `fetch_tree` queries a deleted legacy table: `SELECT ... FROM tenant_documents`.
- The database returns `Table 'mimir.tenant_documents' doesn't exist`, but an `.unwrap_or_default()` suppresses the panic.
- The actual tree structure currently lives in the `data_sources` table under the `pageindex_tree` column (confirmed that `cpg` source has 12,750+ characters of data).
**Fix required in:** `ro-ai-bridge/src/routes/search.rs`
- Point the Tree SQL query to `SELECT id, name, raw_markdown, pageindex_tree FROM data_sources WHERE tenant_id = ?`.

---

## 4. "Index Golden QA" Kanban Button Fails
**Symptom:** Pressing the button via UI does not populate `golden_qa`.
**Root Cause:**
1. **LlmRouter Crash**: `run_indexer` attempts to initialize an `LlmRouter` using `LlmRouter::new("megacare")`. But because `megacare` currently uses default LLM environment configurations and lacks a dedicated explicit row in `tenant_configs`, `get_tenant_config` panics with "no rows returned".
2. **Unnamed Vector Mapping**: Even if the router initializes, `indexer.rs` packages the vector payload using `"vector": vector_data` instead of `"vector": { "dense": vector_data }`, which Qdrant will reject since `golden_qa` is configured as a Named Vector collection.
**Fix required in:** 
- `mimir-core-ai/src/services/llm_router.rs`: Implement a default fallback config when a specific tenant row doesn't exist.
- `mimir-core-ai/src/qa_qc/indexer.rs`: Convert vector JSON payload to `{ "vector": { "dense": vector } }`.

---

## 5. Deletion of `wiki_qa`
**Requirement:** `wiki_qa` must be fully wiped, and Playground switches default routing to `golden_qa`.
**Action:**
- Run `curl -X DELETE http://localhost:6335/collections/wiki_qa`.
- Confirm all references targeting `wiki_qa` strictly point to `golden_qa`.
