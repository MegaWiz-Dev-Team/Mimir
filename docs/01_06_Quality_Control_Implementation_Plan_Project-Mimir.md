# 📋 Data Quality Control (QC) Implementation Plan

## 1. Overview and Gap Analysis

This document identifies the gaps between the Technical Requirement Document (TRD v2.3) and the current implementation of the "Data Quality Control" feature, which includes the Dashboard UI (`src/app/quality_control/page.tsx`) and the Backend API (`ro-ai-bridge`).

### TRD Requirements (Section 4.4)
The TRD specifies a 3-step Data Quality Control pipeline to handle duplicates and conflicting Q/A pairs:
1. **Clustering Phase**: Use an Embedding Model (e.g., `nomic-embed-text`) and a Clustering Algorithm (e.g., HDBSCAN) to group semantic similarities.
2. **Consensus & Conflict Detection**: Use an LLM-as-a-Judge (Gemini 2.5 Flash) to evaluate if the clustered Q/As are a "Consensus/Duplicate" or a "Conflict".
   - *Consensus*: Suggest a merged "Golden Answer".
   - *Conflict*: Flag for human review.
3. **Review Dashboard**: A UI for admins to resolve conflicts and approve automated merges.

### Current Implementation State
- **Frontend (`ro-ai-dashboard/src/app/quality_control/page.tsx`)**:
  - **Status:** 🟡 **Partially Implemented (UI structure exists, but missing modals)**
  - The UI has tabs for "Conflicts" and "Auto-Merges".
  - It successfully links to API calls (`fetchQcClusters`, `resolveQcCluster`, `triggerQcGeneration`).
  - ❌ "Manual Override" buttons currently just show `alert("Open Manual Override Editor")`. The real edit dialog is missing.
- **Backend (`ro-ai-bridge/mimir-core-ai/src/qa_qc/`)**:
  - **Status:** 🟡 **MVP/Mock Implementation Exists, but Lacks Full Logic**
  - ✅ Database tables (`qa_clusters`, `qa_cluster_items`) are already created and have `tenant_id`.
  - ✅ The API routes (`/api/v1/qc/clusters`, `/api/v1/qc/resolve`, `/api/v1/qc/generate`) are wired up in `monitor.rs`.
  - ❌ **Gap 1 (Real Clustering)**: `clustering.rs` does not use Embeddings or HDBSCAN. It simply queries 10 random unclustered QA rows and directly asks Gemini to "mock" picking 1 duplicate and 1 conflict out of them. This is an MVP stub and does not scale or work as semantic clustering.
  - ❌ **Gap 2 (Resolution Completion)**: `resolve_cluster` in `clustering.rs` only updates the `status` string to `MERGED` or `RESOLVED_A`. It **does not** actually take the `golden_answer` and re-index it into `qa_results` or the Vector Database. It also doesn't deactivate the original duplicated items.

---

## 2. Implementation Plan

To fully implement the Data Quality Control pipeline according to the TRD, the following steps must be taken.

### Phase 1: Real Semantic Clustering Engine (Backend)
Replace the MVP mock logic in `mimir-core-ai/src/qa_qc/clustering.rs`.
1. **Fetch QA Combinations**: Fetch all `qa_results` for the tenant.
2. **Embedding Gen**: Use `nomic-embed-text` to generate vector embeddings for the questions (if not already embedded) or fetch them directly from the Qdrant DB.
3. **Clustering Algorithm**: Implement a proper grouping mechanism. Since writing a full HDBSCAN in Rust can be complex, a simpler **Cosine Similarity Threshold** approach (e.g., > 0.85 similarity) can group unclustered Q/As first.
4. **LLM Evaluation**: Pass **only the highly similar groups** to Gemini to decide if they are "CONFLICT" or "DUPLICATE", and generate the `reasoning` and `golden_answer`.

### Phase 2: Complete the Resolution Flow (Backend)
Update the `resolve_cluster` function in `clustering.rs` to take concrete database actions.
1. **On `MERGE` or `MANUAL_OVERRIDE`**: 
   - Take the `golden_answer` and create a **new** row in `qa_results` with the merged knowledge. 
   - Tag the original `qa_results` rows (found via `qa_cluster_items`) as inactive or deleted so they don't pollute the RAG context.
2. **On `ACCEPT_A` / `ACCEPT_B` (Conflict Resolution)**:
   - Keep the chosen `qa_id` active.
   - Deactivate/delete the rejected `qa_id`.
   - Update `qa_clusters.status` to `RESOLVED`.
   - Optionally trigger vector re-indexing (`run_indexer`) to sync changes to Qdrant.

### Phase 3: Frontend UI Refinement (Frontend)
Update `src/app/quality_control/page.tsx` and related components.
1. **Remove `alert()` placeholders**: Implement real `Dialog` components using `shadcn/ui` for:
   - **Edit before approve** (allows tweaking the AI's golden answer before saving).
   - **Manual Override** (allows staff to write a completely new answer to resolve a conflict).
2. **Data Refreshing**: After a successful resolution API call, automatically re-fetch the cluster list so the resolved item disappears smoothly.

---

## 3. Verification Plan

1. **Auto-Scan Trigger Test**: Click "Auto-scan QC Issues" in the Dashboard. Verify the backend logs show the vector-based clustering grouping similar items (not just 10 random items).
2. **Resolution Test (Duplicate Merge)**: 
   - Open a duplicate group in the UI. 
   - Edit the Golden Answer slightly -> Click "Approve & Index". 
   - *Verification*: Check the mariadb `qa_results` table to ensure a new row was added with the edited golden answer, and the old grouped rows are appropriately archived.
3. **Resolution Test (Conflict)**: 
   - Click "Accept Source A" on a conflict group. 
   - *Verification*: The status in DB changes to `RESOLVED_A` and only Source A's data remains active for RAG.
