use sqlx::MySqlPool;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct OllamaTagResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct HeimdallModelResponse {
    data: Vec<HeimdallModel>,
}

#[derive(Deserialize)]
struct HeimdallModel {
    id: String,
    owned_by: Option<String>,
}

#[derive(Serialize)]
pub struct ModelSyncResult {
    pub synced_models: usize,
    pub deactivated_models: usize,
}

/// Synchronizes models from Ollama and Heimdall into the DB
pub async fn sync_models(pool: &MySqlPool) -> Result<ModelSyncResult, String> {
    info!("Starting AI models sync process...");
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let mut active_models = std::collections::HashSet::new();

    // 1. Sync Ollama Models
    // In OrbStack K3s, 'host.docker.internal' safely routes to Mac localhost
    // Allow override via SYNC_OLLAMA_URL for client deployments
    let ollama_url = std::env::var("SYNC_OLLAMA_URL")
        .unwrap_or_else(|_| "http://host.docker.internal:11434".to_string());
    
    match client.get(&format!("{}/api/tags", ollama_url)).send().await {
        Ok(res) if res.status().is_success() => {
            if let Ok(json) = res.json::<OllamaTagResponse>().await {
                for m in json.models {
                    let model_id = m.name;
                    active_models.insert(("ollama".to_string(), model_id.clone()));
                    upsert_model(pool, &model_id, "ollama", "{\"reasoning\":false,\"tools\":true,\"vision\":false}").await;
                }
            } else {
                warn!("Failed to parse Ollama tags response");
            }
        }
        _ => warn!("Failed to reach Ollama at {}", ollama_url),
    }

    // 2. Sync Heimdall Models (Includes Google, OpenAI, MLX, etc.)
    // In OrbStack K3s, 'host.docker.internal' safely routes to Mac localhost
    // Allow override via SYNC_HEIMDALL_URL for client deployments
    let heimdall_url = std::env::var("SYNC_HEIMDALL_URL")
        .unwrap_or_else(|_| "http://host.docker.internal:8080".to_string());
    let heimdall_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_default();

    let mut heimdall_req = client.get(&format!("{}/v1/models", heimdall_url));
    if !heimdall_key.is_empty() {
        heimdall_req = heimdall_req.header("Authorization", format!("Bearer {}", heimdall_key));
    }

    match heimdall_req.send().await {
        Ok(res) if res.status().is_success() => {
            match res.json::<HeimdallModelResponse>().await {
                Ok(json) => {
                    for m in json.data {
                        let provider = if m.owned_by.as_deref().unwrap_or("unknown") == "unknown" || m.owned_by.as_deref().unwrap_or("").is_empty() {
                            "heimdall".to_string()
                        } else {
                            m.owned_by.unwrap()
                        };
                        
                        active_models.insert((provider.clone(), m.id.clone()));
                        upsert_model(pool, &m.id, &provider, "{\"reasoning\":true,\"tools\":true,\"vision\":false}").await;
                    }
                }
                Err(e) => {
                    warn!("Failed to parse Heimdall response from {}: {}. Seeding default models.", heimdall_url, e);
                    seed_default_heimdall_models(pool, &mut active_models).await;
                }
            }
        }
        _ => {
            warn!("Failed to reach Heimdall at {}/v1/models. Seeding default models.", heimdall_url);
            seed_default_heimdall_models(pool, &mut active_models).await;
        }
    }

    // 3. Mark missing models as inactive
    let deactivated = deactivate_missing_models(pool, &active_models).await;
    
    info!("Model sync complete. Synced: {}, Deactivated: {}", active_models.len(), deactivated);
    
    Ok(ModelSyncResult {
        synced_models: active_models.len(),
        deactivated_models: deactivated,
    })
}

async fn upsert_model(pool: &MySqlPool, model_id: &str, provider: &str, capabilities: &str) {
    let query = "
        INSERT INTO ai_models (model_id, provider, model_type, is_active, capabilities)
        VALUES (?, ?, 'llm', true, ?)
        ON DUPLICATE KEY UPDATE is_active = true, capabilities = VALUES(capabilities)
    ";

    if let Err(e) = sqlx::query(query)
        .bind(model_id)
        .bind(provider)
        .bind(capabilities)
        .execute(pool)
        .await
    {
        error!("Failed to upsert model {} ({}): {:?}", model_id, provider, e);
    }
}

async fn deactivate_missing_models(pool: &MySqlPool, active_models: &std::collections::HashSet<(String, String)>) -> usize {
    let query = "SELECT provider, model_id FROM ai_models WHERE is_active = true AND provider IN ('ollama', 'heimdall', 'google', 'openai', 'anthropic')";
    let mut deactivated = 0;

    if let Ok(rows) = sqlx::query_as::<_, (String, String)>(query).fetch_all(pool).await {
        for (provider, model_id) in rows {
            if !active_models.contains(&(provider.clone(), model_id.clone())) {
                warn!("Model {} ({}) is no longer available on gateway. Marking inactive.", model_id, provider);
                let update_q = "UPDATE ai_models SET is_active = false WHERE provider = ? AND model_id = ?";
                if let Err(e) = sqlx::query(update_q)
                    .bind(&provider)
                    .bind(&model_id)
                    .execute(pool)
                    .await
                {
                    error!("Failed to deactivate model {}: {:?}", model_id, e);
                } else {
                    deactivated += 1;
                }
            }
        }
    }
    
    deactivated
}

async fn seed_default_heimdall_models(pool: &MySqlPool, active_models: &mut std::collections::HashSet<(String, String)>) {
    let default_models = vec![
        ("heimdall", "mlx-community/Qwen3.5-35B-A3B-4bit", "{\"reasoning\":true,\"tools\":true,\"vision\":false}"),
        ("heimdall", "mlx-community/Qwen3.5-27B-4bit", "{\"reasoning\":true,\"tools\":true,\"vision\":false}"),
        ("heimdall", "mlx-community/Qwen3.5-9B-MLX-4bit", "{\"reasoning\":false,\"tools\":true,\"vision\":false}"),
        ("heimdall", "lmstudio-community/medgemma-4b-it-MLX-4bit", "{\"reasoning\":false,\"tools\":false,\"vision\":false}"),
        ("heimdall", "qwen2.5", "{\"reasoning\":false,\"tools\":true,\"vision\":false}"),
        ("heimdall", "llama3.2", "{\"reasoning\":false,\"tools\":true,\"vision\":false}"),
        ("google", "gemini-2.0-flash", "{\"reasoning\":false,\"tools\":true,\"vision\":true}"),
        ("google", "gemini-2.5-flash", "{\"reasoning\":false,\"tools\":true,\"vision\":true}"),
        ("google", "gemini-2.5-pro", "{\"reasoning\":true,\"tools\":true,\"vision\":true}"),
        ("sakura", "sakura/Qwen3.5-110B-Chat", "{\"reasoning\":true,\"tools\":true,\"vision\":false}"),
    ];
    for (provider, model, caps) in default_models {
        active_models.insert((provider.to_string(), model.to_string()));
        upsert_model(pool, model, provider, caps).await;
    }
}
