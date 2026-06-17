//! `lit_search` — local-only research RAG (ADR-024 P5). A POST shim that proxies to
//! mimir-api's `GET /api/v1/knowledge/search` (unified semantic search over the
//! tenant's shared KBs). POST (not GET) so it dispatches through Hermodr's analytics
//! sidecar like the other analyst tools — analyst-research stays single-sidecar, and
//! we avoid Hermodr's GET-args-in-body limitation. Offline: searches only the local
//! Mimir KBs (external arXiv/Semantic Scholar = a network-gated future opt-in).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn def_limit() -> u32 {
    10
}

#[derive(Deserialize)]
pub struct LitReq {
    pub tenant_id: String,
    pub q: String,
    #[serde(default = "def_limit")]
    pub limit: u32,
}

pub struct LitErr(String);
impl IntoResponse for LitErr {
    fn into_response(self) -> Response {
        (StatusCode::BAD_GATEWAY, Json(json!({ "error": self.0 }))).into_response()
    }
}

/// Query the local Mimir RAG and return its results verbatim (kb hits with codes /
/// labels) for the research agent to cite.
pub async fn lit_search(Json(r): Json<LitReq>) -> Result<Json<Value>, LitErr> {
    let base = std::env::var("MIMIR_API_URL").unwrap_or_else(|_| "http://mimir-api.asgard.svc:8080".into());
    let lim = r.limit.to_string();
    let resp = reqwest::Client::new()
        .get(format!("{base}/api/v1/knowledge/search"))
        .query(&[("q", r.q.as_str()), ("tenant_id", r.tenant_id.as_str()), ("limit", lim.as_str())])
        .send()
        .await
        .map_err(|e| LitErr(format!("mimir-api knowledge/search unreachable: {e}")))?;
    let status = resp.status();
    let v: Value = resp
        .json()
        .await
        .map_err(|e| LitErr(format!("non-JSON from mimir-api knowledge/search: {e}")))?;
    if !status.is_success() {
        return Err(LitErr(format!("mimir-api knowledge/search {status}: {v}")));
    }
    Ok(Json(v))
}
