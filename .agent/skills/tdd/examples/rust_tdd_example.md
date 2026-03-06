# Rust TDD Example: Adding a New API Endpoint

## Scenario
Adding a `GET /api/roles` endpoint to list all roles.

## Step 1: 🔴 Red — Write Failing Test

```rust
// src/routes/iam.rs

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_list_roles_returns_seeded_roles() {
        // Arrange
        let pool = setup_test_db().await;
        seed_default_roles(&pool).await;

        // Act
        let response = list_roles(State(pool)).await;
        let status = response.status();
        let body: Vec<Role> = parse_body(response).await;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert!(body.len() >= 3); // admin, editor, viewer
        assert!(body.iter().any(|r| r.name == "admin"));
    }

    #[tokio::test]
    async fn test_list_roles_empty_database() {
        let pool = setup_test_db().await;
        // No seeding

        let response = list_roles(State(pool)).await;
        let body: Vec<Role> = parse_body(response).await;

        assert!(body.is_empty());
    }
}
```

Run: `cargo test test_list_roles` → ❌ Compilation error (function doesn't exist yet)

## Step 2: 🟢 Green — Write Minimum Implementation

```rust
pub async fn list_roles(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Role>>, StatusCode> {
    let roles = sqlx::query_as!(Role, "SELECT id, name, permissions FROM roles")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(roles))
}
```

Run: `cargo test test_list_roles` → ✅ Both tests pass

## Step 3: 🔵 Refactor — Improve Error Handling

```rust
pub async fn list_roles(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Role>>, (StatusCode, Json<ErrorResponse>)> {
    let roles = sqlx::query_as!(Role, "SELECT id, name, permissions FROM roles ORDER BY id")
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch roles: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(ErrorResponse { error: "Failed to fetch roles".into() }))
        })?;

    Ok(Json(roles))
}
```

Run: `cargo test test_list_roles` → ✅ Tests still pass after refactor
