-- AI Layer Tables Migration
-- Phase 1: Infrastructure for NPC AI System
-- Does NOT touch rAthena tables

-- NPC Persona Configuration
CREATE TABLE IF NOT EXISTS ai_npc_persona (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(50) NOT NULL UNIQUE,
    display_name VARCHAR(100) NOT NULL,
    tier TINYINT NOT NULL DEFAULT 1 COMMENT '1=Simple, 2=RAG Oracle',
    system_prompt TEXT NOT NULL,
    greeting TEXT,
    allowed_actions JSON COMMENT '["custom_action_1","custom_action_2","give_item"]',
    personality_traits JSON COMMENT '{"formality":0.8,"humor":0.3}',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_name (name),
    INDEX idx_tier (tier)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Chat Session (Conversation Memory)
CREATE TABLE IF NOT EXISTS ai_chat_session (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL COMMENT 'UUID per conversation',
    persona_id INT NOT NULL,
    player_char_id INT NOT NULL COMMENT 'rAthena char.char_id',
    player_name VARCHAR(30),
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_message_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    message_count INT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    FOREIGN KEY (persona_id) REFERENCES ai_npc_persona(id) ON DELETE CASCADE,
    INDEX idx_session (session_id),
    INDEX idx_player (player_char_id),
    INDEX idx_persona (persona_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Chat Messages (Individual messages in a session)
CREATE TABLE IF NOT EXISTS ai_chat_message (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    role VARCHAR(10) NOT NULL COMMENT 'user or assistant',
    content TEXT NOT NULL,
    tokens_used INT DEFAULT 0,
    latency_ms INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_session (session_id),
    INDEX idx_created (created_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Action Audit Log
CREATE TABLE IF NOT EXISTS ai_action_log (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    session_id VARCHAR(36),
    persona_name VARCHAR(50) NOT NULL,
    player_char_id INT NOT NULL,
    action_type VARCHAR(30) NOT NULL COMMENT 'custom_action_1, custom_action_2, give_item, warp, info',
    action_params JSON COMMENT '{"item_id":501,"amount":5}',
    result VARCHAR(20) NOT NULL DEFAULT 'SUCCESS' COMMENT 'SUCCESS, DENIED, LIMIT_REACHED, ERROR',
    denial_reason TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_player (player_char_id),
    INDEX idx_action (action_type),
    INDEX idx_created (created_at),
    INDEX idx_result (result)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Server-wide Economy Daily Limits
CREATE TABLE IF NOT EXISTS ai_economy_daily (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    date DATE NOT NULL,
    total_currency_given BIGINT DEFAULT 0,
    total_items_given INT DEFAULT 0,
    total_custom_action_1 INT DEFAULT 0,
    total_custom_action_2 INT DEFAULT 0,
    max_currency_limit BIGINT DEFAULT 1000000 COMMENT 'Daily server-wide currency cap',
    max_items_limit INT DEFAULT 500 COMMENT 'Daily server-wide item cap',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_date (date)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Per-player Daily Limits
CREATE TABLE IF NOT EXISTS ai_player_daily_limits (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    player_char_id INT NOT NULL,
    date DATE NOT NULL,
    chat_count INT DEFAULT 0,
    custom_action_1_count INT DEFAULT 0,
    custom_action_2_count INT DEFAULT 0,
    items_received INT DEFAULT 0,
    currency_received BIGINT DEFAULT 0,
    max_chat_limit INT DEFAULT 50 COMMENT 'Max chats per player per day',
    max_custom_action_1_limit INT DEFAULT 10,
    max_custom_action_2_limit INT DEFAULT 5,
    max_items_limit INT DEFAULT 10,
    max_currency_limit BIGINT DEFAULT 50000,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_player_date (player_char_id, date),
    INDEX idx_player (player_char_id),
    INDEX idx_date (date)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
