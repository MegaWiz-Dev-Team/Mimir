//! Oracle RAG Agent - Tier 2: RAG Agent with rig-core + Qdrant
//!
//! This agent provides enhanced NPC capabilities with:
//! - Retrieval Augmented Generation (RAG) from wiki_qa and game_data collections
//! - Custom tools for direct rAthena database queries (mobs, items)
//! - Confidence scoring and source citation in responses
//! - Support for multiple LLM providers (Ollama local, Gemini cloud)
//!
//! ## Architecture
//! ```text
//! User Query → Query Analysis → RAG Retrieval → Tool Execution (if needed)
//!                    ↓                ↓                    ↓
//!              Intent Detection   Qdrant Search      DB Query Tools
//!                    ↓                ↓                    ↓
//!                    └────────────────┴────────────────────┘
//!                                      ↓
//!                              Context Assembly
//!                                      ↓
//!                              LLM Generation (Ollama/Gemini)
//!                                      ↓
//!                         Response + Confidence + Citations
//! ```

use anyhow::Result;
use rig::providers::ollama;
use rig::providers::gemini;
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::env;

use crate::models::persona::Persona;
use crate::services::qdrant::QdrantService;
use crate::services::db::DbPool;
use sqlx::FromRow;

// ─── Constants ─────────────────────────────────────────────────────────────

/// Default model for Tier 2 (more capable for RAG tasks)
const DEFAULT_MODEL: &str = "llama3.2";

/// Default Gemini model
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

/// Default timeout for completion requests (45 seconds for RAG operations)
const DEFAULT_TIMEOUT_SECS: u64 = 45;

// ─── LLM Provider ───────────────────────────────────────────────────────────

/// Supported LLM providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Ollama,
    Gemini,
}

impl Default for LlmProvider {
    fn default() -> Self {
        LlmProvider::Ollama
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Ollama => write!(f, "ollama"),
            LlmProvider::Gemini => write!(f, "gemini"),
        }
    }
}

impl std::str::FromStr for LlmProvider {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ollama" | "local" => Ok(LlmProvider::Ollama),
            "gemini" | "google" => Ok(LlmProvider::Gemini),
            _ => Err(format!("Unknown provider: {}", s))
        }
    }
}

/// Qdrant collection names
pub const COLLECTION_WIKI_QA: &str = "wiki_qa";
pub const COLLECTION_GAME_DATA: &str = "game_data";

/// Minimum confidence threshold for high-confidence responses
pub const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.75;
pub const MEDIUM_CONFIDENCE_THRESHOLD: f32 = 0.50;

// ─── Response Types ────────────────────────────────────────────────────────

/// Source citation for RAG responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCitation {
    /// Source type (wiki, mob_db, item_db, game_data)
    pub source_type: String,
    /// Source identifier (name, ID, or URL)
    pub source_id: String,
    /// Relevance score (0.0 - 1.0)
    pub relevance: f32,
    /// Snippet of retrieved content
    pub snippet: String,
}

/// Confidence level for responses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    Unknown,
}

impl From<f32> for ConfidenceLevel {
    fn from(score: f32) -> Self {
        if score >= HIGH_CONFIDENCE_THRESHOLD {
            ConfidenceLevel::High
        } else if score >= MEDIUM_CONFIDENCE_THRESHOLD {
            ConfidenceLevel::Medium
        } else if score > 0.0 {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::Unknown
        }
    }
}

/// RAG response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleResponse {
    /// The generated response text
    pub content: String,
    /// Overall confidence score (0.0 - 1.0)
    pub confidence_score: f32,
    /// Confidence level classification
    pub confidence_level: ConfidenceLevel,
    /// Sources used to generate the response
    pub sources: Vec<SourceCitation>,
    /// Whether tools were used
    pub tools_used: Vec<String>,
    /// Latency in milliseconds
    pub latency_ms: u64,
}

// ─── Database Models ───────────────────────────────────────────────────────

/// Mob data from rAthena database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MobData {
    pub id: u32,
    pub name_aegis: String,
    pub name_english: String,
    pub level: u32,
    pub hp: u64,
    pub sp: u32,
    pub base_exp: u64,
    pub job_exp: u64,
    pub attack: u32,
    pub defense: u32,
    pub magic_defense: u32,
    pub str: u32,
    pub agi: u32,
    pub vit: u32,
    pub int: u32,
    pub dex: u32,
    pub luk: u32,
    pub size: String,
    pub race: String,
    pub element: String,
    pub element_level: u32,
}

/// Item data from rAthena database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ItemData {
    pub id: u32,
    pub name_aegis: String,
    pub name_english: String,
    pub item_type: String,
    pub subtype: Option<String>,
    pub attack: u32,
    pub magic_attack: u32,
    pub defense: u32,
    pub weight: u32,
    pub slots: u32,
    pub weapon_level: u32,
    pub armor_level: u32,
    pub equip_level_min: u32,
    pub price_buy: u64,
    pub price_sell: u64,
    pub refineable: bool,
}

// ─── Custom Tools ──────────────────────────────────────────────────────────

/// Tool for querying mob database
pub struct QueryMobDbTool {
    db_pool: DbPool,
}

impl QueryMobDbTool {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    /// Query mob by name (partial match)
    pub async fn query_by_name(&self, name: &str) -> Result<Vec<MobData>> {
        let pattern = format!("%{}%", name);
        let mobs = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db 
            WHERE name_english LIKE ? OR name_aegis LIKE ?
            LIMIT 10"#
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(mobs)
    }

    /// Query mob by ID
    pub async fn query_by_id(&self, id: u32) -> Result<Option<MobData>> {
        let mob = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await?;
        
        Ok(mob)
    }

    /// Query mobs by level range
    pub async fn query_by_level_range(&self, min_level: u32, max_level: u32) -> Result<Vec<MobData>> {
        let mobs = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db 
            WHERE level BETWEEN ? AND ?
            ORDER BY level, base_exp DESC
            LIMIT 20"#
        )
        .bind(min_level)
        .bind(max_level)
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(mobs)
    }

    /// Format mob data as human-readable text
    pub fn format_mob(mob: &MobData) -> String {
        format!(
            "**{}** (ID: {})\n\
            - Level: {} | HP: {} | SP: {}\n\
            - ATK: {} | DEF: {} | MDEF: {}\n\
            - EXP: Base {} / Job {}\n\
            - Size: {} | Race: {} | Element: {} Lv{}\n\
            - Stats: STR {} AGI {} VIT {} INT {} DEX {} LUK {}",
            mob.name_english,
            mob.id,
            mob.level,
            mob.hp,
            mob.sp,
            mob.attack,
            mob.defense,
            mob.magic_defense,
            mob.base_exp,
            mob.job_exp,
            mob.size,
            mob.race,
            mob.element,
            mob.element_level,
            mob.str,
            mob.agi,
            mob.vit,
            mob.int,
            mob.dex,
            mob.luk
        )
    }
}

/// Tool for querying item database
pub struct QueryItemDbTool {
    db_pool: DbPool,
}

impl QueryItemDbTool {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    /// Query item by name (partial match)
    pub async fn query_by_name(&self, name: &str) -> Result<Vec<ItemData>> {
        let pattern = format!("%{}%", name);
        let items = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db 
            WHERE name_english LIKE ? OR name_aegis LIKE ?
            LIMIT 10"#
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(items)
    }

    /// Query item by ID
    pub async fn query_by_id(&self, id: u32) -> Result<Option<ItemData>> {
        let item = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await?;
        
        Ok(item)
    }

    /// Query items by type
    pub async fn query_by_type(&self, item_type: &str) -> Result<Vec<ItemData>> {
        let items = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db 
            WHERE item_type = ?
            ORDER BY name_english
            LIMIT 50"#
        )
        .bind(item_type)
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(items)
    }

    /// Format item data as human-readable text
    pub fn format_item(item: &ItemData) -> String {
        let mut parts = vec![format!("**{}** (ID: {})", item.name_english, item.id)];
        
        parts.push(format!("- Type: {}", item.item_type));
        
        if let Some(ref subtype) = item.subtype {
            parts.push(format!("  Subtype: {}", subtype));
        }
        
        if item.attack > 0 {
            parts.push(format!("- ATK: {}", item.attack));
        }
        if item.magic_attack > 0 {
            parts.push(format!("- MATK: {}", item.magic_attack));
        }
        if item.defense > 0 {
            parts.push(format!("- DEF: {}", item.defense));
        }
        
        parts.push(format!("- Weight: {:.1}", item.weight as f32 / 10.0));
        
        if item.slots > 0 {
            parts.push(format!("- Slots: {}", item.slots));
        }
        
        if item.price_buy > 0 {
            parts.push(format!("- Buy Price: {} zeny", item.price_buy));
        }
        if item.price_sell > 0 {
            parts.push(format!("- Sell Price: {} zeny", item.price_sell));
        }
        
        if item.weapon_level > 0 {
            parts.push(format!("- Weapon Level: {}", item.weapon_level));
        }
        if item.armor_level > 0 {
            parts.push(format!("- Armor Level: {}", item.armor_level));
        }
        if item.equip_level_min > 0 {
            parts.push(format!("- Required Level: {}", item.equip_level_min));
        }
        
        if item.refineable {
            parts.push("- Refineable: Yes".to_string());
        }
        
        parts.join("\n")
    }
}

// ─── RAG Retriever ─────────────────────────────────────────────────────────

/// RAG retriever for Qdrant vector search
pub struct RagRetriever {
    qdrant: QdrantService,
    ollama_url: String,
    embed_model: String,
}

impl RagRetriever {
    pub fn new(qdrant: QdrantService) -> Self {
        let ollama_url = env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let embed_model = env::var("EMBED_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text".to_string());
        
        Self {
            qdrant,
            ollama_url,
            embed_model,
        }
    }

    /// Get embedding for text using Ollama
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        
        #[derive(serde::Serialize)]
        struct EmbedRequest {
            model: String,
            input: Vec<String>,
        }
        
        #[derive(serde::Deserialize)]
        struct EmbedResponse {
            embeddings: Vec<Vec<f32>>,
        }
        
        let req = EmbedRequest {
            model: self.embed_model.clone(),
            input: vec![text.to_string()],
        };
        
        let resp = client
            .post(format!("{}/api/embed", self.ollama_url))
            .json(&req)
            .send()
            .await?;
        
        if !resp.status().is_success() {
            let err = resp.text().await?;
            return Err(anyhow::anyhow!("Ollama embed error: {}", err));
        }
        
        let embed_resp: EmbedResponse = resp.json().await?;
        
        embed_resp.embeddings.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No embedding returned"))
    }

    /// Search wiki_qa collection
    pub async fn search_wiki(&self, query: &str, limit: usize) -> Result<Vec<SourceCitation>> {
        let vector = self.get_embedding(query).await?;
        
        let results = self.qdrant.search(COLLECTION_WIKI_QA, vector, limit).await?;
        
        let citations = Self::parse_search_results(results, "wiki")?;
        Ok(citations)
    }

    /// Search game_data collection
    pub async fn search_game_data(&self, query: &str, limit: usize) -> Result<Vec<SourceCitation>> {
        let vector = self.get_embedding(query).await?;
        
        let results = self.qdrant.search(COLLECTION_GAME_DATA, vector, limit).await?;
        
        let citations = Self::parse_search_results(results, "game_data")?;
        Ok(citations)
    }

    /// Search all collections
    pub async fn search_all(&self, query: &str, limit_per_collection: usize) -> Result<Vec<SourceCitation>> {
        let mut all_citations = Vec::new();
        
        // Search wiki_qa
        match self.search_wiki(query, limit_per_collection).await {
            Ok(citations) => all_citations.extend(citations),
            Err(e) => tracing::warn!("Wiki search failed: {}", e),
        }
        
        // Search game_data
        match self.search_game_data(query, limit_per_collection).await {
            Ok(citations) => all_citations.extend(citations),
            Err(e) => tracing::warn!("Game data search failed: {}", e),
        }
        
        // Sort by relevance
        all_citations.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(all_citations)
    }

    /// Parse Qdrant search results into citations
    fn parse_search_results(results: serde_json::Value, source_type: &str) -> Result<Vec<SourceCitation>> {
        let mut citations = Vec::new();
        
        if let Some(results_array) = results.get("result").and_then(|r| r.as_array()) {
            for result in results_array {
                let score = result.get("score")
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) as f32;
                
                let payload = result.get("payload");
                
                let source_id = payload
                    .and_then(|p| p.get("source_id"))
                    .or_else(|| payload.and_then(|p| p.get("source"))) // Fallback for wiki indexer
                    .or_else(|| payload.and_then(|p| p.get("name")))
                    .or_else(|| payload.and_then(|p| p.get("id")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let snippet = payload
                    .and_then(|p| p.get("answer")) // Prefer answer for Q/A
                    .or_else(|| payload.and_then(|p| p.get("content")))
                    .or_else(|| payload.and_then(|p| p.get("text")))
                    .or_else(|| payload.and_then(|p| p.get("question")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                // Truncate snippet if too long
                let snippet = if snippet.len() > 300 {
                    format!("{}...", &snippet[..300])
                } else {
                    snippet
                };
                
                citations.push(SourceCitation {
                    source_type: source_type.to_string(),
                    source_id,
                    relevance: score,
                    snippet,
                });
            }
        }
        
        Ok(citations)
    }
}

// ─── Oracle RAG Agent ──────────────────────────────────────────────────────

/// Agent backend enum to support multiple LLM providers
pub enum AgentBackend {
    Ollama(rig::agent::Agent<ollama::CompletionModel>),
    Gemini(rig::agent::Agent<gemini::completion::CompletionModel>),
}

impl AgentBackend {
    /// Send a prompt to the underlying LLM
    pub async fn prompt(&self, message: &str) -> Result<String> {
        match self {
            AgentBackend::Ollama(agent) => {
                agent.prompt(message).await
                    .map_err(|e| anyhow::anyhow!("Ollama prompt failed: {}", e))
            }
            AgentBackend::Gemini(agent) => {
                agent.prompt(message).await
                    .map_err(|e| anyhow::anyhow!("Gemini prompt failed: {}", e))
            }
        }
    }
}

/// Oracle RAG Agent - Tier 2 NPC with RAG capabilities
pub struct OracleRagAgent {
    pub persona: Persona,
    pub provider: LlmProvider,
    pub model_name: String,
    pub timeout: Duration,
    agent: AgentBackend,
    retriever: RagRetriever,
    mob_tool: Option<QueryMobDbTool>,
    item_tool: Option<QueryItemDbTool>,
}

impl OracleRagAgent {
    /// Create a new OracleRagAgent with RAG capabilities (default: Ollama)
    pub fn new(
        persona: Persona,
        qdrant: QdrantService,
        db_pool: Option<DbPool>,
    ) -> Self {
        Self::with_provider(persona, qdrant, db_pool, LlmProvider::Ollama, None, None)
    }

    /// Create an OracleRagAgent with custom options (legacy, uses Ollama)
    pub fn with_options(
        persona: Persona,
        qdrant: QdrantService,
        db_pool: Option<DbPool>,
        model: Option<&str>,
        timeout: Option<Duration>,
    ) -> Self {
        Self::with_provider(persona, qdrant, db_pool, LlmProvider::Ollama, model, timeout)
    }

    /// Create an OracleRagAgent with a specific provider
    pub fn with_provider(
        persona: Persona,
        qdrant: QdrantService,
        db_pool: Option<DbPool>,
        provider: LlmProvider,
        model: Option<&str>,
        timeout: Option<Duration>,
    ) -> Self {
        let timeout = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        
        // Build enhanced system prompt with RAG context instructions
        let enhanced_prompt = Self::build_enhanced_prompt(&persona);
        
        // Create agent based on provider
        let (agent, model_name) = match provider {
            LlmProvider::Ollama => {
                let client = ollama::Client::new();
                let model_name = model.unwrap_or(DEFAULT_MODEL).to_string();
                let agent = client.agent(&model_name)
                    .preamble(&enhanced_prompt)
                    .build();
                (AgentBackend::Ollama(agent), model_name)
            }
            LlmProvider::Gemini => {
                let api_key = env::var("GEMINI_API_KEY")
                    .or_else(|_| env::var("GOOGLE_API_KEY"))
                    .expect("GEMINI_API_KEY or GOOGLE_API_KEY must be set for Gemini provider");
                let client = gemini::Client::new(&api_key);
                let model_name = model.unwrap_or(DEFAULT_GEMINI_MODEL).to_string();
                let agent = client.agent(&model_name)
                    .preamble(&enhanced_prompt)
                    .build();
                (AgentBackend::Gemini(agent), model_name)
            }
        };
        
        let retriever = RagRetriever::new(qdrant);
        
        let mob_tool = db_pool.as_ref().map(|pool| QueryMobDbTool::new(pool.clone()));
        let item_tool = db_pool.as_ref().map(|pool| QueryItemDbTool::new(pool.clone()));
        
        Self {
            persona,
            provider,
            model_name,
            timeout,
            agent,
            retriever,
            mob_tool,
            item_tool,
        }
    }

    /// Build enhanced system prompt with RAG instructions
    fn build_enhanced_prompt(persona: &Persona) -> String {
        format!(
            r#"{}

## RAG Capabilities
You have access to a knowledge base containing:
- Wiki articles and Q&A about Ragnarok Online
- Game data including monster and item databases

## Response Guidelines
1. When answering questions, use the provided context from the knowledge base
2. If you're uncertain, acknowledge the limitation
3. Cite your sources when providing specific game data
4. For monster/item queries, provide accurate stats from the database
5. Keep responses informative but concise

## Personality Traits
{}"#,
            persona.system_prompt,
            persona.personality_traits.join(", ")
        )
    }

    /// Chat with RAG retrieval
    pub async fn chat(&self, message: &str) -> Result<OracleResponse> {
        let start = std::time::Instant::now();
        let mut tools_used = Vec::new();
        let mut all_sources = Vec::new();
        
        // Step 1: Retrieve relevant context from RAG
        let rag_sources = self.retriever.search_all(message, 3).await?;
        all_sources.extend(rag_sources);
        
        // Step 2: Check if we need to query databases directly
        let db_context = self.query_databases(message, &mut tools_used).await?;
        
        // Step 3: Build context-enhanced prompt
        let context = self.build_context(&all_sources, &db_context);
        let enhanced_message = format!(
            "Context from knowledge base:\n{}\n\nUser question: {}",
            context,
            message
        );
        
        // Step 4: Generate response using the agent backend
        let response = tokio::time::timeout(
            self.timeout,
            self.agent.prompt(enhanced_message.as_str())
        )
        .await
        .map_err(|_| anyhow::anyhow!("Request timeout after {}s", self.timeout.as_secs()))??;
        
        let latency_ms = start.elapsed().as_millis() as u64;
        
        // Step 5: Calculate confidence score
        let confidence_score = self.calculate_confidence(&all_sources, &tools_used);
        let confidence_level = ConfidenceLevel::from(confidence_score);
        
        Ok(OracleResponse {
            content: response,
            confidence_score,
            confidence_level,
            sources: all_sources,
            tools_used,
            latency_ms,
        })
    }

    /// Query databases based on message content
    async fn query_databases(&self, message: &str, tools_used: &mut Vec<String>) -> Result<String> {
        let mut context = String::new();
        let message_lower = message.to_lowercase();
        
        // Detect mob-related queries
        if let Some(ref mob_tool) = self.mob_tool {
            if self.is_mob_query(&message_lower) {
                // Try to extract mob name from message
                if let Some(mob_name) = self.extract_entity_name(message) {
                    let mobs = mob_tool.query_by_name(&mob_name).await?;
                    if !mobs.is_empty() {
                        tools_used.push("QueryMobDbTool".to_string());
                        context.push_str("\n### Monster Data:\n");
                        for mob in mobs.iter().take(3) {
                            context.push_str(&QueryMobDbTool::format_mob(mob));
                            context.push_str("\n\n");
                        }
                    }
                }
            }
        }
        
        // Detect item-related queries
        if let Some(ref item_tool) = self.item_tool {
            if self.is_item_query(&message_lower) {
                if let Some(item_name) = self.extract_entity_name(message) {
                    let items = item_tool.query_by_name(&item_name).await?;
                    if !items.is_empty() {
                        tools_used.push("QueryItemDbTool".to_string());
                        context.push_str("\n### Item Data:\n");
                        for item in items.iter().take(3) {
                            context.push_str(&QueryItemDbTool::format_item(item));
                            context.push_str("\n\n");
                        }
                    }
                }
            }
        }
        
        Ok(context)
    }

    /// Check if message is asking about monsters
    fn is_mob_query(&self, message: &str) -> bool {
        let keywords = ["monster", "mob", "monster", "enemy", "creature", "boss", "mvp"];
        keywords.iter().any(|k| message.contains(k))
    }

    /// Check if message is asking about items
    fn is_item_query(&self, message: &str) -> bool {
        let keywords = ["item", "weapon", "armor", "equipment", "card", "drop", "loot"];
        keywords.iter().any(|k| message.contains(k))
    }

    /// Extract entity name from message (simple heuristic)
    fn extract_entity_name(&self, message: &str) -> Option<String> {
        // Simple extraction: look for quoted strings or capitalized words
        let words: Vec<&str> = message.split_whitespace().collect();
        
        // Look for capitalized words (potential names)
        let name_words: Vec<&str> = words
            .iter()
            .filter(|w| {
                let first_char = w.chars().next().unwrap_or(' ');
                first_char.is_uppercase() && w.len() > 2
            })
            .cloned()
            .collect();
        
        if !name_words.is_empty() {
            Some(name_words.join(" "))
        } else {
            None
        }
    }

    /// Build context string from sources
    fn build_context(&self, sources: &[SourceCitation], db_context: &str) -> String {
        let mut context = String::new();
        
        // Add RAG sources
        for (i, source) in sources.iter().enumerate() {
            context.push_str(&format!(
                "[{}] {} (relevance: {:.2})\n{}\n\n",
                i + 1,
                source.source_id,
                source.relevance,
                source.snippet
            ));
        }
        
        // Add database context
        if !db_context.is_empty() {
            context.push_str(db_context);
        }
        
        if context.is_empty() {
            context = "No relevant context found in knowledge base.".to_string();
        }
        
        context
    }

    /// Calculate confidence score based on sources and tools used
    fn calculate_confidence(&self, sources: &[SourceCitation], tools_used: &[String]) -> f32 {
        if sources.is_empty() && tools_used.is_empty() {
            return 0.2; // Low confidence - no external data
        }
        
        let mut score = 0.0;
        
        // Factor in source relevance
        if !sources.is_empty() {
            let avg_relevance: f32 = sources.iter()
                .map(|s| s.relevance)
                .sum::<f32>() / sources.len() as f32;
            score += avg_relevance * 0.5;
        }
        
        // Factor in tool usage (direct DB queries are more reliable)
        if !tools_used.is_empty() {
            score += 0.3;
        }
        
        // Bonus for multiple sources
        if sources.len() >= 3 {
            score += 0.1;
        }
        
        // Cap at 1.0
        score.min(1.0)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_persona() -> Persona {
        Persona {
            name: "oracle_test".to_string(),
            display_name: "Oracle Test".to_string(),
            tier: 2,
            system_prompt: "You are an oracle with access to game knowledge.".to_string(),
            greeting: Some("Greetings, seeker of knowledge.".to_string()),
            allowed_actions: vec!["query_mob".to_string(), "query_item".to_string()],
            personality_traits: vec!["wise".to_string(), "knowledgeable".to_string()],
        }
    }

    #[test]
    fn test_confidence_level_from_score() {
        assert_eq!(ConfidenceLevel::from(0.8), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from(0.75), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from(0.6), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from(0.5), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from(0.3), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from(0.0), ConfidenceLevel::Unknown);
    }

    #[test]
    fn test_is_mob_query() {
        let persona = create_test_persona();
        let qdrant = QdrantService::new();
        let agent = OracleRagAgent::new(persona, qdrant, None);
        
        assert!(agent.is_mob_query("tell me about the poring monster"));
        assert!(agent.is_mob_query("what mobs drop this item?"));
        assert!(agent.is_mob_query("boss information"));
        assert!(!agent.is_mob_query("what is the best weapon?"));
    }

    #[test]
    fn test_is_item_query() {
        let persona = create_test_persona();
        let qdrant = QdrantService::new();
        let agent = OracleRagAgent::new(persona, qdrant, None);
        
        assert!(agent.is_item_query("what does this item do?"));
        assert!(agent.is_item_query("best weapon for knight"));
        assert!(agent.is_item_query("armor recommendations"));
        assert!(!agent.is_item_query("where to level up?"));
    }

    #[test]
    fn test_extract_entity_name() {
        let persona = create_test_persona();
        let qdrant = QdrantService::new();
        let agent = OracleRagAgent::new(persona, qdrant, None);
        
        // The function extracts capitalized words, so "Tell" and "Poring" are both captured
        let name = agent.extract_entity_name("Tell me about Poring monster");
        assert!(name.is_some());
        assert!(name.unwrap().contains("Poring"));
        
        let name = agent.extract_entity_name("what is the best weapon?");
        assert_eq!(name, None);
    }

    #[test]
    fn test_format_mob() {
        let mob = MobData {
            id: 1002,
            name_aegis: "PORING".to_string(),
            name_english: "Poring".to_string(),
            level: 1,
            hp: 50,
            sp: 0,
            base_exp: 2,
            job_exp: 1,
            attack: 7,
            defense: 0,
            magic_defense: 5,
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 6,
            luk: 5,
            size: "Small".to_string(),
            race: "Plant".to_string(),
            element: "Water".to_string(),
            element_level: 1,
        };
        
        let formatted = QueryMobDbTool::format_mob(&mob);
        assert!(formatted.contains("Poring"));
        assert!(formatted.contains("Level: 1"));
        assert!(formatted.contains("HP: 50"));
    }

    #[test]
    fn test_format_item() {
        let item = ItemData {
            id: 1201,
            name_aegis: "Knife".to_string(),
            name_english: "Knife".to_string(),
            item_type: "Weapon".to_string(),
            subtype: Some("Dagger".to_string()),
            attack: 17,
            magic_attack: 0,
            defense: 0,
            weight: 400,
            slots: 0,
            weapon_level: 1,
            armor_level: 0,
            equip_level_min: 1,
            price_buy: 50,
            price_sell: 25,
            refineable: true,
        };
        
        let formatted = QueryItemDbTool::format_item(&item);
        assert!(formatted.contains("Knife"));
        assert!(formatted.contains("ATK: 17"));
        assert!(formatted.contains("Weapon"));
    }
}
