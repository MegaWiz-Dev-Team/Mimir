use anyhow::Result;
use rig::completion::Prompt;
use rig::providers::openai;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing Rig with Heimdall Gateway...");
    let api_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_else(|_| "gateway_key_here".to_string());
    let endpoint = std::env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
    let client = openai::Client::from_url(&api_key, &endpoint);
    let agent = client.agent("mlx-community/Qwen3.5-35B-A3B-4bit").build();

    match agent.prompt("hi").await {
        Ok(resp) => println!("Success: {}", resp),
        Err(e) => println!("Error: {:?}", e),
    }

    Ok(())
}
