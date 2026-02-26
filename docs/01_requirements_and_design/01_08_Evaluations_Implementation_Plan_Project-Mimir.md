# 📋 Evaluations Implementation Plan

## 1. Overview and Analysis of Current Usage

The **Evaluations Page** is designed to automatically test AI Agents (e.g., `oracle_rag`, `simple_npc`) paired with different LLM models (e.g., `gemini-2.5-flash`, `ollama:llama3`) against a set of expected Q/A pairs. It uses an **LLM-as-a-Judge** (Gemini) to score the responses on Accuracy, Completeness, and Relevance.

### Is it appropriate and how is it used?
✅ **Highly Appropriate.** 
This is the only objective way to determine which combinations of prompt instructions (Agents) and base AI models (LLMs) yield the best results. It allows admins to scientifically choose the most cost-effective or highest-performing model for their RAG system. The hybrid Agent × Model matrix UI is excellent.

---

## 2. Gap Analysis (Frontend vs Backend vs TRD)

There are significant architectural gaps between the current implementation and a production-grade Multi-Tenant system.

### Database Gaps (`migrations/202602200000_eval_system.sql`)
- 🔴 **No Multi-Tenancy**: The tables `eval_runs`, `eval_scores`, and `eval_summary` lack a `tenant_id` column. A run currently evaluates the entire system globally, which breaches data isolation.

### Backend API Gaps (`ro-ai-bridge/src/routes/eval.rs` & `run_eval.rs`)
- 🔴 **Missing Security**: The API routes in `eval.rs` do not use `tenant_auth_middleware`. Anyone can read the evaluation results.
- 🔴 **Missing Trigger Endpoint**: The TRD lists `POST /api/eval/run` as an implemented endpoint to trigger evaluations from the dashboard. This endpoint does **not** exist.
- 🟡 **Hardcoded Dataset**: The evaluation runner script (`src/bin/run_eval.rs`) is hardcoded to load Q/A pairs from a local file (`data/qa_dataset.json`). It should instead fetch validated (`status = 'COMPLETED'`) "Golden Answers" from the `qa_results` table for a specific tenant.

### Frontend Gaps (`ro-ai-dashboard/src/app/evaluations/page.tsx`)
- 🟡 **No Tenant Filter**: The UI has no way to filter eval runs by tenant.
- 🟡 **No Run Trigger**: The empty state tells the user to run `cargo run --bin run_eval` in the terminal. The UI needs a "Run Evaluation" button to trigger the missing API endpoint.

---

## 3. Implementation Plan

To align the Evaluation system with Project Mimir's TRD and Multi-Tenant architecture, we need a phased approach:

### Phase 1: Database and Security Overhaul
1. **Migration Script**: Create a new SQL migration to add `tenant_id` to `eval_runs`, `eval_scores`, and `eval_summary`.
   ```sql
   ALTER TABLE eval_runs ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
   -- (Add identically to eval_scores and eval_summary)
   ```
2. **API Middleware**: Wrap the `eval_routes()` inside `monitor.rs` with `tenant_auth_middleware`.
3. **Filter by Tenant**: Update all SQL queries in `eval.rs` (`list_runs`, `get_run_detail`, etc.) to append `WHERE tenant_id = ?` based on the authenticated context.

### Phase 2: Refactoring the Evaluation Runner
1. **Move Logic to Crate**: Export the core logic of `src/bin/run_eval.rs` into a functional service inside `mimir-core-ai/src/evaluation/runner.rs` so it can be called by both the CLI and the API.
2. **Dynamic DB Dataset**: Modify the runner to fetch its questions from MariaDB instead of `qa_dataset.json`:
   ```sql
   SELECT question, answer AS expected_answer 
   FROM qa_results 
   WHERE tenant_id = ? AND status = 'COMPLETED'
   ORDER BY RAND() LIMIT ?
   ```
3. **API Endpoint**: Implement `POST /api/v1/eval/run` in `monitor.rs`. This will spawn a detached Tokio task to run the evaluation in the background, allowing the HTTP request to return a `202 Accepted` immediately.

### Phase 3: Frontend Enhancements & UX/UI Redesign
1. **Trigger Button & Run Wizard**: 
   - Add a prominent "New Evaluation" button next to "Refresh".
   - Clicking it opens a Wizard/Modal: 
     - Step 1: Select Agents (e.g., `oracle_rag`, `simple_npc`).
     - Step 2: Select LLM Models (multiselect).
     - Step 3: Select Question Pool (e.g., All COMPLETED questions, or specific tags/limits).
     - Step 4: Confirm & Run.
2. **Real-time Progress UI**:
   - While a run is `RUNNING`, show a visible progress bar (e.g., "Evaluating 45/100...").
   - This keeps the user informed that the background job `POST /api/v1/eval/run` is active.
3. **Tenant Context & Filter**: 
   - Add the standard Tenant Selection dropdown (from the Vector/QC pages) if the user is a Super Admin. Include the `tenantId` in the `fetch` calls to `/api/eval/runs`.
   - Display the active Tenant clearly at the top of the evaluation dashboard so admins know whose data they are testing.
4. **Enhanced Heatmap (Agent × Model Matrix)**:
   - Add rich tooltips on hover over each cell showing the breakdown (Accuracy, Completeness, Relevance) without needing to click.
   - Highlight the "Best in Class" model for the selected agent with a star or trophy icon.
5. **Interactive Review & Side-by-Side Comparison**:
   - When expanding a cell to see the individual question scores, use a **Split-Pane** or **Side-by-Side** layout for "Expected Answer" vs "Actual Answer" to easily spot discrepancies.
   - Add an inline **"Override Score"** button next to each question row. This allows the human Admin to quickly submit a `PATCH /api/eval/scores/:id/review` if they disagree with the LLM-as-a-Judge.
6. **Historical Trends (Optional Phase 3.5)**:
   - Provide a sparkline or small line chart showing the *Historical Overall Score* for a specific Agent+Model combination across the last 5 runs, to easily see if prompts are degrading over time (Regression Testing).

---

## 4. Verification Plan

1. **Database Multi-Tenancy Test**:
   - Manually trigger an evaluation run for `ragnarok_th` (via the new frontend button).
   - Verify that the `eval_runs` table inserts the row with `tenant_id = 'ragnarok_th'`.
   - Log in as an admin for `med_clinic_a` and verify that the `ragnarok_th` run does **not** appear in their Evaluations dashboard list.
   
2. **Dataset Retrieval Test**:
   - Instead of testing against the JSON file, ensure that the evaluation only uses questions specific to the selected tenant by checking the `eval_scores` table questions against that tenant's `qa_results`.

3. **Background Job Test**:
   - Click "Run New Evaluation" on the frontend.
   - Assert that the UI receives a success response immediately without freezing, while the backend logs show the evaluation progressing chunk-by-chunk in the background.
