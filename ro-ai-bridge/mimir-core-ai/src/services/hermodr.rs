//! Hermodr MCP sidecar client.
//!
//! Hermodr is the Universal MCP Sidecar (separate Rust binary, repo `Hermodr/`)
//! that wraps stateless / external-API tools as MCP tools — keeping Mimir
//! focused on stateful in-process tools (Qdrant, Neo4j, MariaDB).
//!
//! Architectural rationale: see Asgard `MultiAgent_Architecture_Plan.md`
//! → "Hybrid Tool Placement". Hermodr instances are deployed one-per-upstream
//! (pubmed, ct.gov, fda, ...) so each gets its own rate-limit budget and can
//! scale independently.
//!
//! ## Configuration
//!
//! Each Hermodr instance is registered via env var. Mimir's MCP server appends
//! tools from every reachable Hermodr to its own `tools/list` response, so
//! agents see local + remote tools as one flat catalog.
//!
//! | Env var                    | Hermodr deployment           |
//! |----------------------------|------------------------------|
//! | `HERMODR_EIR_MEDICAL_URL`  | Bundled medical tools (dev)  |
//! | `HERMODR_PUBMED_URL`       | NCBI E-utilities             |
//! | `HERMODR_TRIALS_URL`       | ClinicalTrials.gov v2        |
//! | `HERMODR_FDA_URL`          | openFDA                      |
//! | `HERMODR_RXNAV_URL`        | RxNav drug interactions      |
//! | `HERMODR_WEBFETCH_URL`     | Generic web fetch            |
//! | `HERMODR_MEDCALC_URL`      | Pure-compute calculators     |
//!
//! Each value is the Hermodr `/rpc` endpoint, e.g.
//! `http://hermodr-pubmed.asgard.svc:8090/rpc`.
//!
//! Sprint 42 (B-58) will wire `discover_tools` + `call_tool` into
//! `mcp_server::list_tools` / `dispatch_tool_call`. Until then this module
//! provides only the config + transport scaffolding.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use tracing::{debug, warn};

/// One configured Hermodr endpoint.
#[derive(Debug, Clone)]
pub struct HermodrEndpoint {
    /// Logical name (used for log lines + tool-name namespacing if needed).
    pub name: String,
    /// JSON-RPC endpoint, e.g. `http://hermodr-pubmed.asgard.svc:8090/rpc`.
    pub rpc_url: String,
}

/// Read all configured Hermodr endpoints from env. Returns empty Vec if none set.
pub fn endpoints_from_env() -> Vec<HermodrEndpoint> {
    const VARS: &[(&str, &str)] = &[
        ("HERMODR_EIR_MEDICAL_URL", "eir_medical"),
        ("HERMODR_PUBMED_URL", "pubmed"),
        ("HERMODR_TRIALS_URL", "trials"),
        ("HERMODR_FDA_URL", "fda"),
        ("HERMODR_RXNAV_URL", "rxnav"),
        ("HERMODR_WEBFETCH_URL", "webfetch"),
        ("HERMODR_MEDCALC_URL", "medcalc"),
    ];
    VARS.iter()
        .filter_map(|(env_key, name)| {
            std::env::var(env_key)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(|rpc_url| HermodrEndpoint {
                    name: (*name).into(),
                    rpc_url,
                })
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
}

/// Hermodr exposes tools via JSON-RPC `tools/list` per MCP spec.
/// Returns the raw tool definitions (caller can convert to McpToolDefinition).
pub async fn discover_tools(ep: &HermodrEndpoint) -> Result<Vec<Value>, String> {
    let req = JsonRpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/list",
        params: json!({}),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let res = client
        .post(&ep.rpc_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("hermodr {} unreachable: {e}", ep.name))?;
    let body: JsonRpcResponse = res.json().await.map_err(|e| format!("decode: {e}"))?;
    if let Some(err) = body.error {
        return Err(format!("hermodr {} error: {err}", ep.name));
    }
    let tools = body
        .result
        .as_ref()
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    debug!(target: "hermodr", "{} → {} tools", ep.name, tools.len());
    Ok(tools)
}

/// Forward a `tools/call` request to a Hermodr endpoint and return the result.
pub async fn call_tool(
    ep: &HermodrEndpoint,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    let req = JsonRpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: json!({ "name": tool_name, "arguments": arguments }),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let res = client
        .post(&ep.rpc_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("hermodr {} unreachable: {e}", ep.name))?;
    let body: JsonRpcResponse = res.json().await.map_err(|e| format!("decode: {e}"))?;
    if let Some(err) = body.error {
        warn!(target: "hermodr", "{} {} → {err}", ep.name, tool_name);
        return Err(format!("hermodr error: {err}"));
    }
    Ok(body.result.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_empty_when_no_env() {
        // Cleanup any test pollution.
        for (k, _) in [
            ("HERMODR_EIR_MEDICAL_URL", ""),
            ("HERMODR_PUBMED_URL", ""),
            ("HERMODR_TRIALS_URL", ""),
            ("HERMODR_FDA_URL", ""),
            ("HERMODR_RXNAV_URL", ""),
            ("HERMODR_WEBFETCH_URL", ""),
            ("HERMODR_MEDCALC_URL", ""),
        ] {
            // SAFETY: tests in this crate run single-threaded by default for env mutation.
            unsafe { std::env::remove_var(k); }
        }
        assert!(endpoints_from_env().is_empty());
    }

    #[test]
    fn endpoint_picked_up_from_env() {
        unsafe { std::env::set_var("HERMODR_EIR_MEDICAL_URL", "http://example/rpc"); }
        let eps = endpoints_from_env();
        assert!(eps.iter().any(|e| e.name == "eir_medical" && e.rpc_url == "http://example/rpc"));
        unsafe { std::env::remove_var("HERMODR_EIR_MEDICAL_URL"); }
    }
}
