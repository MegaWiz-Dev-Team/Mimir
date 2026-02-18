# 🤖 Implementation Plan: Phase 2 — Agent Chat Testing

This phase focuses on implementing the AI agents (Tier 1 & Tier 2) and providing a playground in the dashboard to test them.

## User Review Required

> [!IMPORTANT]
> Tier 2 (Oracle) requires a running Qdrant instance. For testing, we may need to populate a small subset of the Wiki data into Qdrant if it hasn't been done yet.

## Proposed Changes

### [Backend] [src/agents/simple_npc.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/agents/simple_npc.rs) [NEW]
Implement the Tier 1 agent using `rig-core`.
- Basic completion agent with a system prompt.
- Supports "Personas" via system strings.

### [Backend] [src/agents/oracle_rag.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/agents/oracle_rag.rs) [NEW]
Implement the Tier 2 agent using `rig-core`.
- Incorporates RAG with Qdrant.
- Uses the Wiki Q/A and Lore data.

### [API] [src/bin/monitor.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/bin/monitor.rs) [MODIFY]
Expose new endpoints for testing.
- `POST /api/agents/chat`: Send a message to a specific agent tier.
    - Payload: `{ "tier": 1|2, "message": "...", "persona": "..." }`

### [Frontend] [Agent Playground] [NEW]
Add a new page in the dashboard for interactive testing.
- `ro-ai-dashboard/src/app/playground/page.tsx`: Interactive chat interface.
- Support selecting Agent Tier (1 or 2).
- Pre-defined personas (e.g., "Sage Ariel", "Fortune Teller").

## Verification Plan

### Automated Tests
- Unit tests for `SimpleAgent` ensuring it uses the system prompt.
- Mocked RAG tests for `OracleAgent`.

### Manual Verification
- Use the Dashboard Playground to chat with Tier 1.
- Use the Dashboard Playground to ask Lore/Game questions to Tier 2 and verify source citations.
