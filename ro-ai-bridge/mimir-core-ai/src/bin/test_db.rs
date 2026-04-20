use mimir_core_ai::services::db::init_db;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::from_filename("../.env").ok();
    dotenvy::dotenv().ok();
    
    let pool = init_db().await?;

    let rows = sqlx::query("SELECT id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, use_rag, use_knowledge_graph, tools, is_published, tier FROM agent_configs WHERE display_name LIKE '%CPAP%'")
        .fetch_all(&pool)
        .await?;

    println!("Found {} agents", rows.len());

    for row in rows {
        let id: i64 = row.get("id");
        let name: String = row.get("name");
        let display_name: Option<String> = row.get("display_name");
        let description: Option<String> = row.get("description");
        let system_prompt: String = row.get("system_prompt");
        let model_id: String = row.get("model_id");
        let provider: String = row.get("provider");
        let temperature: Option<f64> = row.get("temperature");
        let max_tokens: Option<i32> = row.get("max_tokens");
        let use_rag: Option<bool> = row.get("use_rag");
        let use_knowledge_graph: Option<bool> = row.get("use_knowledge_graph");
        let tools: Option<sqlx::types::JsonValue> = row.get("tools");
        let is_published: Option<bool> = row.get("is_published");
        let tier: Option<i32> = row.get("tier");

        println!("---");
        println!("ID: {}", id);
        println!("Name: {}", name);
        println!("Display Name: {:?}", display_name);
        println!("Description: {:?}", description);
        println!("System Prompt: {}", system_prompt);
        println!("Model ID: {}", model_id);
        println!("Provider: {}", provider);
        println!("Temperature: {:?}", temperature);
        println!("Max Tokens: {:?}", max_tokens);
        println!("Use RAG: {:?}", use_rag);
        println!("Use Knowledge Graph: {:?}", use_knowledge_graph);
        println!("Tools: {}", serde_json::to_string(&tools).unwrap_or("None".into()));
        println!("Is Published: {:?}", is_published);
        println!("Tier: {:?}", tier);
    }
    
    Ok(())
}
