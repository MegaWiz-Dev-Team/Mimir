# Implementation Plan: Multi-Tenant Modular Architecture (v2.3)

**Date:** 2026-02-21
**Target:** Transform `ro-ai-bridge` from a monolithic application into a domain-agnostic, multi-tenant AI platform.

## 1. Goal Description
Decouple the Core AI capabilities (Ingress, RAG, QA/QC, Vector Search, Evaluation) from the Ragnarok Online specific game logic. This enables:
1.  **Reusability:** The Core AI can be deployed for medical, corporate, or other game domains without code duplication.
2.  **Multi-Tenancy:** A single active deployment can serve multiple tenants (e.g., `ro_th_server`, `ro_ph_server`, `clinic_a`) by isolating data using `tenant_id`.

**Related Specifications:**
- 🎨 [UI/UX Specification - Multi-Tenant Dashboard](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/05_01_UI_UX_Spec_Multi_Tenant.md)
- 🔒 [Security Specification - Isolation & Access](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/06_01_Security_Spec_Multi_Tenant.md)
- 👤 [User Management & RBAC Specification (On-Premise)](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/07_01_User_Management_RBAC_Spec.md)

## 2. Architecture Changes
The codebase will transition into a Rust **Workspace (Monorepo)** structure:

*   `mimir-core-ai` (Crate): The Domain-Agnostic Core Platform.
*   `ro-ai-domain-game` (Crate): The Game Connector for rAthena.
*   *(Future)* `medical-domain` (Crate): Medical connector.

## 3. Execution Roadmap

### Phase 1: Foundation (Workspace Setup)
*   **Action:** Convert the root `ro-ai-bridge` into a Cargo Workspace.
*   **Action:** Create the `mimir-core-ai` library crate.
*   **Action:** Create the `ro-ai-domain-game` binary crate.
*   **Action:** Migrate core foundational services (`db`, `qdrant`, `mcp_client`, `scraper`) into `mimir-core-ai/src/services`.

### Phase 2: Core Platform Isolation
*   **Action:** Move the `wiki_workshop` pipeline (generator, extractor, indexer, verifier) into `mimir-core-ai/src/qa_qc`.
*   **Action:** Move `oracle_rag.rs` into `mimir-core-ai/src/rag_engine`.
*   **Action:** Refactor these modules to remove any hardcoded Ragnarok terminology (e.g., "Zeny", "Prontera"). They must accept generic system prompts and context.

### Phase 3: Domain Connection (Game Integration)
*   **Action:** Set up the main Axum web server inside `ro-ai-domain-game`.
*   **Action:** Move `simple_npc.rs` and `rathena_gateway` specific routes into this domain crate.
*   **Action:** Wire up the Game Domain to initialize and call `mimir-core-ai` functions when handling game events.

### Phase 4: Multi-Tenant Data Isolation
*   **SQL Migration (MariaDB):** Add `tenant_id` (VARCHAR) to `qa_results`, `pipeline_runs`, `pipeline_steps`, `qa_clusters`, and `evaluation_reports`. Make it part of the primary composites or indexes.
*   **Vector DB (Qdrant):** Implement Payload-based Partitioning. Every inserted vector gets `{"tenant_id": "<value>"}`. Every RAG query must prepend a filter matching the requesting `tenant_id`.
*   **Middleware:** Introduce an Axum middleware in the Game Domain to extract `tenant_id` from API Keys or Origin headers and inject it into the request Context.
*   **Security Restrictions:** Implement M2M Auth and strictly follow guidelines in the [Security Specification](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/06_01_Security_Spec_Multi_Tenant.md).

### Phase 5: On-Premise IAM & RBAC (New)
*   **Prerequisite:** Ensure Phase 1-3 are stable before implementing User Management.
*   **Action:** Add `users`, `tenants`, and `tenant_users` tables to the MariaDB Schema.
*   **Action:** Implement JWT-based Auth endpoints (`/login`) in the Core AI platform using `argon2` for password hashing and establish token rotation rules.
*   **Action:** Add `UserContext` Axum middleware to protect routes based on the token's `role` (Admin, Editor, Viewer).

### Phase 6: Dashboard Expansion (UX/UI)
*   **Action:** Implement Login screen and `NextAuth` / Custom Session wrapper in Next.js.
*   **Action:** Build the Global Tenant Switcher so the admin can traverse between domains.
*   **Action:** Design and implement the `/sources` config and `/quality_control` Kanban screens.

## 4. Current Status
*   [ ] Phase 1: Foundation
*   [ ] Phase 2: Core Platform Isolation
*   [ ] Phase 3: Domain Connection
*   [ ] Phase 4: Multi-Tenant Data Isolation
*   [ ] Phase 5: On-Premise IAM & RBAC
*   [ ] Phase 6: Dashboard Expansion (UX/UI)
