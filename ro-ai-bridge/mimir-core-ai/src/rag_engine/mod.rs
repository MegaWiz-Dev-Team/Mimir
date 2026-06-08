//! Oracle RAG Agent - Tier 2: RAG Agent with Qdrant
//!
//! This agent provides enhanced NPC capabilities with:
//! - Retrieval Augmented Generation (RAG) from golden_qa and game_data collections
//! - Custom tools for direct rAthena database queries (mobs, items)
//! - Confidence scoring and source citation in responses
//! - LLM routing via Heimdall Gateway
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
//!                              LLM Generation (via Heimdall Gateway)
//!                                      ↓
//!                         Response + Confidence + Citations
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

use crate::models::persona::Persona;
use crate::services::qdrant::QdrantService;

// ─── Constants ─────────────────────────────────────────────────────────────

/// Default Heimdall model
const DEFAULT_HEIMDALL_MODEL: &str = "mlx-community/Qwen3.5-35B-A3B-4bit";

/// Default timeout for completion requests (45 seconds for RAG operations)
const DEFAULT_TIMEOUT_SECS: u64 = 45;

// ─── RAG Configuration ───────────────────────────────────────────────────────

/// Per-tenant RAG configuration — resolved from `tenant_configs.search_settings` JSON.
///
/// Controls retrieval and generation parameters for the Oracle RAG Agent.
/// When `None` is passed, sensible defaults are used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Number of chunks to retrieve per Qdrant collection (default: 3)
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// LLM sampling temperature (default: 0.7)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum response tokens (default: 4096)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u16,
    /// Minimum similarity score — results below this threshold are discarded
    #[serde(default)]
    pub similarity_threshold: Option<f32>,
}

fn default_top_k() -> usize { 3 }
fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> u16 { 4096 }

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            similarity_threshold: None,
        }
    }
}

// ─── LLM Provider ───────────────────────────────────────────────────────────

/// Supported LLM providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Heimdall,
}

impl Default for LlmProvider {
    fn default() -> Self {
        LlmProvider::Heimdall
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Heimdall => write!(f, "heimdall"),
        }
    }
}

impl std::str::FromStr for LlmProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "heimdall" | "local" | "ollama" | "gemini" | "google" => Ok(LlmProvider::Heimdall),
            _ => Err(format!("Unknown provider (using Heimdall routing): {}", s)),
        }
    }
}

/// Qdrant collection names
pub const COLLECTION_WIKI_QA: &str = "golden_qa";
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

#[async_trait::async_trait]
pub trait DynamicContextPlugin: Send + Sync {
    async fn get_context<'a>(
        &'a self,
        message: &'a str,
        tools_used: &'a mut Vec<String>,
    ) -> Result<String>;
}

// ─── RAG Retriever ─────────────────────────────────────────────────────────

/// RAG retriever for Qdrant vector search
pub struct RagRetriever {
    qdrant: QdrantService,
    endpoint_url: String,
    embed_model: String,
}

/// Expose the Heimdall-backed embedding path to `services::resolve` without
/// making `get_embedding` part of the public API. The entity-resolution chain
/// depends only on this narrow trait, so it stays unit-testable with a stub.
#[async_trait::async_trait]
impl crate::services::resolve::Embedder for RagRetriever {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.get_embedding(text).await
    }

    fn model_id(&self) -> &str {
        &self.embed_model
    }

    fn dim(&self) -> usize {
        // Best-effort: the configured default model is BGE-M3 (1024-dim). Callers
        // that store embeddings should prefer the actual returned vector length;
        // this is advisory for drift-stamping when no vector is at hand.
        if self.embed_model.contains("bge-m3") {
            1024
        } else {
            0
        }
    }
}

impl RagRetriever {
    pub fn new(qdrant: QdrantService) -> Self {
        // Use Heimdall embedding endpoint
        let endpoint_url = env::var("HEIMDALL_API_URL")
            .unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
        let embed_model = env::var("EMBED_MODEL").unwrap_or_else(|_| "BAAI/bge-m3".to_string());

        Self {
            qdrant,
            endpoint_url,
            embed_model,
        }
    }

    /// Get embedding for text — uses Heimdall API (OpenAI compatible)
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();

        let url = format!("{}/embeddings", self.endpoint_url.trim_end_matches('/'));
        let api_key = env::var("HEIMDALL_API_KEY").unwrap_or_default();

        let body = serde_json::json!({
            "model": self.embed_model,
            "input": [text]
        });
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let err = resp.text().await?;
            return Err(anyhow::anyhow!("Embedding error: {}", err));
        }
        let json: serde_json::Value = resp.json().await?;
        let embedding = json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No embedding in response"))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        Ok(embedding)
    }

    pub async fn search_wiki(
        &self,
        query: &str,
        limit: usize,
        tenant_id: &str,
    ) -> Result<Vec<SourceCitation>> {
        let vector = self.get_embedding(query).await?;

        let results = self
            .qdrant
            .search(COLLECTION_WIKI_QA, vector, limit, tenant_id, false)
            .await?;

        let citations = Self::parse_search_results(results, "wiki")?;
        Ok(citations)
    }

    pub async fn search_game_data(
        &self,
        query: &str,
        limit: usize,
        tenant_id: &str,
    ) -> Result<Vec<SourceCitation>> {
        let vector = self.get_embedding(query).await?;

        let results = self
            .qdrant
            .search(COLLECTION_GAME_DATA, vector, limit, tenant_id, false)
            .await?;

        let citations = Self::parse_search_results(results, "game_data")?;
        Ok(citations)
    }

    pub async fn search_all(
        &self,
        query: &str,
        limit_per_collection: usize,
        tenant_id: &str,
    ) -> Result<Vec<SourceCitation>> {
        let mut all_citations = Vec::new();

        // Search wiki_qa
        match self
            .search_wiki(query, limit_per_collection, tenant_id)
            .await
        {
            Ok(citations) => all_citations.extend(citations),
            Err(e) => tracing::warn!("Wiki search failed: {}", e),
        }

        // Search game_data
        match self
            .search_game_data(query, limit_per_collection, tenant_id)
            .await
        {
            Ok(citations) => all_citations.extend(citations),
            Err(e) => tracing::warn!("Game data search failed: {}", e),
        }

        // Sort by relevance
        all_citations.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(all_citations)
    }

    /// Parse Qdrant search results into citations
    fn parse_search_results(
        results: serde_json::Value,
        source_type: &str,
    ) -> Result<Vec<SourceCitation>> {
        let mut citations = Vec::new();

        if let Some(results_array) = results.get("result").and_then(|r| r.as_array()) {
            for result in results_array {
                let score = result.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;

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
    /// Heimdall uses reqwest directly (OpenAI-compatible HTTP API)
    Heimdall {
        client: reqwest::Client,
        endpoint: String,
        model: String,
        api_key: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: u16,
    },
}

impl AgentBackend {
    /// Send a prompt to the underlying LLM
    pub async fn prompt(&self, message: &str) -> Result<String> {
        match self {
            AgentBackend::Heimdall {
                client,
                endpoint,
                model,
                api_key,
                system_prompt,
                temperature,
                max_tokens,
            } => {
                let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": message }
                    ],
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                    "stream": false
                });
                let resp = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .header("ngrok-skip-browser-warning", "true")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("Heimdall request failed: {}", e))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let err = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Heimdall error {}: {}", status, err));
                }

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| anyhow::anyhow!("Heimdall parse error: {}", e))?;

                json.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Heimdall: no content in response"))
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
    pub tenant_id: String,
    pub rag_config: RagConfig,
    agent: AgentBackend,
    retriever: RagRetriever,
    plugins: Vec<Box<dyn DynamicContextPlugin>>,
}

impl OracleRagAgent {
    pub fn new(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
        tenant_id: String,
    ) -> Self {
        Self::with_provider(
            persona,
            qdrant,
            plugins,
            LlmProvider::Heimdall,
            None,
            None,
            tenant_id,
            None,
        )
    }

    /// Create an OracleRagAgent with custom options (legacy, uses Ollama)
    pub fn with_options(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
        model: Option<&str>,
        timeout: Option<Duration>,
        tenant_id: String,
    ) -> Self {
        Self::with_provider(
            persona,
            qdrant,
            plugins,
            LlmProvider::Heimdall,
            model,
            timeout,
            tenant_id,
            None,
        )
    }

    /// Create an OracleRagAgent with a specific provider
    ///
    /// `rag_config`: Optional per-tenant RAG parameters (top_k, temperature, etc.).
    /// Pass `None` to use defaults. Typically resolved from `TenantConfig.search_settings`.
    pub fn with_provider(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
        provider: LlmProvider,
        model: Option<&str>,
        timeout: Option<Duration>,
        tenant_id: String,
        rag_config: Option<RagConfig>,
    ) -> Self {
        let timeout = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        let rag_config = rag_config.unwrap_or_default();

        // Build enhanced system prompt with RAG context instructions
        let enhanced_prompt = Self::build_enhanced_prompt(&persona);

        // Create agent based on provider (now all point to Heimdall)
        let (agent, model_name) = match provider {
            _ => { // Always use Heimdall backend implementation logic
                let api_key = env::var("HEIMDALL_API_KEY").unwrap_or_default();
                let endpoint = env::var("HEIMDALL_API_URL")
                    .unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
                let model_name = model.unwrap_or(DEFAULT_HEIMDALL_MODEL).to_string();
                let backend = AgentBackend::Heimdall {
                    client: reqwest::Client::new(),
                    endpoint,
                    model: model_name.clone(),
                    api_key,
                    system_prompt: enhanced_prompt.clone(),
                    temperature: rag_config.temperature,
                    max_tokens: rag_config.max_tokens,
                };
                (backend, model_name)
            }
        };

        let retriever = RagRetriever::new(qdrant);

        Self {
            persona,
            provider,
            model_name,
            timeout,
            tenant_id,
            rag_config,
            agent,
            retriever,
            plugins,
        }
    }

    /// Build enhanced system prompt with RAG instructions
    fn build_enhanced_prompt(persona: &Persona) -> String {
        format!(
            r#"{}

## RAG Capabilities
You have access to a knowledge document base containing domain-specific intelligence.

## Response Guidelines
1. When answering questions, use the provided context from the knowledge base
2. If you're uncertain, acknowledge the limitation
3. Cite your sources when providing specific data
4. Keep responses informative but concise
5. Always reply in the same language as the user's input

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

        // Step 1: Retrieve relevant context from RAG (top_k from RagConfig)
        let rag_sources = self
            .retriever
            .search_all(message, self.rag_config.top_k, &self.tenant_id)
            .await?;
        // Apply similarity_threshold filter if configured
        let rag_sources = if let Some(threshold) = self.rag_config.similarity_threshold {
            rag_sources.into_iter().filter(|s| s.relevance >= threshold).collect()
        } else {
            rag_sources
        };
        all_sources.extend(rag_sources);

        // Step 2: Check if we need to query databases directly
        let db_context = self.query_databases(message, &mut tools_used).await?;

        // Step 3: Build context-enhanced prompt
        let context = self.build_context(&all_sources, &db_context);
        let enhanced_message = format!(
            "Context from knowledge base:\n{}\n\nUser question: {}\n\nIMPORTANT: You must reply in the EXACT SAME LANGUAGE as the user question above.",
            context, message
        );

        // Step 4: Generate response using the agent backend
        let response =
            tokio::time::timeout(self.timeout, self.agent.prompt(enhanced_message.as_str()))
                .await
                .map_err(|_| {
                    anyhow::anyhow!("Request timeout after {}s", self.timeout.as_secs())
                })??;

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

    /// Query databases based on message content using loaded plugins
    async fn query_databases(&self, message: &str, tools_used: &mut Vec<String>) -> Result<String> {
        let mut context = String::new();

        for plugin in &self.plugins {
            if let Ok(plugin_context) = plugin.get_context(message, tools_used).await {
                if !plugin_context.is_empty() {
                    context.push_str(&plugin_context);
                    context.push_str("\n\n");
                }
            }
        }

        Ok(context)
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
            let avg_relevance: f32 =
                sources.iter().map(|s| s.relevance).sum::<f32>() / sources.len() as f32;
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
