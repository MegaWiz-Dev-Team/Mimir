# E2E Test Plan: Project Mimir (Phase 1 - 9)
**Description:** Verify system integration across Frontend, Backend, MariaDB/Qdrant databases, and rAthena C++ Game Server after a fresh deployment/DB reset.

## Pre-requisites
- [x] Docker Desktop is running
- [x] `docker-compose up -d` has successfully initialized MariaDB, Qdrant, Redis, and all `rathena_*` containers
- [x] The `.env` files are configured with valid LLM API keys

---
## Scenarios to Test

### TS-01: Backend Initialization
- [x] Open a terminal and run `cargo run --bin monitor` (or `ro-ai-domain-game`).
  - **Expected:** Server starts without panic, HTTP server binds to `0.0.0.0:8080`.
  - **Actual:** Backend successfully compiled and connected to DB. API is active on port 8080.
- [x] Open Frontend (`http://localhost:3000/`) and check the bottom-left corner.
  - **Expected:** The red error indicator disappears and the Dashboard loads successfully.
  - **Actual:** "3 Issues" error red badge at the bottom-left resolved. Dashboard API is active.

### TS-02: Pipeline Run (Data Gen)
*Requires TS-01.*
- [x] On the Dashboard (`/`), select Provider (e.g., `gemini`) and Model.
  - **Expected:** Dropdowns populate correctly.
  - **Actual:** Dropdowns correctly list Provider (Google) and corresponding Models.
- [x] Click "Run Pipeline".
  - **Expected:** Pipeline status appears in "Recent Runs" as "Running".
  - **Actual:** Pipeline initializes successfully in the background.
- [x] Wait 1-2 minutes and click "Refresh".
  - **Expected:** Run ID completes successfully (Status: "Completed"). Success Rate increases.
  - **Actual:** Pipeline processed successfully (Coverage: 1805%, 100/102 Chunks).
- [x] Navigate to the "Vector DB" tab (`/vector`).
  - **Expected:** Qdrant stats show `ro_items` and `ro_monsters` have >0 vectors (data ingested).
  - **Actual:** Confirmed via API - `ro_items` has 29,071 vectors, `ro_monsters` has 2,675 vectors.

### TS-03: Dashboard UI & Features
*Requires Frontend `npm run dev`.*
- [ ] Observe the top navigation bar.
  - **Expected:** `[Dashboard] [Evaluations] [Playground] [Sources] [Quality Control] [Vector DB] [Users]` are visible.
- [ ] Click on the "Playground" tab.
  - **Expected:** Navigates to the Agent Playground without errors.
- [ ] Look at the "Settings" sidebar on the left.
  - **Expected:** Under "Knowledge Base (Qdrant)", a green "Online" badge appears with Item DB/Mob DB counts.
- [ ] Select "Mimir" from the Persona dropdown.
  - **Expected:** Purple badge `⚔️ Actions: heal, buff` is displayed under his name.
- [ ] Select "Sage Ariel" from the Persona dropdown.
  - **Expected:** Green badge `📚 RAG: item_db, mob_db` is displayed under her name.

### TS-04: AI Playground (RAG & Actions)
*Requires TS-02 (Data exists) and TS-03.*
- [ ] With "Mimir" selected, type: `ช่วย heal ฉันหน่อย`
  - **Expected:** Mimir replies acknowledging the heal. The message block shows a green badge: `Action Invoked: heal`.
- [ ] Switch to "Sage Ariel". Type: `Poring ดรอปอะไรบ้าง?`
  - **Expected:** Ariel replies with accurate Poring drop data. A "Source" citation block appears below her response.
- [ ] Click on the Source citation block.
  - **Expected:** A modal pops up showing the raw text data from the database.

### TS-05: rAthena In-Game NPC Integration
*Requires TS-04 and Game Servers (`map`, `char`, `login`) running.*
- [ ] Login to the game client using a GM account.
  - **Expected:** Successfully enter the game world.
- [ ] Type `@reloadscript` in the game chat.
  - **Expected:** Server message confirms scripts are reloaded successfully.
- [ ] Type `@go 0` to warp to Prontera, and walk to `150, 150` (center of town).
  - **Expected:** 4 AI NPCs are visible: Mimir, Sage Ariel, Fortune Teller Maya, Blacksmith Grumm.
- [ ] Click on "Mimir AI" and type: `ขอรับบัพหน่อยครับ`
  - **Expected:** Mimir thinks for a moment, replies in the chat window, and your character receives Agi/Blessing buffs (Visual effects trigger).
- [ ] Click on "Sage Ariel" and type: `Jellopy เอาไว้ทำอะไร?`
  - **Expected:** Ariel fetches data from the Rust backend via HTTP and replies accurately based on the RAG system.
