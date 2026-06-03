//! Agent-config save-path validation.
//!
//! Agent Studio historically stored ANY model/tool/MCP string with zero checks,
//! which is the root cause of config-vs-runtime drift (agents pointing at models
//! Heimdall doesn't serve, tool names the runtime ignores, decorative MCP URLs).
//! This guards the create/update path: hard-reject unservable models, soft-warn
//! on tool/MCP strings that won't take effect.
//!
//! Sources of truth (the live ones are unreliable — Heimdall `/v1/models` returns
//! empty and the `ai_models` table is polluted with image tags), so:
//!   * models → a curated allowlist (env `AGENT_MODEL_ALLOWLIST`, default
//!     `gemma-4-26b`) plus the `gemini/` provider prefix.
//!   * tools  → the runtime-recognized set (mirrors Bifrost `skills.rs`
//!     `kb_tool_label` + the RAG/memory tool names).

/// Tool names the Bifrost runtime actually recognizes. Mirrors
/// `Bifrost/src/swarm_engine/skills.rs::kb_tool_label` (KB tools, incl. legacy
/// aliases) plus the RAG/memory tools governed by the use_* booleans. Keep in
/// sync with that function.
pub const RECOGNIZED_TOOLS: &[&str] = &[
    // RAG / memory (governed by use_rag / use_knowledge_graph / use_pageindex)
    "vector_search",
    "graph_search",
    "tree_search",
    "memvid_agent_memory_search",
    // KB tools recognized by kb_tool_label (canonical + legacy aliases)
    "search_primekg",
    "primekg_search",
    "primekg_disease_relations",
    "search_clinical_kb",
    "clinical_kb_search",
    "snomed_search",
    "resolve_snomed",
    "search_snomed",
    "pubmed_search",
    "search_pubmed",
];

/// Resolve the model allowlist: env `AGENT_MODEL_ALLOWLIST` (comma-separated)
/// overrides; default is the one model Heimdall MLX actually serves.
pub fn model_allowlist() -> Vec<String> {
    std::env::var("AGENT_MODEL_ALLOWLIST")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec!["gemma-4-26b".to_string()])
}

#[derive(Debug, Default, PartialEq)]
pub struct AgentValidation {
    /// Hard failures — the save must be rejected (HTTP 422).
    pub errors: Vec<String>,
    /// Soft issues — the save proceeds but the client is told what won't work.
    pub warnings: Vec<String>,
}

impl AgentValidation {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate the effective config that is about to be written.
///
/// `model_id` is the effective (post-merge) model. `tools` / `mcp_servers` should
/// be the values the caller is setting (skip when the update doesn't touch them,
/// to avoid warning on untouched pre-existing values).
pub fn validate_agent(
    model_id: &str,
    tools: Option<&[String]>,
    mcp_servers: Option<&[String]>,
    allowlist: &[String],
) -> AgentValidation {
    let mut v = AgentValidation::default();
    let m = model_id.trim();

    // ── model ─────────────────────────────────────────────────────────────
    if m.is_empty() {
        v.errors.push("model_id must not be empty".to_string());
    } else if m.starts_with("gemini/") {
        // Gemini via Heimdall's gemini/ provider routing — served.
    } else if m == "gemini" || m.starts_with("gemini-") {
        // Bare gemini name routes to the local MLX backend and hangs.
        v.errors.push(format!(
            "model '{m}' will be routed to the local MLX backend and hang — Gemini models require the 'gemini/' prefix (e.g. 'gemini/{m}')."
        ));
    } else if m.starts_with("claude") {
        v.warnings.push(format!(
            "model '{m}' uses Claude routing via Heimdall, which is currently failing (timeouts/502). Verify the gateway's Anthropic upstream before relying on it."
        ));
    } else if !allowlist.iter().any(|a| a == m) {
        v.errors.push(format!(
            "model '{m}' is not served by Heimdall. Served local model(s): [{}]. For Gemini use the 'gemini/' prefix (e.g. gemini/gemini-3.1-flash-lite). Override the allowlist with AGENT_MODEL_ALLOWLIST.",
            allowlist.join(", ")
        ));
    }

    // ── tools (soft) ──────────────────────────────────────────────────────
    if let Some(ts) = tools {
        for t in ts {
            let t = t.trim();
            if t.is_empty() {
                continue;
            }
            if !RECOGNIZED_TOOLS.contains(&t) {
                v.warnings.push(format!(
                    "tool '{t}' is not a runtime-recognized tool name and will be ignored. Recognized: [{}].",
                    RECOGNIZED_TOOLS.join(", ")
                ));
            }
        }
    }

    // ── mcp_servers (soft) ──────────────────────────────────────────────────
    if let Some(ms) = mcp_servers {
        if ms.iter().any(|s| !s.trim().is_empty()) {
            v.warnings.push(
                "mcp_servers are stored but NOT consumed by the execution engine yet (decorative); per-agent MCP servers will not take effect until that wiring lands."
                    .to_string(),
            );
        }
    }

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow() -> Vec<String> {
        vec!["gemma-4-26b".to_string()]
    }

    #[test]
    fn served_local_model_ok() {
        let v = validate_agent("gemma-4-26b", None, None, &allow());
        assert!(v.is_ok(), "errors: {:?}", v.errors);
        assert!(v.warnings.is_empty());
    }

    #[test]
    fn gemini_prefixed_ok() {
        let v = validate_agent("gemini/gemini-3.1-flash-lite", None, None, &allow());
        assert!(v.is_ok(), "errors: {:?}", v.errors);
    }

    #[test]
    fn bare_gemini_rejected_with_prefix_hint() {
        let v = validate_agent("gemini-3.1-flash-lite", None, None, &allow());
        assert!(!v.is_ok());
        assert!(v.errors[0].contains("gemini/"));
    }

    #[test]
    fn unservable_model_rejected() {
        let v = validate_agent("mlx-community/Qwen3.5-35B-A3B-4bit", None, None, &allow());
        assert!(!v.is_ok());
        assert!(v.errors[0].contains("not served"));
    }

    #[test]
    fn claude_warns_not_rejects() {
        let v = validate_agent("claude-sonnet-4-6", None, None, &allow());
        assert!(v.is_ok(), "claude should warn, not reject");
        assert_eq!(v.warnings.len(), 1);
        assert!(v.warnings[0].contains("Claude routing"));
    }

    #[test]
    fn empty_model_rejected() {
        let v = validate_agent("  ", None, None, &allow());
        assert!(!v.is_ok());
    }

    #[test]
    fn known_tools_no_warning() {
        let tools = vec![
            "vector_search".to_string(),
            "search_primekg".to_string(),
            "clinical_kb_search".to_string(),
        ];
        let v = validate_agent("gemma-4-26b", Some(&tools), None, &allow());
        assert!(v.is_ok());
        assert!(v.warnings.is_empty(), "warnings: {:?}", v.warnings);
    }

    #[test]
    fn unknown_tool_warns() {
        let tools = vec!["primekg_search".to_string(), "bogus_tool".to_string()];
        let v = validate_agent("gemma-4-26b", Some(&tools), None, &allow());
        assert!(v.is_ok(), "unknown tool is a warning, not error");
        assert_eq!(v.warnings.len(), 1);
        assert!(v.warnings[0].contains("bogus_tool"));
    }

    #[test]
    fn mcp_servers_warn() {
        let mcp = vec!["http://some-mcp:9000".to_string()];
        let v = validate_agent("gemma-4-26b", None, Some(&mcp), &allow());
        assert!(v.is_ok());
        assert_eq!(v.warnings.len(), 1);
        assert!(v.warnings[0].contains("mcp_servers"));
    }

    #[test]
    fn allowlist_env_override_respected() {
        let custom = vec!["my-custom-model".to_string()];
        let v = validate_agent("my-custom-model", None, None, &custom);
        assert!(v.is_ok());
    }
}
