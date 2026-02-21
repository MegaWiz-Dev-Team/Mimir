# Implementation Plan: Playground UI Enhancements for Persona & Game Data

## Overview
This document outlines the frontend UI/UX updates needed to support custom Persona Avatars, display NPC AI capabilities (like RAG and Actions), and show the Vector Database (Knowledge Base) status directly within the Playground.

## Proposed Changes

### 1. Persona Avatars
The `api.ts` file currently defines `avatar_url` pointing to `/avatars/xxx.png`, but the directory and image files do not exist.
- **Action**: Create the `public/avatars` folder.
- **Action**: Add avatar image files (`mimir.png`, `sage_ariel.png`, `fortune_teller.png`, `blacksmith.png`) to be rendered in the Playground UI sidebar and chat messages.

### 2. Displaying NPC Capabilities & RAG Connection
We need to visually communicate what each NPC can do, especially since we are integrating rAthena databases.
- **Action**: Modify `ro-ai-dashboard/src/app/playground/page.tsx`.
- **Capabilities Badges:** Under the Persona description in the sidebar, iterate over the hardcoded `PERSONAS` traits or infer from their tier/name to display specific capability badges.
  - *Sage Ariel:* badge labeled "📚 RAG: item_db, mob_db".
  - *Mimir:* badge labeled "⚔️ Actions: heal, buff".
- **Vector DB Status Widget:** Integrate a small database status indicator using the existing `fetchVectorStats()` API (or a new dedicated backend API) to show the number of ingested vectors in Qdrant (ro_items, ro_monsters). This proves to the user that the "Game Data RAG is online" right inside the Playground.
- **Avatar Component:** Update the chat bubble renderer to display the NPC's `avatar_url` icon next to their messages.

### 3. Chat Interface Immersiveness
- Enhance the Chat UI in `page.tsx` with premium spacing and styling for NPC responses to make it feel more like an authentic RPG interaction.

### 4. Global Navigation & Layout Refactoring
The current layout places core features inside the Pipeline Monitor, causing confusion and duplicate navigation buttons.
- **Action**: Modify `ro-ai-dashboard/src/components/navbar.tsx` to include `Evaluations`, `Playground`, and `Quality Control` as top-level navigation items.
- **Action**: Modify `ro-ai-dashboard/src/app/page.tsx` (Dashboard) to remove the conflicting `Evaluations`, `Agent Playground`, and `Vector Database` sub-menu buttons.

## Verification Plan
1. Open the Playground at `http://localhost:3000/playground`.
2. Select different Personas and verify their unique avatars appear in the UI.
3. Verify Capability Badges show up correctly for Mimir (Actions) and Ariel (RAG).
4. Verify the Knowledge Base connection status is visible and fetching real stats from Qdrant.
