use axum::http::HeaderMap;

/// Extract tenant_id from the X-Tenant-Id request header.
/// Falls back to "default_tenant" if the header is missing or invalid UTF-8.
pub fn extract_tenant_id<'a>(headers: &'a HeaderMap) -> &'a str {
    headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or("default_tenant")
}
