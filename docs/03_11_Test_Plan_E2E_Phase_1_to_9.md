# End-to-End (E2E) Test Plan: Project Mimir (Phase 1 - 9)

## 1. Test Plan Overview
**Objective:** To verify the system integration and data flow across all components of Project Mimir (Frontend Dashboard, Backend Core AI, MariaDB/Qdrant databases, and rAthena C++ Game Server) after a fresh database wipe.
**Scope:** Covers Data Generation Pipelines, AI Playground (UI & RAG), Action Execution (Heal/Buff), and In-Game NPC interaction via HTTP Bridge.
**Pre-requisites:**
1. Docker Desktop is running.
2. `docker-compose up -d` has successfully initialized MariaDB, Qdrant, Redis, and all `rathena_*` containers.
3. The `.env` files are configured with valid LLM API keys.

---

## 2. Test Scenarios (High Level)
| Scenario ID | Name                          | Description                                                             |
| :---------- | :---------------------------- | :---------------------------------------------------------------------- |
| **TS-01**   | Backend Initialization        | Verify the Rust backend starts successfully and connects to all DBs.    |
| **TS-02**   | Pipeline Run (Data Gen)       | Verify the system can generate QA pairs and ingest into Vector DB.      |
| **TS-03**   | Dashboard UI & Features       | Verify new UI elements (Avatar, Badges, DB Status) function correctly.  |
| **TS-04**   | AI Playground (RAG & Actions) | Verify Mimir can execute actions and Sage Ariel can fetch RAG data.     |
| **TS-05**   | rAthena In-Game NPC           | Verify players can talk to AI NPCs in Prontera and trigger Actions/RAG. |

---

## 3. Test Scripts (Step-by-Step execution)

### TS-01: Backend Initialization
**Pre-condition:** All Docker containers are up.
| Step | Action                                                                          | Expected Result                                                                      | Status (Pass/Fail) |
| :--- | :------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------- | :----------------- |
| 1.1  | Open a terminal and run `cargo run --bin ro-ai-domain-game` (or `monitor`).     | Server starts without panic, HTTP server binds to `0.0.0.0:8080`.                    |                    |
| 1.2  | Open Frontend (URL: `http://localhost:3000/`) and check the bottom-left corner. | The red error indicator (e.g., "3 Issues") disappears. Dashboard loads successfully. |                    |

### TS-02: Pipeline Run (Data Gen)
**Pre-condition:** TS-01 has passed.
| Step | Action                                                              | Expected Result                                                                 | Status (Pass/Fail) |
| :--- | :------------------------------------------------------------------ | :------------------------------------------------------------------------------ | :----------------- |
| 2.1  | On the Dashboard (`/`), select Provider (e.g., `gemini`) and Model. | Dropdowns populate correctly.                                                   |                    |
| 2.2  | Click "Run Pipeline".                                               | Pipeline status appears in "Recent Runs". Status is "Running".                  |                    |
| 2.3  | Wait 1-2 minutes and click "Refresh".                               | Run ID completes successfully (Status: "Completed"). Success Rate increases.    |                    |
| 2.4  | Navigate to the "Vector DB" tab (`/vector`).                        | Qdrant stats show `ro_items` and `ro_monsters` have >0 vectors (data ingested). |                    |

### TS-03: Dashboard UI & Features
**Pre-condition:** Frontend `npm run dev` is active.
| Step | Action                                         | Expected Result                                                                                       | Status (Pass/Fail) |
| :--- | :--------------------------------------------- | :---------------------------------------------------------------------------------------------------- | :----------------- |
| 3.1  | Observe the top navigation bar.                | `[Dashboard] [Evaluations] [Playground] [Sources] [Quality Control] [Vector DB] [Users]` are visible. |                    |
| 3.2  | Click on the "Playground" tab.                 | Navigates to the Agent Playground without errors.                                                     |                    |
| 3.3  | Look at the "Settings" sidebar on the left.    | Under "Knowledge Base (Qdrant)", a green "Online" badge appears with Item DB/Mob DB counts.           |                    |
| 3.4  | Select "Mimir" from the Persona dropdown.      | Purple badge `⚔️ Actions: heal, buff` is displayed under his name.                                     |                    |
| 3.5  | Select "Sage Ariel" from the Persona dropdown. | Green badge `📚 RAG: item_db, mob_db` is displayed under her name.                                     |                    |

### TS-04: AI Playground (RAG & Actions)
**Pre-condition:** TS-02 (Data exists) and TS-03 have passed.
| Step | Action                                              | Expected Result                                                                                      | Status (Pass/Fail) |
| :--- | :-------------------------------------------------- | :--------------------------------------------------------------------------------------------------- | :----------------- |
| 4.1  | With "Mimir" selected, type: `ช่วย heal ฉันหน่อย`      | Mimir replies acknowledging the heal. The message block shows a green badge: `Action Invoked: heal`. |                    |
| 4.2  | Switch to "Sage Ariel". Type: `Poring ดรอปอะไรบ้าง?` | Ariel replies with accurate Poring drop data. A "Source" citation block appears below her response.  |                    |
| 4.3  | Click on the Source citation block.                 | A modal pops up showing the raw text data from the database.                                         |                    |

### TS-05: rAthena In-Game NPC Integration
**Pre-condition:** Backend API (Port 8080) is running. Game servers (`map`, `char`, `login`) are running.
| Step | Action                                                                     | Expected Result                                                                                                                 | Status (Pass/Fail) |
| :--- | :------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------ | :----------------- |
| 5.1  | Login to the game client using a GM account.                               | Successfully enter the game world.                                                                                              |                    |
| 5.2  | Type `@reloadscript` in the game chat.                                     | Server message confirms scripts are reloaded successfully.                                                                      |                    |
| 5.3  | Type `@go 0` to warp to Prontera, and walk to `150, 150` (center of town). | 4 AI NPCs are visible: Mimir, Sage Ariel, Fortune Teller Maya, Blacksmith Grumm.                                                |                    |
| 5.4  | Click on "Mimir AI" and type: `ขอรับบัพหน่อยครับ`                              | Mimir thinks for a moment, replies in the chat window, and your character receives Agi/Blessing buffs (Visual effects trigger). |                    |
| 5.5  | Click on "Sage Ariel" and type: `Jellopy เอาไว้ทำอะไร?`                      | Ariel fetches data from the Rust backend via HTTP and replies accurately based on the RAG system.                               |                    |

---

## Conclusion
If all steps TS-01 through TS-05 pass, the entire foundational architecture of Project Mimir (Phase 1-9) is intact and functioning flawlessly. You are then cleared to proceed to Phase 10 (AI GM).
