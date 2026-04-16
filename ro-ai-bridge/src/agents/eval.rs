//! Evaluation module — Unified interface for evaluating agents with different models
//!
//! Provides a common `evaluate_agent()` function that takes an agent name + model_id
//! and runs a single Q/A evaluation, returning the answer and latency.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use mimir_core_ai::models::persona::Persona;
use mimir_core_ai::rag_engine::{DynamicContextPlugin, LlmProvider, OracleRagAgent};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::qdrant::QdrantService;
use ro_ai_domain_game::tools::rag_tools::{QueryItemDbTool, QueryMobDbTool};

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
pub fn provider_from_model_id(_model_id: &str) -> LlmProvider {
    LlmProvider::Heimdall
}

/// Unified evaluation function: runs a question through an agent with a specific model
///
/// Returns `EvalResult` with the answer text and latency.
/// All agents route through Heimdall Gateway.
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
            let persona = create_eval_persona("simple_npc", 1);
            let agent = ro_ai_domain_game::simple_npc::SimpleNpcAgent::with_model(persona, model_id);

            agent.chat(question).await?
        }

        "oracle_rag" => {
            let persona = create_eval_persona("oracle_rag", 2);
            let qdrant_svc = qdrant.cloned().unwrap_or_else(|| QdrantService::new());

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
                None, // Use default RagConfig for evaluation
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

/// Use LLM-as-Judge (via Heimdall) to score a response
pub async fn judge_response(
    question: &str,
    expected_answer: &str,
    actual_answer: &str,
    judge_model: &str,
) -> Result<JudgeScores> {
    let api_key = env::var("HEIMDALL_API_KEY").unwrap_or_default();
    let endpoint = env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
    
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

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

    let payload = serde_json::json!({
        "model": judge_model,
        "messages": [
            { "role": "system", "content": "You are an expert evaluator for AI agent responses. You score responses on a 1-5 scale. Always respond in valid JSON only, no markdown." },
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.0,
        "max_tokens": 1024
    });

    let resp_result = tokio::time::timeout(Duration::from_secs(60), client.post(&url).header("Authorization", format!("Bearer {}", api_key)).json(&payload).send()).await;
    let resp = resp_result
        .map_err(|_| anyhow::anyhow!("Judge timeout after 60s"))?
        .map_err(|e| anyhow::anyhow!("Judge request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Judge request error: {} - {}", resp.status(), resp.text().await.unwrap_or_default()));
    }

    let json: serde_json::Value = resp.json().await?;
    let response = json.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("No content in judge response"))?;

    // Parse JSON from response (handle potential markdown wrapping)
    let json_str = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let scores: JudgeScores = serde_json::from_str(json_str).map_err(|e| {
        anyhow::anyhow!("Failed to parse judge response: {} | Raw: {}", e, json_str)
    })?;

    // Validate score range
    if scores.accuracy < 1
        || scores.accuracy > 5
        || scores.completeness < 1
        || scores.completeness > 5
        || scores.relevance < 1
        || scores.relevance > 5
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
            info!(
                "No persona config found for '{}', using default eval persona",
                agent_name
            );
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
                avatar_url: None,
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
pub fn is_compatible(_agent_name: &str, _model_id: &str) -> bool {
    // All agents support Heimdall now
    true
}
