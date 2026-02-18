use rig::providers::ollama;
use rig::completion::Prompt;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing Rig with Native Ollama Client...");
    let client = ollama::Client::new();
    let agent = client.agent("gemma:2b").build();

    match agent.prompt("hi").await {
        Ok(resp) => println!("Success: {}", resp),
        Err(e) => println!("Error: {:?}", e),
    }

    Ok(())
}
