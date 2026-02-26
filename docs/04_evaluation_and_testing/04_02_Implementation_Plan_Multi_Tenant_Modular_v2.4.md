# Implementation Plan v2.4: Multi-Tenant ZeroClaw Integration & Dashboard Evolution

**Date:** 2026-02-23
**Status:** Sprint 1 & 2 Completed | Sprint 3 In Progress

## 1. Goal Description
Evolve the Project Mimir architecture by integrating **ZeroClaw** as a high-performance, domain-agnostic AI Bridge. This simplifies the `mimir-core-ai` library by delegating heavy lifting (Model Swapping, Identity Management, Vector Retrieval) to ZeroClaw, allowing Mimir to focus on higher-level orchestration and multi-tenant management.

## 2. Updated Architecture (v2.4)
The system transitions from a custom-built monolithic core towards a microservice-oriented architecture:

*   **`mimir-core-ai` (Core Orchestrator):** Manages Multi-tenancy, IAM, SQL Schema, and Tenant Configuration.
*   **`ZeroClaw` (AI Bridge - Standalone):** Handles LLM providers, AIEOS Persona management, and high-performance Vector Search.
*   **`ro-ai-domain-game` (Domain Connector):** Specifically connects Ragnarok Online (rAthena) events to the Mimir/ZeroClaw ecosystem.
*   **`ro-ai-dashboard` (Next.js):** The unified management interface for all tenants.

## 3. Execution Roadmap

### Phase 1-2: Completion Report (Legacy Modularization)
*   **[COMPLETED]** Cargo Workspace setup with `mimir-core-ai` and `ro-ai-domain-game`.
*   **[COMPLETED]** Migration of database services and basic RAG/QA-QC modules.
*   **[COMPLETED]** SQL Schema updates for `tenant_id` across all major tables.

### Phase 3: ZeroClaw Integration (New)
*   **Action:** Deploy ZeroClaw as a standalone microservice (Docker/Standalone Binary).
*   **Action:** Refactor `ro-ai-domain-game` to use ZeroClaw's HTTP Gateway for NPC interactions instead of direct `rig-core` calls.
*   **Action:** Migrate Persona definitions from hardcoded strings/files to ZeroClaw's AIEOS (JSON) format.

### Phase 4: Sprint 3 - Tenant Provisioning (Current)
*   **Action:** Implement the `TenantConfig` and `Provisioning` logic in `iam.rs` to automatically create Qdrant collections and default DB entries for new tenants.
*   **Action:** Wire up the "Create Tenant" wizard in the Dashboard.
*   **Action:** Implement "Tenant Isolation Middleware" in the domain gateway to ensure requests are routed to the correct ZeroClaw instance or collection.

### Phase 5: Dashboard UI/UX Redesign
*   **Action:** Finalize the `/sources` (Knowledge Ingestion) and `/quality-control` (Human-in-the-loop) screens.
*   **Action:** Implement the Global Tenant Switcher with role-based visibility.
*   **Action:** Add real-time monitoring of ZeroClaw performance metrics to the dashboard.

## 4. Completed Milestones (Verification Proof)
*   **Sprint 1**: Foundation & Monolith-to-Modular migration verified.
*   **Sprint 2**: Data isolation and IAM User Routing verified ([SI_04_2_Sprint2_TestScript.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/iso_29110/si/SI_04_2_Sprint2_TestScript.md)).

## 5. Verification Plan

### Automated Tests
*   Run `cargo test -p mimir-core-ai` to verify IAM and Tenant service logic.
*   Run `cargo run --bin monitor` to observe API request flow across the workspace.

### Manual Verification
*   **Login Flow:** Verify admin login and redirect to tenant-specific dashboard.
*   **Tenant Switching:** Change tenant in the UI and verify that API calls include the new `X-Tenant-ID` header.
*   **ZeroClaw Hello World:** Send a test request to ZeroClaw Gateway and verify response via `ro-ai-domain-game`.

> [!IMPORTANT]
> This plan assumes ZeroClaw will be managed as an external dependency to reduce maintenance overhead on the Mimir core repository.
