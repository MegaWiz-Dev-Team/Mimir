# 📊 Sources Page & Data Ingress Implementation Plan

## 1. Overview and Gap Analysis

This document outlines the gaps between the Technical Requirement Document (TRD v2.3) and the current implementation of the "Data Ingress Sources" page (`src/app/sources/page.tsx`), along with the roadmap to implement the missing features.

### TRD Requirements (Section 4.1 & 4.2)
The TRD specifies that the system must support configurable input sources to feed the Core AI Platform:
- **Web URLs**: Scraped via Headless Browser (chromiumoxide) or standard scraper.
- **Tabular Files**: CSV/XLSX parsed via Tabular Parser (Chunking & Verbalization).
- **Media/Documents**: PDF, PPT, Images parsed via Vision Parser (OCR).
- **MCP Servers**: External server connections (e.g., GitBook).

### Current Implementation State (`ro-ai-dashboard/src/app/sources/page.tsx`)
- **Status:** 🔴 **Fully Mocked UI**
- **Gaps:**
  1. The page currently uses a hardcoded `MOCK_SOURCES` array.
  2. The "Add Source" button triggers a native `alert()` instead of a functional dialog.
  3. "Configure" and "Delete" buttons have no attached handlers.
  4. There is no connected Backend API to fetch, create, update, or delete source configurations.
  5. There is no database table to store tenant-specific source configurations.

---

## 2. Implementation Plan

To close these gaps and achieve End-to-End functionality, the following steps must be implemented across both the Database, Backend (Rust), and Frontend (Next.js).

### Phase 1: Database Schema Expansion
Create a new migration (e.g., `20260222000000_create_data_sources.sql`) in `ro-ai-bridge` to store source configurations per tenant.

```sql
CREATE TABLE data_sources (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    source_type ENUM('web', 'tabular', 'document', 'mcp') NOT NULL,
    config_json JSON NOT NULL, -- Stores URLs, file paths, or MCP connection strings
    schedule VARCHAR(100), -- e.g., 'Manual', 'Daily at 00:00'
    last_sync_status ENUM('PENDING', 'RUNNING', 'COMPLETED', 'FAILED') DEFAULT 'PENDING',
    last_sync_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_tenant (tenant_id)
);
```

### Phase 2: Backend API Development (`ro-ai-bridge`)
Develop standard CRUD endpoints in the Axum HTTP server (`mimir-core-ai/src/ingress/api.rs`).

1. **`GET /api/sources`**: Fetch all sources for the current `tenant_id`.
2. **`POST /api/sources`**: Create a new data source configuration.
3. **`PUT /api/sources/:id`**: Update an existing source configuration.
4. **`DELETE /api/sources/:id`**: Remove a source configuration.
5. **`POST /api/sources/:id/sync`**: Trigger an immediate data ingestion job for the specified source.

### Phase 3: Ingress Services Implementation (`mimir-core-ai/src/ingress/`)
Implement the actual data parsing logic defined in the TRD:
1. **Web Scraper**: Integrate `chromiumoxide` to fetch and convert HTML to Markdown.
2. **Tabular Parser**: (`services/tabular_parser.rs`) Convert CSV/XLSX rows into readable Markdown chunks.
3. **Vision/Document Parser**: (`services/vision_parser.rs`) Integrate with Gemini 2.5 Flash Vision capabilities or a local OCR to extract text from PDFs and images.
4. **MCP Connector**: Create a generic client to communicate with external MCP servers.

*Note: All ingested data should be formatted as standardized Markdown and saved to the `data/wiki` directory (or directly passed to the Generator Pipeline) so the existing QA generation process can process them seamlessly.*

### Phase 4: Frontend UI Integration (`ro-ai-dashboard`)
Connect the React UI to the new Backend APIs.

1. **API Client (`src/lib/api.ts`)**:
   - Add functions: `fetchSources()`, `createSource()`, `updateSource()`, `deleteSource()`, `syncSource()`.
2. **State Management (`src/app/sources/page.tsx`)**:
   - Replace `MOCK_SOURCES` with `useEffect` fetching real data from `fetchSources()`.
3. **Add Source Dialog**:
   - Implement a Modal form (using `shadcn/ui` Dialog) that dynamically changes input fields based on the selected `source_type` (e.g., File upload for tabular vs. URL input for Web).
4. **Action Handlers**:
   - Wire up the Delete button (with confirmation).
   - Wire up the Sync button to trigger `POST /api/sources/:id/sync`.

---

## 3. Verification Plan

Once implemented, the feature will be verified through the following workflow:

1. **Creation Test**: Log into the Dashboard, navigate to Sources, and add a "Web" source pointing to a public wiki page. Verify it appears in the table.
2. **Sync Test**: Click "Sync" on the newly created source. Verify the backend logs show the Web Scraper running and saving a `.md` file to the `data/wiki` folder.
3. **E2E Integration**: Go to the Dashboard home, click "Trigger Run", and verify the pipeline picks up the newly scraped `.md` file and generates Q/A pairs normally.
4. **Deletion Test**: Delete the source from the UI and verify it disappears from the `data_sources` table in MariaDB.
