---
description: standard practices for the Rust/Axum backend
---
# Rust Backend Guidelines

1. **Error Handling**: Use `anyhow::Result` for service layers. When returning HTTP responses in Axum handlers, map internal errors to appropriate `StatusCode`s and DO NOT leak sensitive database or internal error details to the client.
2. **Security Focus (IAM)**: All protected routes MUST be secured by the `tenant_auth_middleware`. Ensure valid JWT token decoding. Legacy bypasses are strictly forbidden.
3. **Database (SQLx)**: Always use `sqlx::query!` macros for compile-time SQL verification where possible. If dynamic queries are required, use `sqlx::QueryBuilder`. Prevent SQL injection at all costs.
