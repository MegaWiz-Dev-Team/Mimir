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

#[async_trait::async_trait]
pub trait DynamicContextPlugin: Send + Sync {
    async fn get_context<'a>(&'a self, message: &'a str, tools_used: &'a mut Vec<String>) -> Result<String>;
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
    plugins: Vec<Box<dyn DynamicContextPlugin>>,
}

impl OracleRagAgent {
    /// Create a new OracleRagAgent with RAG capabilities (default: Ollama)
    pub fn new(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
    ) -> Self {
        Self::with_provider(persona, qdrant, plugins, LlmProvider::Ollama, None, None)
    }

    /// Create an OracleRagAgent with custom options (legacy, uses Ollama)
    pub fn with_options(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
        model: Option<&str>,
        timeout: Option<Duration>,
    ) -> Self {
        Self::with_provider(persona, qdrant, plugins, LlmProvider::Ollama, model, timeout)
    }

    /// Create an OracleRagAgent with a specific provider
    pub fn with_provider(
        persona: Persona,
        qdrant: QdrantService,
        plugins: Vec<Box<dyn DynamicContextPlugin>>,
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
        
        Self {
            persona,
            provider,
            model_name,
            timeout,
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

