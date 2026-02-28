//! Request ID middleware — generates UUID per request, logs request/response,
//! and returns `X-Request-Id` header.

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
    http::{HeaderValue, header},
};
use std::time::Instant;
use tracing::{info, Span, info_span};
use uuid::Uuid;

/// Header name for request correlation ID.
pub const X_REQUEST_ID: &str = "x-request-id";

/// Axum middleware that:
/// 1. Generates a UUID `request_id` for every incoming request
/// 2. Creates a tracing span with request metadata
/// 3. Logs request entry and response exit with latency
/// 4. Sets `X-Request-Id` response header
pub async fn request_id_middleware(
    req: Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = req.method().clone();
    let uri = req.uri().path().to_string();

    // Extract tenant_id if available from extensions (set by tenant_auth_middleware)
    // We can't access it here before it's set, so we log it as "anonymous"
    let tenant_id = req.extensions()
        .get::<crate::middleware::tenant::TenantContext>()
        .map(|ctx| ctx.tenant_id.clone())
        .unwrap_or_else(|| "-".to_string());

    let span = info_span!(
        "request",
        request_id = %request_id,
        method = %method,
        path = %uri,
        tenant_id = %tenant_id,
    );

    let start = Instant::now();

    info!(
        parent: &span,
        "→ request started"
    );

    let mut response = {
        let _guard = span.enter();
        next.run(req).await
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    info!(
        parent: &span,
        status_code = status,
        latency_ms = latency_ms,
        "← request completed"
    );

    // Set X-Request-Id response header
    if let Ok(val) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(X_REQUEST_ID, val);
    }

    response
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get, body::Body, middleware};
    use http::Request as HttpRequest;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "ok"
    }

    fn build_test_app() -> Router {
        Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(request_id_middleware))
    }

    // ========================================
    // UT-014aa: request_id_middleware — generates and propagates request_id
    // ========================================
    #[tokio::test]
    async fn test_request_id_generated() {
        let app = build_test_app();

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        // Verify X-Request-Id header exists and is a valid UUID
        let request_id = response.headers().get(X_REQUEST_ID);
        assert!(request_id.is_some(), "X-Request-Id header should be present");
        let id_str = request_id.unwrap().to_str().unwrap();
        assert!(
            Uuid::parse_str(id_str).is_ok(),
            "X-Request-Id should be a valid UUID, got: {}",
            id_str
        );
    }

    // ========================================
    // UT-014ac: response_header — returns X-Request-Id in response
    // ========================================
    #[tokio::test]
    async fn test_response_header_x_request_id() {
        let app = build_test_app();

        let resp1 = app.clone()
            .oneshot(HttpRequest::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let resp2 = app
            .oneshot(HttpRequest::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let id1 = resp1.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap().to_string();
        let id2 = resp2.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap().to_string();

        // Each request gets a unique ID
        assert_ne!(id1, id2, "Each request should get a unique request_id");
    }
}
