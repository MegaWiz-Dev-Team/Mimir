//! Evaluation module — Unified interface for evaluating agents with different models
//!
//! Provides a common `evaluate_agent()` function that takes an agent name + model_id
//! and runs a single Q/A evaluation, returning the answer and latency.

use anyhow::{Result, bail};
use rig::providers::{ollama, gemini};
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::env;
use tracing::{info, warn};

use mimir_core_ai::models::persona::Persona;
use mimir_core_ai::services::qdrant::QdrantService;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::rag_engine::{OracleRagAgent, LlmProvider, DynamicContextPlugin};
use ro_ai_domain_game::tools::rag_tools::{QueryMobDbTool, QueryItemDbTool};

/// Result of a single evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub answer: String,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// LLM-as-Judge scores for a single Q/A
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeScores {
    pub accuracy: i8,
    pub completeness: i8,
    pub relevance: i8,
    pub reasoning: String,
}

/// Determine the provider from a model_id string
pub fn provider_from_model_id(model_id: &str) -> LlmProvider {
    if model_id.starts_with("gemini") {
        LlmProvider::Gemini
    } else {
        LlmProvider::Ollama
    }
}

/// Unified evaluation function: runs a question through an agent with a specific model
///
/// Returns `EvalResult` with the answer text and latency.
/// For `simple_npc` — only Ollama models are supported.
/// For `oracle_rag` — both Ollama and Gemini models are supported.
pub async fn evaluate_agent(
    agent_name: &str,
    model_id: &str,
    question: &str,
    db: Option<&DbPool>,
    qdrant: Option<&QdrantService>,
) -> Result<EvalResult> {
    let provider = provider_from_model_id(model_id);
    let start = Instant::now();

    let answer = match agent_name {
        "simple_npc" => {
            // simple_npc only supports Ollama
            if provider != LlmProvider::Ollama {
                bail!("simple_npc only supports Ollama models, got: {}", model_id);
            }
            
            let persona = create_eval_persona("simple_npc", 1);
            let client = ollama::Client::new();
            let agent = client.agent(model_id)
                .preamble(&persona.system_prompt)
                .build();
            
            tokio::time::timeout(
                Duration::from_secs(120),
                agent.prompt(question)
            )
            .await
            .map_err(|_| anyhow::anyhow!("Timeout after 120s"))?
            .map_err(|e| anyhow::anyhow!("Prompt failed: {}", e))?
        }

        "oracle_rag" => {
            let persona = create_eval_persona("oracle_rag", 2);
            let qdrant_svc = qdrant
                .cloned()
                .unwrap_or_else(|| QdrantService::new());
            
            let mut plugins: Vec<Box<dyn DynamicContextPlugin>> = vec![];
            if let Some(pool) = db {
                plugins.push(Box::new(QueryMobDbTool::new(pool.clone())));
                plugins.push(Box::new(QueryItemDbTool::new(pool.clone())));
            }

            let oracle = OracleRagAgent::with_provider(
                persona,
                qdrant_svc,
                plugins,
                provider,
                Some(model_id),
                Some(Duration::from_secs(120)),
                "default_tenant".to_string(),
            );

            let response = oracle.chat(question).await?;
            response.content
        }

        _ => bail!("Unknown agent: {}", agent_name),
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    Ok(EvalResult {
        answer,
        latency_ms,
        error: None,
    })
}

/// Use Gemini as LLM-as-Judge to score a response
pub async fn judge_response(
    question: &str,
    expected_answer: &str,
    actual_answer: &str,
    judge_model: &str,
) -> Result<JudgeScores> {
    let api_key = env::var("GEMINI_API_KEY")
        .or_else(|_| env::var("GOOGLE_API_KEY"))
        .map_err(|_| anyhow::anyhow!("GEMINI_API_KEY or GOOGLE_API_KEY must be set for judge"))?;
    
    let client = gemini::Client::new(&api_key);
    let agent = client.agent(judge_model)
        .preamble("You are an expert evaluator for AI agent responses. You score responses on a 1-5 scale. Always respond in valid JSON only, no markdown.")
        .build();

    let prompt = format!(
        r#"Evaluate the following AI response against the expected answer.

**Question:** {question}

**Expected Answer:** {expected_answer}

**Actual Answer:** {actual_answer}

Score each dimension from 1 (worst) to 5 (best):
- **accuracy**: How factually correct is the actual answer compared to the expected answer?
- **completeness**: Does the actual answer cover all key points from the expected answer?
- **relevance**: Does the actual answer stay on topic without hallucination or irrelevant info?

Respond with ONLY this JSON (no markdown, no code fences):
{{"accuracy": <1-5>, "completeness": <1-5>, "relevance": <1-5>, "reasoning": "<brief explanation>"}}"#
    );

    let response = tokio::time::timeout(
        Duration::from_secs(60),
        agent.prompt(prompt.as_str())
    )
    .await
    .map_err(|_| anyhow::anyhow!("Judge timeout after 60s"))?
    .map_err(|e| anyhow::anyhow!("Judge prompt failed: {}", e))?;

    // Parse JSON from response (handle potential markdown wrapping)
    let json_str = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let scores: JudgeScores = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse judge response: {} | Raw: {}", e, json_str))?;

    // Validate score range
    if scores.accuracy < 1 || scores.accuracy > 5
        || scores.completeness < 1 || scores.completeness > 5
        || scores.relevance < 1 || scores.relevance > 5
    {
        warn!("Judge returned out-of-range scores, clamping");
    }

    Ok(scores)
}

/// Create a minimal persona for evaluation purposes
fn create_eval_persona(agent_name: &str, tier: i8) -> Persona {
    // Try loading from config first, fallback to a minimal persona
    match Persona::load_by_name(agent_name) {
        Ok(p) => p,
        Err(_) => {
            info!("No persona config found for '{}', using default eval persona", agent_name);
            Persona {
                name: agent_name.to_string(),
                display_name: format!("Eval {}", agent_name),
                tier,
                model_id: None,
                system_prompt: match agent_name {
                    "simple_npc" => "You are a helpful NPC in Ragnarok Online. Answer questions about the game accurately and concisely.".to_string(),
                    "oracle_rag" => "You are an oracle NPC with deep knowledge of Ragnarok Online. Use provided context to answer questions accurately. Cite sources when possible.".to_string(),
                    _ => "You are a helpful assistant.".to_string(),
                },
                greeting: None,
                allowed_actions: vec![],
                personality_traits: vec!["helpful".to_string(), "knowledgeable".to_string()],
            }
        }
    }
}

/// List of agents available for evaluation
pub fn available_agents() -> Vec<&'static str> {
    vec!["simple_npc", "oracle_rag"]
}

/// Check if a given (agent, model) combination is compatible
pub fn is_compatible(agent_name: &str, model_id: &str) -> bool {
    let provider = provider_from_model_id(model_id);
    match agent_name {
        "simple_npc" => provider == LlmProvider::Ollama,
        "oracle_rag" => true, // Supports both
        _ => false,
    }
}
