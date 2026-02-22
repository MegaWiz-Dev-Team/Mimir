# 📋 Vector Management Implementation Plan

## 1. Overview and Analysis of Current Usage

The Vector Management page is designed to give admins visibility into the health of the RAG system's core component: **The Vector Database (Qdrant)**. 

### Is it appropriate and how is it used?
✅ **Highly Appropriate for Admins.**
1. **Sync Monitoring**: It compares the MariaDB `qa_results` total against the Qdrant `points_count`. This provides a visual "Sync Rate", immediately alerting admins if the background Indexer is failing or lagging.
2. **Search Preview**: The most critical feature. It allows an admin to type a question and see *exactly* what vectors Qdrant returns, along with their similarity scores. This "black box" visibility is essential for debugging why an AI agent might be giving wrong answers (e.g., Is the data missing? Or is the embedding similarity too low?).

---

## 2. Gap Analysis (Frontend vs Backend)

While the UI is well-designed, there is a major gap in the **Search API** that completely breaks the **Multi-Tenancy** requirement defined in the TRD.

### Frontend Gaps (`src/app/vector/page.tsx` & `src/lib/api.ts`)
- **Status**: 🟡 **API Disconnect**
- The UI contains dropdowns for **"Filter by Tenant"** and a checkbox for **"Show Expired Data"**.
- ❌ **Gap**: The `handleSearch` function and `searchVectors` API wrapper completely ignore these UI elements. They only send `{ query, limit }` to the backend. The chosen Tenant filter is never transmitted.

### Backend Gaps (`src/bin/monitor.rs` & `src/services/qdrant.rs`)
- **Status**: 🔴 **Security & Logic Flaw (Hardcoded Tenant)**
- ❌ **Gap 1 (`monitor.rs: search_vectors`)**: The endpoint `POST /api/vector/search` is missing the `tenant_auth_middleware` extraction. Anyone can call it without a tenant token.
- ❌ **Gap 2 (Hardcoded Logic)**: The endpoint accepts `SearchRequest { query, limit }` but does not accept a `tenant_id`. When it calls Qdrant, it hardcodes `"default_tenant"`:
  ```rust
  // Line 551 in monitor.rs
  state.qdrant.search("wiki_qa", vector_f32, payload.limit.unwrap_or(5), "default_tenant").await
  ```
  This means if an admin from `ragnarok_th` searches, they will only see results from `default_tenant`, breaking multi-tenancy.
- ❌ **Gap 3 (Expired Data)**: There is no logical handling in Qdrant for "Expired Data" (e.g., `is_active = false`). It currently assumes all data in Qdrant is valid.

---

## 3. Implementation Plan

To fix these gaps and make the Vector Management page fully functional, we will implement the following phases:

### Phase 1: Backend API Overhaul (Search and Multi-Tenancy)
1. **Update `SearchRequest` Struct** (`mimir-core-ai/src/bin/monitor.rs`):
   ```rust
   #[derive(Deserialize)]
   struct SearchRequest {
       query: String,
       limit: Option<usize>,
       tenant_id: Option<String>, // Allow admin to override, otherwise use context
       show_expired: Option<bool>,
   }
   ```
2. **Inject Tenant Context**: Update the `search_vectors` function signature to extract the `Extension(tenant): Extension<TenantContext>`.
3. **Dynamic Tenant Routing**: Inside `search_vectors`:
   - If the request provides a `tenant_id` AND the user is a `SuperAdmin` (requires IAM check), use the requested `tenant_id`.
   - Otherwise, strictly enforce the `tenant.tenant_id` from the JWT token.
4. **Update `get_vector_stats`**: Ensure stats calculations (`SELECT count(*) FROM qa_results WHERE tenant_id = ?`) are filtered by the current user's `tenant_id`, rather than doing a global count.

### Phase 2: Qdrant Service Update (`src/services/qdrant.rs`)
1. **Update `search()` method**:
   - Add capability to filter by multiple `must` payload conditions.
   - If `show_expired = false` (default), add a filter `{ "key": "is_active", "match": { "value": true } }`. *(Note: This assumes the `indexer.rs` adds `is_active` to the payload during upsert. This may need verifying).*

### Phase 3: Frontend Enhancements & UX/UI Redesign (`page.tsx`)
1. **Pass Filters to API**:
   - Update `lib/api.ts` to include `filterTenant` and `showExpired` in the payload for `fetch('/api/vector/search')`.
   - Update `handleSearch` to send the current state variables.
2. **Interactive Search Result UI (Redesign)**:
   - **Expandable Rows**: Instead of just showing standard table rows, make the rows clickable (or add a chevron) to expand and reveal the *Full Source Content* (Page Content) that was matched.
   - **Similarity Score Badge**: Display the `score` right prominently with a color-coded badge (e.g., >0.85 = Green/High, >0.70 = Yellow/Medium, <0.70 = Red/Low) so admins immediately understand the confidence level.
   - **Tenant & Status Flags**: Clearly display the `tenant_id` and the `is_active` status flag on each search result.
3. **Delete/Purge Vector UI (New Feature)**:
   - Add a visible **"Delete Vector"** (Trash icon) button next to each search result in the table.
   - This prevents bad data from haunting the RAG system. Clicking it should call an API `DELETE /api/vector/:id` to remove it from Qdrant directly via the Dashboard.
   - *(Optional Phase 3.5)* **Bulk Purge**: A "Purge Expired Data" button at the top of the collection to instantly sweep older, inactive vector chunks across the selected tenant.
4. **View Raw Metadata (Developer Debug Mode)**:
   - Add a "View Raw JSON" toggle or modal to see the exact Qdrant payload (useful for engineering diagnostics).

---

## 4. Verification Plan

1. **Multi-Tenant Security Test**: 
   - Log in as an admin for `ragnarok_th`.
   - Go to Vector Management. Attempt to select `med_clinic_a` from the dropdown and search.
   - *Expected*: The backend should reject the search or override it to `ragnarok_th` (unless the user has a global SuperAdmin role).
2. **Search Accuracy Test**:
   - Type a known question into the Search Preview.
   - *Expected*: The results table should populate with chunks strictly belonging to the correct tenant.
3. **Stats Verification Test**:
   - *Expected*: The "MariaDB Q/A" count and "Qdrant Points" count should reflect the stats for the currently authenticated tenant, not the entire database total.
