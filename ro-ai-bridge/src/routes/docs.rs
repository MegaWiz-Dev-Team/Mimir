//! API Documentation Routes (Issue #162)
//!
//! GET /api/docs — Swagger UI
//! GET /api/docs/openapi.yaml — OpenAPI spec file

use axum::{
    routing::get,
    Router,
    response::Html,
    response::Response,
    http::{header, StatusCode},
};
use sqlx::MySqlPool;

pub fn docs_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/", get(swagger_ui))
        .route("/openapi.yaml", get(openapi_spec))
}

/// GET /api/docs — Swagger UI (loaded from CDN)
async fn swagger_ui() -> Html<String> {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Project Mimir — API Documentation</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        body { margin: 0; background: #1a1a2e; }
        .swagger-ui .topbar { display: none; }
        .swagger-ui { max-width: 1200px; margin: 0 auto; padding: 20px; }
        .swagger-ui .info .title { color: #e94560; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
        SwaggerUIBundle({
            url: '/api/docs/openapi.yaml',
            dom_id: '#swagger-ui',
            deepLinking: true,
            presets: [SwaggerUIBundle.presets.apis],
            layout: 'BaseLayout',
        });
    </script>
</body>
</html>"#.to_string())
}

/// GET /api/docs/openapi.yaml — serve the spec file
async fn openapi_spec() -> Response {
    let spec = include_str!("../../../docs/api/openapi.yaml");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/yaml; charset=utf-8")
        .body(spec.into())
        .unwrap()
}
