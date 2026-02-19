use ro_ai_bridge::agents::simple_npc::SimpleNpcAgent;
use ro_ai_bridge::models::persona::Persona;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    println!("🧪 Testing SimpleNpcAgent...");

    let persona = Persona {
        name: "test".to_string(),
        display_name: "Test NPC".to_string(),
        tier: 1,
        system_prompt: "You are a test NPC.".to_string(),
        greeting: Some("Hello!".to_string()),
        allowed_actions: vec![],
        personality_traits: vec!["friendly".to_string()],
    };

    let agent = SimpleNpcAgent::new(persona);
    println!("🤖 Agent initialized with model: {}", agent.model_name);

    println!("💬 Sending message: 'hello'");
    let response = agent.chat("hello").await?;
    println!("📩 Response received: {}", response);

    Ok(())
}
