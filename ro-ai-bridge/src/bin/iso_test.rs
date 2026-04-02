use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    tenant_id: String,
    role: String,
    exp: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret = "dev_secret_key";

    let expiration = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as usize + 3600;

    let claims = Claims {
        iss: "MimirIAM".to_string(),
        sub: "admin_user_id".to_string(),
        tenant_id: "default_tenant".to_string(),
        role: "admin".to_string(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    println!("Generated Admin JWT: {}", token);

    let client = Client::new();
    let base_url = "http://localhost:8080/api";

    println!("\n=== TC_SP2_01: Tenant Settings Page Loading ===");
    let res = client
        .get(format!("{}/iam/tenants", base_url))
        .bearer_auth(&token)
        .send()
        .await?;
    println!("GET /iam/tenants status: {}", res.status());
    let get_text = res.text().await?;
    println!("GET /iam/tenants body length: {} bytes", get_text.len());

    println!("\n=== TC_SP2_02: Update Tenant Name ===");
    let res = client
        .patch(format!("{}/iam/tenants/default_tenant", base_url))
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        .body(r#"{"name": "Mimir Default Tenant (Updated)"}"#)
        .send()
        .await?;
    println!("PATCH /iam/tenants/default_tenant status: {}", res.status());
    println!(
        "PATCH /iam/tenants/default_tenant body: {}",
        res.text().await?
    );

    println!("\n=== TC_SP2_03: Data Isolation - API Filtering ===");
    let res = client
        .post(format!("{}/vector/search", base_url))
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        .body(r#"{"query": "game mechanics", "limit": 2}"#)
        .send()
        .await?;
    println!("POST /vector/search status: {}", res.status());
    let vector_text = res.text().await?;
    println!(
        "POST /vector/search body length: {} bytes",
        vector_text.len()
    );

    println!("\n=== TC_SP2_04: Vector Management UI Updates (Delete API) ===");
    let res = client
        .delete(format!("{}/vector/999999", base_url))
        .bearer_auth(&token)
        .send()
        .await?;
    println!("DELETE /vector/999999 status: {}", res.status());

    Ok(())
}
