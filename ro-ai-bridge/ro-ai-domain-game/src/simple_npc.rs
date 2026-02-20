use rig::providers::ollama;
use rig::completion::Prompt;
use mimir_core_ai::models::persona::Persona;
use std::time::Duration;

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
    pub const LLAMA3_2_3B: &str = "llama3.2:3b";
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
pub struct SimpleNpcAgent {
    pub persona: Persona,
    pub model_name: String,
    pub timeout: Duration,
    // We store the underlying rig agent
    agent: rig::agent::Agent<ollama::CompletionModel>,
}

impl SimpleNpcAgent {
    /// Create a new SimpleNpcAgent with default model and timeout
    pub fn new(persona: Persona) -> Self {
        Self::with_options(persona, None, None)
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
    pub fn with_options(persona: Persona, model: Option<&str>, timeout: Option<Duration>) -> Self {
        // Initialize Ollama client
        // rig::providers::ollama::Client::new() will use OLLAMA_BASE_URL env var if set
        let client = ollama::Client::new();
        
        let model_name = model.unwrap_or(DEFAULT_MODEL).to_string();
        let timeout = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        
        // Build the agent with the persona's system prompt
        let agent = client.agent(&model_name)
            .preamble(&persona.system_prompt)
            .build();
            
        Self { persona, model_name, timeout, agent }
    }

    /// Create a SimpleNpcAgent with a specific model
    pub fn with_model(persona: Persona, model: &str) -> Self {
        Self::with_options(persona, Some(model), None)
    }

    /// Create a SimpleNpcAgent with a specific timeout
    pub fn with_timeout(persona: Persona, timeout: Duration) -> Self {
        Self::with_options(persona, None, Some(timeout))
    }

    /// Simple chat function that takes a message and returns the completion
    /// 
    /// # Errors
    /// Returns an error if:
    /// - The Ollama server is unreachable
    /// - The request times out
    /// - The model fails to generate a response
    pub async fn chat(&self, message: &str) -> anyhow::Result<String> {
        let response = tokio::time::timeout(
            self.timeout,
            self.agent.prompt(message)
        )
        .await
        .map_err(|_| anyhow::anyhow!("Request timeout after {}s", self.timeout.as_secs()))?
        .map_err(|e| anyhow::anyhow!("Agent prompt failed: {}", e))?;
            
        Ok(response)
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
