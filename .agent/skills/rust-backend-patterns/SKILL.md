---
name: rust-backend-patterns
description: Standard Rust/Axum backend patterns for Project Mimir — error handling with anyhow, SQLx compile-time queries, tenant_auth_middleware security, Vault secret injection, Direct HTTP Dispatch for LLM providers, and inline TDD test modules. Triggers when writing Rust code, creating API routes, handling database queries, or implementing backend features.
---

# Rust Backend Patterns Skill

Project Mimir's backend (`ro-ai-bridge/`) is built with **Rust + Axum + SQLx + tokio**. This skill defines the standard patterns for all backend development.

## Project Structure

```
ro-ai-bridge/
├── src/
│   ├── main.rs              # Axum server setup, router
│   ├── config.rs             # Configuration & environment
│   ├── lib.rs                # Library exports
│   ├── routes/               # API route handlers (one file per domain)
│   │   ├── knowledge.rs
│   │   ├── iam.rs
│   │   ├── agents.rs
│   │   └── ...
│   ├── middleware/            # Axum middleware (auth, logging)
│   ├── agents/               # AI agent logic
│   ├── bin/                   # Binary utilities
│   └── utils/                # Shared utilities
├── mimir-core-ai/            # Core AI crate (RAG, QA, LLM)
│   ├── src/
│   │   ├── rag_engine.rs
│   │   ├── llm_provider.rs
│   │   ├── qa_qc.rs
│   │   └── ...
│   └── migrations/           # SQLx migrations
└── Cargo.toml
```

## 1. Error Handling

### Service Layer
Use `anyhow::Result` for internal service functions:
```rust
use anyhow::Result;

async fn process_knowledge(pool: &PgPool, id: i64) -> Result<Knowledge> {
    let item = sqlx::query_as!(Knowledge, "SELECT * FROM knowledge WHERE id = $1", id)
        .fetch_one(pool)
        .await
        .context("Failed to fetch knowledge item")?;
    Ok(item)
}
```

### HTTP Handlers
Map internal errors to appropriate `StatusCode`. **NEVER** leak database or internal error details:
```rust
pub async fn get_knowledge(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<Json<Knowledge>, (StatusCode, Json<ErrorResponse>)> {
    let item = sqlx::query_as!(Knowledge, "SELECT * FROM knowledge WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            tracing::error!("DB error fetching knowledge {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(ErrorResponse { error: "Internal server error".into() }))
        })?;
    Ok(Json(item))
}
```

## 2. Security — Tenant Authentication

ALL protected routes **MUST** use `tenant_auth_middleware`:
```rust
// In main.rs router setup
let protected = Router::new()
    .route("/knowledge", get(list_knowledge).post(create_knowledge))
    .route("/knowledge/:id", get(get_knowledge).delete(delete_knowledge))
    .layer(middleware::from_fn_with_state(pool.clone(), tenant_auth_middleware));
```

### Extracting Tenant ID
Always extract `X-Tenant-Id` from the request in route handlers:
```rust
pub async fn list_knowledge(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<Knowledge>>, StatusCode> {
    let tenant_id = headers.get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Use tenant_id in ALL queries to prevent data leaks
    let items = sqlx::query_as!(Knowledge,
        "SELECT * FROM knowledge WHERE tenant_id = $1", tenant_id)
        .fetch_all(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(items))
}
```

> ⚠️ **CRITICAL**: Every database query in protected routes MUST filter by `tenant_id`. Omitting this creates a multi-tenant data leak vulnerability.

## 3. Database — SQLx Patterns

### Compile-Time Verified Queries (Preferred)
```rust
let item = sqlx::query_as!(Knowledge,
    "SELECT id, title, content, tenant_id FROM knowledge WHERE id = $1 AND tenant_id = $2",
    id, tenant_id
).fetch_one(&pool).await?;
```

### Dynamic Queries (When Needed)
Use `QueryBuilder` for dynamic conditions — **never** string interpolation:
```rust
let mut qb = sqlx::QueryBuilder::new("SELECT * FROM knowledge WHERE tenant_id = ");
qb.push_bind(tenant_id);

if let Some(search) = search_term {
    qb.push(" AND title ILIKE ");
    qb.push_bind(format!("%{}%", search));
}

let items = qb.build_query_as::<Knowledge>().fetch_all(&pool).await?;
```

### Migrations
Place in `mimir-core-ai/migrations/`:
```
migrations/
├── 20260305000000_custom_roles.sql         # up migration
└── down/
    └── 20260305000000_custom_roles.down.sql # down migration
```

## 4. Vault Secret Injection

Secrets are stored in HashiCorp Vault and injected at startup via `inject_vault_secrets`:
```rust
// In config.rs — secrets loaded from Vault, not .env
let config = Config::from_env_and_vault().await?;

// Access secrets through config
let api_key = config.heimdall_api_key.as_deref()
    .ok_or_else(|| anyhow!("Heimdall API key not configured"))?;
```

**Rules**:
- NEVER hardcode API keys or secrets in source code
- NEVER log secret values
- Use Vault paths like `secret/data/mimir/{key}`

## 5. LLM Provider Integration — Direct HTTP Dispatch

For LLM API calls, use the Direct HTTP Dispatch pattern with `reqwest`:
```rust
let client = reqwest::Client::new();
let response = client
    .post(&format!("{}/v1/chat/completions", provider_url))
    .header("Authorization", format!("Bearer {}", api_key))
    .header("Content-Type", "application/json")
    .json(&request_body)
    .send()
    .await?;
```

This pattern gives maximum flexibility across different providers (Heimdall, Gemini, vLLM, MLX).

## 6. Inline TDD Tests

Every Rust file with logic MUST have a `#[cfg(test)]` module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feature_happy_path() {
        // Arrange → Act → Assert
    }

    #[tokio::test]
    async fn test_feature_error_case() {
        // Test error handling
    }
}
```

See the `tdd` skill for the complete Red-Green-Refactor workflow.
