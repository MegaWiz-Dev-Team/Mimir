-- Rollback: 202602190000_ai_tables.sql
-- Drops AI layer tables

DROP TABLE IF EXISTS ai_player_daily_limits;
DROP TABLE IF EXISTS ai_economy_daily;
DROP TABLE IF EXISTS ai_action_log;
DROP TABLE IF EXISTS ai_chat_message;
DROP TABLE IF EXISTS ai_chat_session;
DROP TABLE IF EXISTS ai_npc_persona;
