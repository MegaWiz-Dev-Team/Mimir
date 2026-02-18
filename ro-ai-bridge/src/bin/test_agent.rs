use rig::providers::openai;
use rig::completion::Prompt;
// Note: In 0.10.0, rig-core might not have 'ollama' module directly or API might be different.
// We'll try to use a generic OpenAI compatible client if Ollama specific one is missing, 
// or check docs/source if this fails.
// For now, let's try to stick to the pattern but be prepared for failures.
// Actually, earlier versions used `rig::providers::ollama`. Let's assume it exists.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialize tracing
    tracing_subscriber::fmt::init();
    
    // In older versions, client creation might differ. 
    // If 0.10.0 doesn't support Ollama directly, we might need to use OpenAI client pointed to Ollama.
    // Let's assume OpenAI compatible client for now as a fallback if Ollama module is missing.
    
    let client = openai::Client::from_url("ollama", "http://localhost:11434/v1");
    
    // Create agent with a single context prompt using "gemma:2b"
    let agent = client
        .agent("gemma:2b")
        .preamble("You are a helpful assistant from Ragnarok Online world.")
        .build();

    // Prompt the agent and print the response
    println!("🤖 Sending 'Hello world' to Agent (gemma:2b)...");
    
    let response = agent.prompt("Hello world! Who are you?").await?;
    
    println!("✅ Agent Response: {}", response);
    
    Ok(())
}
