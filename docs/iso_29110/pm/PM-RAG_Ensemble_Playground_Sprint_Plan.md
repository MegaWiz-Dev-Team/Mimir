# Sprint Planning
**Project Name:** Mimir Asgard Platform 
**Sprint Goal:** Implement the Asgard Autonomous Agent Ecosystem (4-Phase Architecture)
**Document Version:** 2.0 (Master Blueprint Integration)
**Date:** 2026-03-31
**Standard:** Aligned with ISO/IEC 29110 Project Management (PM) Process

## 1. Sprint Objectives
- **Phase 1 (Upstream):** Establish a 5-Agent Data Ingestion pipeline with automated Semantic Splitting, Graph extraction, and Vector Clustering.
- **Phase 2 (Midstream):** Transform the RAG Ensemble Playground into an advanced tuning lab with parallel fetching (`tokio::join!`), AI query optimizers, and batch evaluation.
- **Phase 3 (Downstream):** Upgrade the Agent Builder to support "Soul & Skill" decoupling and Swarm Pattern (A2A Autonomous Handoffs).
- **Phase 4 (Autonomy):** Lay the groundwork for "The Overseer" Meta-Agent to perform Vector Map Overlays and auto-heal deployed agents.

## 2. Gap Analysis & Sidecar Architecture (Current vs. Blueprint 2.0)
Based on an engineering review of the `ro-ai-bridge` and `ro-ai-dashboard` codebases:

### Readiness Assessment: 80% (Robust Foundation)
The Rust backend and database schemas (`agent_configs` table with `system_prompt`, `tools`, `use_rag`) already highly support the "Soul & Skill" architecture. To prevent code bloat and protect the stability of the core `ro-ai-bridge` API, all new Phase 1 (Ingestion) and Phase 4 (Overseer) processing logic will be deployed as a **Dedicated Agent Sidecar Microservice** (e.g., `asgard-swarm-sidecar`). 

### Identified Gaps:
- **Phase 1 Gaps:** Missing the automated "Synthetic Q&A Agent" and nightly "Vector Clustering" background jobs. These heavy workloads will run inside the new Sidecar.
- **Phase 2 Gaps:** Missing the `/api/search` endpoint that returns raw unformatted chunks from Vector/Graph/Tree inside Mimir Core. Lacks the "Optimizer ✨" tool.
- **Phase 3 Gaps:** Missing Swarm routing logic (`handoff_to_agent()` tool) and Ping-Pong Infinite Loop detection safeties. 
- **Phase 4 Gaps:** Missing entirely. Requires a cron-triggered worker service inside the Sidecar to harvest Bifrost telemetry logs for Map Overlays.

---

## 3. Sprint Backlog (User Stories & Tasks)

### Epic 1: Data Ingestion Pipeline & Clustering (Phase 1)
**User Story:** As an Operations Manager, I want data automatically processed by 5 specialized agents and clustered overnight, so I have a visual map of my knowledge base without manual sorting.
- [ ] **Task 1.1:** Develop the Synthetic Q&A Agent to generate testable prompt/answer pairs during document ingestion.
- [ ] **Task 1.2:** Implement a nightly Vector Clustering background job (K-Means/HDBSCAN) to group vectors into semantic topics.
- [ ] **Task 1.3:** Build the Frontend DAG Visualizer showing real-time status of the 5-Agent ingestion pipeline.

### Epic 2: RAG Playground & Batch Benchmarking (Phase 2)
**User Story:** As an Admin, I want to retrieve raw data across Vector/Graph/Tree and run Batch Evaluations using dynamic weight sliders to find the perfect search configuration.
- [ ] **Task 2.1:** Create `POST /api/search` using `tokio::join!` for parallel fetching of raw chunks, bypassing LLM synthesis.
- [ ] **Task 2.2:** Create `POST /api/search/optimize` invoking the Optimizer Agent to suggest 3-5 evolved search prompts.
- [ ] **Task 2.3:** Create `POST /api/search/benchmark` to calculate Hit Rate and MRR against a selected Stratified Eval Set.
- [ ] **Task 2.4:** Build Frontend RAG UI (Source Weight Sliders, Optimizer Button, Batch Evaluator Dashboard).

### Epic 3: Agent Builder & Swarm Handoff (Phase 3)
**User Story:** As an App Admin, I want to equip Agents with specific Skills/Souls and enable them to autonomously hand off tasks to other agents safely before deploying to Bifrost.
- [ ] **Task 3.1:** Implement the `handoff_to_agent()` MCP tool allowing autonomous Agent-to-Agent (A2A) task routing.
- [ ] **Task 3.2:** Implement safety middleware: "Repetitive Handoff Detection Window" to break infinite Ping-Pong routing loops.
- [ ] **Task 3.3:** Implement `POST /api/agents/publish` to lock the Agent profile and inject routing configs into the Bifrost Gateway.
- [ ] **Task 3.4:** Overhaul Frontend Agent Builder to a 3-column Integration Hub (Profile -> Integrations/MCP -> Live Test Sandbox).

### Epic 4: The Overseer Meta-Agent (Phase 4)
**User Story:** As a Platform Owner, I want an Overlord Agent to continuously monitor latency, thumbs-down feedback, and "blind spots", autonomously fixing issues or alerting humans.
- [ ] **Task 4.1:** Develop a telemetry pipeline consuming latency and user feedback from Bifrost.
- [ ] **Task 4.2:** Integrate Vector Map Overlay logic for The Overseer to compare User Queries vs. Knowledge Clusters to find "Blind Spots".
- [ ] **Task 4.3:** Implement Auto-Healing triggers (Overseer invoking RAG Tuner API to fix failing agents).

---

## 4. Risk Management
- **Risk 1:** Database lockups/timeouts during concurrent Vector/Graph/Tree execution in `tokio::join!`.
  - *Mitigation:* Apply strict `limit` clauses, pagination, and context timeout boundaries.
- **Risk 2:** Infinite Loops during Swarm Autonomous Handoffs.
  - *Mitigation:* Hardcoded safety limits (`max_handoffs = 5`, repetitive detection) integrated natively in the Routing Engine.
- **Risk 3:** High cost/token usage from continuous Overseer evaluations.
  - *Mitigation:* Run semantic Map Overlays only once per day (Nightly batch) rather than on every query.

## 5. Dependencies
- Heimdall LLM active connections (essential for Optimizer, 5-Agent pipeline, and Swarm capabilities).
- Bifrost Gateway logging infrastructure must emit standard telemetry for the Overseer to consume.
