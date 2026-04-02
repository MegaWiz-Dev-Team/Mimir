use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::services::db::DbPool;

// ─── Structs ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NpcPersona {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub tier: i8,
    pub system_prompt: String,
    pub greeting: Option<String>,
    pub allowed_actions: Option<serde_json::Value>,
    pub personality_traits: Option<serde_json::Value>,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatSession {
    pub id: i64,
    pub session_id: String,
    pub persona_id: i32,
    pub player_char_id: i32,
    pub player_name: Option<String>,
    pub started_at: NaiveDateTime,
    pub last_message_at: NaiveDateTime,
    pub message_count: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tokens_used: i32,
    pub latency_ms: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActionLog {
    pub id: i64,
    pub session_id: Option<String>,
    pub persona_name: String,
    pub player_char_id: i32,
    pub action_type: String,
    pub action_params: Option<serde_json::Value>,
    pub result: String,
    pub denial_reason: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EconomyDaily {
    pub id: i64,
    pub date: chrono::NaiveDate,
    pub total_currency_given: i64,
    pub total_items_given: i32,
    pub total_custom_action_1: i32,
    pub total_custom_action_2: i32,
    pub max_currency_limit: i64,
    pub max_items_limit: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlayerDailyLimit {
    pub id: i64,
    pub player_char_id: i32,
    pub date: chrono::NaiveDate,
    pub chat_count: i32,
    pub custom_action_1_count: i32,
    pub custom_action_2_count: i32,
    pub items_received: i32,
    pub currency_received: i64,
    pub max_chat_limit: i32,
    pub max_custom_action_1_limit: i32,
    pub max_custom_action_2_limit: i32,
    pub max_items_limit: i32,
    pub max_currency_limit: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ─── CRUD Functions ────────────────────────────────────────

/// Get a persona by its unique name
pub async fn get_persona(db: &DbPool, name: &str) -> anyhow::Result<Option<NpcPersona>> {
    let persona = sqlx::query_as::<_, NpcPersona>(
        "SELECT * FROM ai_npc_persona WHERE name = ? AND is_active = TRUE",
    )
    .bind(name)
    .fetch_optional(db)
    .await?;
    Ok(persona)
}

/// List all active personas
pub async fn list_personas(db: &DbPool) -> anyhow::Result<Vec<NpcPersona>> {
    let personas = sqlx::query_as::<_, NpcPersona>(
        "SELECT * FROM ai_npc_persona WHERE is_active = TRUE ORDER BY tier, name",
    )
    .fetch_all(db)
    .await?;
    Ok(personas)
}

/// Create or get an existing chat session
pub async fn get_or_create_session(
    db: &DbPool,
    session_id: &str,
    persona_id: i32,
    player_char_id: i32,
    player_name: Option<&str>,
) -> anyhow::Result<ChatSession> {
    // Try to find existing active session
    let existing = sqlx::query_as::<_, ChatSession>(
        "SELECT * FROM ai_chat_session WHERE session_id = ? AND is_active = TRUE",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await?;

    if let Some(session) = existing {
        return Ok(session);
    }

    // Create new session
    sqlx::query(
        "INSERT INTO ai_chat_session (session_id, persona_id, player_char_id, player_name) VALUES (?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(persona_id)
    .bind(player_char_id)
    .bind(player_name)
    .execute(db)
    .await?;

    let session =
        sqlx::query_as::<_, ChatSession>("SELECT * FROM ai_chat_session WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(db)
            .await?;

    Ok(session)
}

/// Append a message to a chat session
pub async fn append_message(
    db: &DbPool,
    session_id: &str,
    role: &str,
    content: &str,
    tokens_used: i32,
    latency_ms: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO ai_chat_message (session_id, role, content, tokens_used, latency_ms) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(tokens_used)
    .bind(latency_ms)
    .execute(db)
    .await?;

    // Update session message count
    sqlx::query(
        "UPDATE ai_chat_session SET message_count = message_count + 1 WHERE session_id = ?",
    )
    .bind(session_id)
    .execute(db)
    .await?;

    Ok(())
}

/// Get recent messages for context window
pub async fn get_recent_messages(
    db: &DbPool,
    session_id: &str,
    limit: i32,
) -> anyhow::Result<Vec<ChatMessage>> {
    let messages = sqlx::query_as::<_, ChatMessage>(
        "SELECT * FROM (
            SELECT * FROM ai_chat_message WHERE session_id = ? ORDER BY id DESC LIMIT ?
        ) sub ORDER BY id ASC",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(messages)
}

/// Log an AI action
pub async fn log_action(
    db: &DbPool,
    session_id: Option<&str>,
    persona_name: &str,
    player_char_id: i32,
    action_type: &str,
    action_params: Option<serde_json::Value>,
    result: &str,
    denial_reason: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO ai_action_log (session_id, persona_name, player_char_id, action_type, action_params, result, denial_reason)
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(persona_name)
    .bind(player_char_id)
    .bind(action_type)
    .bind(action_params.map(|v| v.to_string()))
    .bind(result)
    .bind(denial_reason)
    .execute(db)
    .await?;
    Ok(())
}

/// Check if the server-wide economy limit has been reached today
pub async fn check_economy_limit(db: &DbPool, action_type: &str) -> anyhow::Result<bool> {
    let today = chrono::Local::now().date_naive();

    // Ensure today's record exists
    sqlx::query("INSERT IGNORE INTO ai_economy_daily (date) VALUES (?)")
        .bind(today)
        .execute(db)
        .await?;

    let row = sqlx::query_as::<_, EconomyDaily>("SELECT * FROM ai_economy_daily WHERE date = ?")
        .bind(today)
        .fetch_one(db)
        .await?;

    let allowed = match action_type {
        "give_item" => row.total_items_given < row.max_items_limit,
        "give_currency" => row.total_currency_given < row.max_currency_limit,
        "custom_action_1" => row.total_custom_action_1 < 10000, // generous server-wide limit
        "custom_action_2" => row.total_custom_action_2 < 5000,
        _ => true,
    };

    Ok(allowed)
}

/// Increment economy counters for a given action
pub async fn increment_economy(db: &DbPool, action_type: &str, amount: i64) -> anyhow::Result<()> {
    let today = chrono::Local::now().date_naive();
    let field = match action_type {
        "give_item" => "total_items_given",
        "give_currency" => "total_currency_given",
        "custom_action_1" => "total_custom_action_1",
        "custom_action_2" => "total_custom_action_2",
        _ => return Ok(()),
    };

    let query = format!(
        "UPDATE ai_economy_daily SET {} = {} + ? WHERE date = ?",
        field, field
    );
    sqlx::query(&query)
        .bind(amount)
        .bind(today)
        .execute(db)
        .await?;
    Ok(())
}

/// Check if a player has exceeded their daily limit for a given action
pub async fn check_player_limit(
    db: &DbPool,
    player_char_id: i32,
    action_type: &str,
) -> anyhow::Result<bool> {
    let today = chrono::Local::now().date_naive();

    // Ensure today's record exists
    sqlx::query("INSERT IGNORE INTO ai_player_daily_limits (player_char_id, date) VALUES (?, ?)")
        .bind(player_char_id)
        .bind(today)
        .execute(db)
        .await?;

    let row = sqlx::query_as::<_, PlayerDailyLimit>(
        "SELECT * FROM ai_player_daily_limits WHERE player_char_id = ? AND date = ?",
    )
    .bind(player_char_id)
    .bind(today)
    .fetch_one(db)
    .await?;

    let allowed = match action_type {
        "chat" => row.chat_count < row.max_chat_limit,
        "custom_action_1" => row.custom_action_1_count < row.max_custom_action_1_limit,
        "custom_action_2" => row.custom_action_2_count < row.max_custom_action_2_limit,
        "give_item" => row.items_received < row.max_items_limit,
        "give_currency" => row.currency_received < row.max_currency_limit,
        _ => true,
    };

    Ok(allowed)
}

/// Increment a player's daily usage counter
pub async fn increment_player_usage(
    db: &DbPool,
    player_char_id: i32,
    action_type: &str,
    amount: i64,
) -> anyhow::Result<()> {
    let today = chrono::Local::now().date_naive();
    let field = match action_type {
        "chat" => "chat_count",
        "custom_action_1" => "custom_action_1_count",
        "custom_action_2" => "custom_action_2_count",
        "give_item" => "items_received",
        "give_currency" => "currency_received",
        _ => return Ok(()),
    };

    let query = format!(
        "UPDATE ai_player_daily_limits SET {} = {} + ? WHERE player_char_id = ? AND date = ?",
        field, field
    );
    sqlx::query(&query)
        .bind(amount)
        .bind(player_char_id)
        .bind(today)
        .execute(db)
        .await?;
    Ok(())
}
