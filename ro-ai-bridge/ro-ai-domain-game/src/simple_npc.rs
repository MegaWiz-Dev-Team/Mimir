use rig::providers::{ollama, gemini};
use rig::completion::Prompt;
use mimir_core_ai::models::persona::Persona;
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::Value;
use crate::tools::actions::{HealTool, BuffTool};

/// Default provider to use if not specified
const DEFAULT_PROVIDER: &str = "ollama";

/// Default model to use if not specified
/// Optimized for speed: llama3.2 is a fast, efficient model suitable for <2s latency
const DEFAULT_MODEL: &str = "llama3.2";

/// Default timeout for completion requests (30 seconds)
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Recommended models for fast (< 2s) latency
/// These are smaller, quantized models that respond quickly
pub mod fast_models {
    /// Llama 3.2 1B - Very fast, good for simple NPCs
    pub const LLAMA3_2_1B: &str = "llama3.2:1b";
    /// Llama 3.2 3B - Balanced speed/quality
    pub const LLAMA3_2_3B: &str = "llama3.2";
    /// Mistral 7B - Good quality, moderate speed
    pub const MISTRAL: &str = "mistral";
    /// Phi 3 Mini - Very fast, Microsoft model
    pub const PHI3_MINI: &str = "phi3:3.8b";
}

/// SimpleNpcAgent - Tier 1 NPC Chat Agent optimized for < 2s latency
/// 
/// This agent is designed for fast, simple NPC conversations without RAG.
/// It uses a completion-based approach with the persona's system prompt as preamble.
/// 
/// ## Latency Optimization
/// - No RAG/vector search calls
/// - Cached persona loading
/// - Fast default model (llama3.2)
/// - Configurable timeout to prevent hanging
/// 
/// ## Usage
/// ```ignore
/// // Basic usage
/// let agent = SimpleNpcAgent::new(persona);
/// let response = agent.chat("Hello!").await?;
/// 
/// // With custom model for speed
/// let agent = SimpleNpcAgent::with_model(persona, "llama3.2:1b"); // Smaller, faster model
/// ```
enum AgentImplementation {
    Ollama(rig::agent::Agent<ollama::CompletionModel>),
    Gemini(rig::agent::Agent<gemini::completion::CompletionModel>),
}

pub struct SimpleNpcAgent {
    pub persona: Persona,
    pub provider_name: String,
    pub model_name: String,
    pub timeout: Duration,
    pub action_capture: Arc<Mutex<Option<Value>>>,
    agent_impl: AgentImplementation,
}

impl SimpleNpcAgent {
    /// Create a new SimpleNpcAgent with default model and timeout
    pub fn new(persona: Persona) -> Self {
        Self::with_options(persona, None, None, None)
    }

    /// Create a SimpleNpcAgent with custom model and/or timeout
    /// 
    /// # Arguments
    /// * `persona` - The persona configuration
    /// * `model` - Optional model name (e.g., "llama3.2", "mistral"). Defaults to DEFAULT_MODEL
    /// * `timeout` - Optional timeout duration. Defaults to 30 seconds
    /// 
    /// # Example
    /// ```ignore
    /// let agent = SimpleNpcAgent::with_options(
    ///     persona,
    ///     Some("mistral"),
    ///     Some(Duration::from_secs(60))
    /// );
    /// ```
    pub fn with_options(persona: Persona, provider: Option<&str>, model: Option<&str>, timeout: Option<Duration>) -> Self {
        let model_name = model.unwrap_or(DEFAULT_MODEL).to_string();
        
        let provider_name = provider.map(|p| p.to_string()).unwrap_or_else(|| {
            if model_name.starts_with("gemini") || model_name.starts_with("google") {
                "google".to_string()
            } else {
                DEFAULT_PROVIDER.to_string()
            }
        });

        let timeout = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        
        let action_capture = Arc::new(Mutex::new(None));
        
        // Enforce the language constraint and ReAct tool fallback instructions
        let preamble = format!(
            "{}\n\
            Always reply in the same language as the user's input.\n\
            If you need to heal the player, you MUST output this exact block in your response:\n\
            [ACTION: {{\"command\": \"heal\", \"params\": {{\"amount\": 100}}}}]", 
            persona.system_prompt
        );

        let agent_impl = match provider_name.as_str() {
            "google" => {
                let client = gemini::Client::from_env();
                AgentImplementation::Gemini(client.agent(&model_name)
                    .preamble(&preamble)
                    .tool(HealTool::new(action_capture.clone()))
                    .tool(BuffTool::new(action_capture.clone()))
                    .build())
            },
            _ => {
                // Default to Ollama
                let client = ollama::Client::new();
                AgentImplementation::Ollama(client.agent(&model_name)
                    .preamble(&preamble)
                    .tool(HealTool::new(action_capture.clone()))
                    .tool(BuffTool::new(action_capture.clone()))
                    .build())
            }
        };
            
        Self { persona, provider_name, model_name, timeout, action_capture, agent_impl }
    }

    /// Create a SimpleNpcAgent with a specific model (provider defaults to ollama)
    pub fn with_model(persona: Persona, model: &str) -> Self {
        Self::with_options(persona, None, Some(model), None)
    }

    /// Create a SimpleNpcAgent with a specific timeout
    pub fn with_timeout(persona: Persona, timeout: Duration) -> Self {
        Self::with_options(persona, None, None, Some(timeout))
    }

    /// Simple chat function that takes a message and returns the completion
    /// 
    /// # Errors
    /// Returns an error if:
    /// - The provider server is unreachable
    /// - The request times out
    /// - The model fails to generate a response
    pub async fn chat(&self, message: &str) -> anyhow::Result<String> {
        let enhanced_message = format!(
            "{}\n\nIMPORTANT: You must reply in the EXACT SAME LANGUAGE as the user message above.",
            message
        );
        
        let prompt_future = async {
            match &self.agent_impl {
                AgentImplementation::Ollama(agent) => agent.prompt(enhanced_message.as_str()).await,
                AgentImplementation::Gemini(agent) => agent.prompt(enhanced_message.as_str()).await,
            }
        };

        let response = tokio::time::timeout(self.timeout, prompt_future)
        .await
        .map_err(|_| anyhow::anyhow!("Request timeout after {}s", self.timeout.as_secs()))?
        .map_err(|e| anyhow::anyhow!("Agent prompt failed: {}", e))?;
            
        let mut final_response = response.clone();
        
        // Fallback: Check for ReAct-style action output
        if let Some(start_idx) = response.find("[ACTION:") {
            if let Some(end_idx) = response[start_idx..].find(']') {
                let action_str = response[start_idx + 8..start_idx + end_idx].trim();
                if let Ok(action_json) = serde_json::from_str::<serde_json::Value>(action_str) {
                    let mut capture = self.action_capture.lock().await;
                    *capture = Some(action_json);
                } else {
                    tracing::error!("Failed to parse ReAct JSON: {}", action_str);
                }
                // Strip the action block from the response sent to the user
                final_response = response[..start_idx].to_string() + &response[start_idx + end_idx + 1..];
            }
        }
        
        Ok(final_response.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_persona() -> Persona {
        Persona {
            name: "test".to_string(),
            display_name: "Test NPC".to_string(),
            tier: 1,
            system_prompt: "You are a test NPC.".to_string(),
            greeting: Some("Hello!".to_string()),
            allowed_actions: vec![],
            personality_traits: vec!["friendly".to_string()],
            model_id: None,
        }
    }

    #[test]
    fn test_agent_uses_default_model() {
        let persona = create_test_persona();
        let agent = SimpleNpcAgent::new(persona);
        assert_eq!(agent.model_name, DEFAULT_MODEL);
    }

    #[test]
    fn test_agent_uses_custom_model() {
        let persona = create_test_persona();
        let agent = SimpleNpcAgent::with_model(persona, "mistral");
        assert_eq!(agent.model_name, "mistral");
    }

    #[test]
    fn test_agent_uses_default_timeout() {
        let persona = create_test_persona();
        let agent = SimpleNpcAgent::new(persona);
        assert_eq!(agent.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn test_agent_uses_custom_timeout() {
        let persona = create_test_persona();
        let agent = SimpleNpcAgent::with_timeout(persona, Duration::from_secs(60));
        assert_eq!(agent.timeout, Duration::from_secs(60));
    }
}
