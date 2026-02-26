# 🏗️ Implementation Plan: Phase 1 Completion & Vector Indexing

This final part of Phase 1 focuses on connecting the generated Q/A data from MariaDB to our Vector Database (Qdrant) and hardening the pipeline for a production-like local setup on M3.

## User Review Required

> [!NOTE]
> We will use `nomic-embed-text` via Ollama for embeddings, as it runs efficiently on the M3 GPU.

## Proposed Changes

### [Backend] [src/services/qdrant.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/services/qdrant.rs) [NEW]
- Helper service to interact with Qdrant.
- Logic to create a collection with both Dense and Sparse vector support (for Hybrid Search).

### [Backend] [src/agents/wiki_workshop/indexer.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/agents/wiki_workshop/indexer.rs) [NEW]
- Create the indexing logic:
    1. Fetch `COMPLETED` steps/chunks from MariaDB.
    2. Generate embeddings for each Q/A pair using Ollama.
    3. Upsert data into Qdrant with metadata (Source URL, Filename).

### [Frontend] [RunDetailsPage](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-dashboard/src/app/runs/[id]/page.tsx) [MODIFY]
- Add a **"Resume"** button for runs in `FAILED` status.
- Logic: The backend should check for already `COMPLETED` steps and skip them, only processing `PENDING` or `FAILED` steps.

### [System] [Ollama] [NEW]
- Ensure `nomic-embed-text` and `bge-reranker-v2-m3` are pulled and ready for local inference.

## 📊 Vector Monitoring & Dashboard Integration

To provide visibility into the Vector Indexing process, we will add the following:

### [Backend] [src/bin/monitor.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/bin/monitor.rs) [MODIFY]
- **`GET /api/vector/stats`**: Returns:
    - Qdrant points count.
    - Total points in MariaDB vs. Indexed points.
    - Collection health status.
- **`POST /api/vector/index`**: Trigger a background task that calls `run_indexer`.

### [Frontend] [Vector Management Page](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-dashboard/src/app/vector/page.tsx) [NEW]
- Create a dedicated dashboard page for Vector Management.
- **Stats Overview**: Cards showing collection size, embedding model used, and indexing progress.
- **Indexing Trigger**: A button to manually start/resume the indexing process with a live progress bar.
- **Search Preview**: A simple text field to test vector search results directly from the dashboard.

## Verification Plan

### Automated Tests
- Verify that Qdrant contains the same number of points as the `qa_results` table.
- Test the "Resume" button by failing a run intentionally and clicking Resume.

### Manual Verification
- Visual inspection of the indexing log in the terminal.
- Test a raw query against Qdrant to ensure vectors are correctly stored.
