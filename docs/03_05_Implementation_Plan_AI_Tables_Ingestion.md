# Implementation Plan: AI Tables & Game Data Ingestion

Phase 1 remaining tasks: AI database schema and rAthena → Qdrant ingestion.

## Part 1: AI Tables Migration

5 new tables in MariaDB (does NOT touch rAthena tables):

| Table                    | Purpose                                                          |
| ------------------------ | ---------------------------------------------------------------- |
| `ai_npc_persona`         | NPC persona configs (name, system prompt, tier, allowed actions) |
| `ai_chat_session`        | Conversation history for NPC memory                              |
| `ai_action_log`          | Audit trail for every AI action (heal/buff/give)                 |
| `ai_economy_daily`       | Server-wide daily economy limits                                 |
| `ai_player_daily_limits` | Per-player daily interaction limits                              |

**Files:**
- `migrations/202602190000_ai_tables.sql`
- `src/models/ai.rs` — Structs + CRUD functions
- `src/models/mod.rs` — Add `pub mod ai`
- `src/lib.rs` — Add `pub mod models`

## Part 2: Game Data Ingestion (rAthena → Qdrant)

Binary `ingest_gamedata` that:
1. Reads `item_db` and `mob_db` from rAthena MariaDB
2. Builds text descriptions per row
3. Calls Ollama `/api/embed` for embeddings
4. Upserts into Qdrant `ro_items` and `ro_monsters`

**Files:**
- `src/bin/ingest_gamedata.rs`
- `src/services/qdrant.rs` — Add `delete_collection()`

## Verification

```bash
# AI Tables
docker exec -it mimir_mariadb mysql -u mimir -pmimir_pass mimir_db -e "SHOW TABLES LIKE 'ai_%';"

# Ingestion
cargo run --bin ingest_gamedata
curl http://localhost:6333/collections/ro_items | jq '.result.points_count'
curl http://localhost:6333/collections/ro_monsters | jq '.result.points_count'
```
