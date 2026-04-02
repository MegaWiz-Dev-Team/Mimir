use dotenvy::dotenv;
use mimir_core_ai::models::persona::Persona;
use ro_ai_domain_game::simple_npc::SimpleNpcAgent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    println!("🧪 Testing SimpleNpcAgent...");

    let persona = Persona {
        name: "test".to_string(),
        avatar_url: None,
        display_name: "Test NPC".to_string(),
        tier: 1,
        model_id: None,
        system_prompt: "You are a test NPC. If someone says please heal me, use your heal tool."
            .to_string(),
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
