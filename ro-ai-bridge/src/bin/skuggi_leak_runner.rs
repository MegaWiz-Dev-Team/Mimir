//! 🌑 Skuggi leak-detection runner (Sprint 50b).
//!
//! End-to-end PII leak gate: fetches the `pii_test_corpus` for a tenant,
//! sends each prompt to a Mimir agent, collects responses, submits them
//! to the score-batch endpoint, and exits non-zero if any leak is detected.
//!
//! Runs anywhere that can reach Mimir's HTTP API — CI, OrbStack local
//! stack, or staging. No DB connection needed (Mimir is the data plane).
//!
//! ## Usage
//!
//! ```sh
//! cargo run --bin skuggi-leak-runner -- \
//!     --mimir-url https://mimir.asgard.internal \
//!     --tenant-id asgard_insurance \
//!     --agent-id 7 \
//!     --auth-token "$MIMIR_TOKEN" \
//!     --concurrency 4
//! ```
//!
//! Or via env: `MIMIR_URL`, `MIMIR_TENANT_ID`, `MIMIR_AGENT_ID`,
//! `MIMIR_AUTH_TOKEN`, `SKUGGI_CONCURRENCY` (cli flags take precedence).
//!
//! ## Exit codes
//!   - `0` — every corpus row scored clean
//!   - `1` — at least one row leaked PII (substring match OR PII regex hit)
//!   - `2` — infra error (network, auth, schema mismatch)
//!
//! ## Architecture
//!
//! The runner is a thin orchestrator. All the scoring logic lives in
//! `skuggi-core` (regex set) + Mimir's admin_skuggi (`/score-batch`
//! endpoint). The runner just glues corpus → agent → scorer over HTTP.

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::ExitCode;
use std::time::{Duration, Instant};

// ─── CLI args ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Args {
    mimir_url: String,
    tenant_id: String,
    agent_id: i64,
    auth_token: Option<String>,
    concurrency: usize,
    /// Optional filter on corpus.test_class (free_text / anchored / mixed
    /// / insurance / negative_clinical / negative_edge).
    test_class: Option<String>,
    /// Optional path to write a markdown report. Defaults to stdout only.
    output: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut mimir_url = std::env::var("MIMIR_URL").ok();
    let mut tenant_id = std::env::var("MIMIR_TENANT_ID").ok();
    let mut agent_id = std::env::var("MIMIR_AGENT_ID").ok().and_then(|s| s.parse().ok());
    let mut auth_token = std::env::var("MIMIR_AUTH_TOKEN").ok();
    let mut concurrency: usize = std::env::var("SKUGGI_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let mut test_class: Option<String> = None;
    let mut output: Option<String> = None;

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        let next = argv.get(i + 1).cloned();
        match arg.as_str() {
            "--mimir-url"   => { mimir_url = next; i += 2; }
            "--tenant-id"   => { tenant_id = next; i += 2; }
            "--agent-id"    => { agent_id = next.and_then(|s| s.parse().ok()); i += 2; }
            "--auth-token"  => { auth_token = next; i += 2; }
            "--concurrency" => { concurrency = next.and_then(|s| s.parse().ok()).unwrap_or(4); i += 2; }
            "--test-class"  => { test_class = next; i += 2; }
            "--output"      => { output = next; i += 2; }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => { i += 1; }
        }
    }

    let mimir_url = mimir_url.ok_or("missing --mimir-url or MIMIR_URL")?;
    let tenant_id = tenant_id.ok_or("missing --tenant-id or MIMIR_TENANT_ID")?;
    let agent_id = agent_id.ok_or("missing --agent-id or MIMIR_AGENT_ID")?;

    Ok(Args {
        mimir_url,
        tenant_id,
        agent_id,
        auth_token,
        concurrency: concurrency.max(1),
        test_class,
        output,
    })
}

fn print_help() {
    eprintln!("🌑 skuggi-leak-runner — end-to-end PII leak gate");
    eprintln!();
    eprintln!("USAGE: skuggi-leak-runner --mimir-url URL --tenant-id TENANT --agent-id N [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --mimir-url URL        Mimir API base, e.g. https://mimir.asgard.internal");
    eprintln!("  --tenant-id TENANT     Tenant to test (typically asgard_insurance)");
    eprintln!("  --agent-id N           Numeric agent_configs.id of the agent to exercise");
    eprintln!("  --auth-token TOKEN     Optional Bearer token (or MIMIR_AUTH_TOKEN env)");
    eprintln!("  --concurrency N        Parallel /agents/chat calls (default 4)");
    eprintln!("  --test-class CLASS     Filter corpus.test_class (default: all rows)");
    eprintln!("  --output PATH          Write markdown report to PATH (in addition to stdout)");
    eprintln!();
    eprintln!("EXIT CODES:");
    eprintln!("  0 = clean (no leaks)");
    eprintln!("  1 = LEAK detected — at least one row failed");
    eprintln!("  2 = infra error (network, auth, schema mismatch)");
}

// ─── HTTP shapes ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct CorpusItem {
    id: String,
    leak_marker: String,
    prompt: String,
    #[serde(default)]
    expected_categories: Vec<String>,
    is_negative: bool,
    test_class: String,
}

#[derive(Debug, Deserialize)]
struct CorpusResponse {
    #[serde(default)]
    items: Vec<CorpusItem>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    content: String,
    #[serde(default)]
    latency_ms: i64,
}

#[derive(Debug, Serialize)]
struct ScoreItem<'a> {
    corpus_id: &'a str,
    response: &'a str,
}

#[derive(Debug, Serialize)]
struct ScoreBatchRequest<'a> {
    items: Vec<ScoreItem<'a>>,
    tenant_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct ScoredRow {
    corpus_id: String,
    leak_marker: String,
    is_negative: bool,
    marker_echoed: bool,
    #[serde(default)]
    pii_matches_in_response: Vec<String>,
    leaked: bool,
}

#[derive(Debug, Deserialize)]
struct ScoreBatchSummary {
    total: usize,
    leaks: usize,
    clean: usize,
    negative_controls_total: usize,
    negative_controls_with_leak: usize,
}

#[derive(Debug, Deserialize)]
struct ScoreBatchResponse {
    summary: ScoreBatchSummary,
    items: Vec<ScoredRow>,
}

// ─── Per-row run record ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RowRun {
    corpus_id: String,
    test_class: String,
    is_negative: bool,
    leak_marker: String,
    response: String,
    chat_latency_ms: i64,
    chat_error: Option<String>,
}

// ─── HTTP helpers ────────────────────────────────────────────────────────

fn auth_header(args: &Args) -> Option<(String, String)> {
    args.auth_token.as_ref().map(|t| ("authorization".into(), format!("Bearer {}", t)))
}

async fn fetch_corpus(
    client: &reqwest::Client,
    args: &Args,
) -> Result<Vec<CorpusItem>, String> {
    let mut url = format!(
        "{}/api/v1/admin/skuggi/corpus?tenant_id={}",
        args.mimir_url.trim_end_matches('/'),
        urlencoding_minimal(&args.tenant_id),
    );
    if let Some(tc) = &args.test_class {
        url.push_str(&format!("&test_class={}", urlencoding_minimal(tc)));
    }

    let mut req = client.get(&url).header("x-tenant-id", &args.tenant_id);
    if let Some((k, v)) = auth_header(args) {
        req = req.header(k, v);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("corpus GET transport: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("corpus GET HTTP {}: {}", status, body));
    }
    let parsed: CorpusResponse = resp
        .json()
        .await
        .map_err(|e| format!("corpus GET decode: {}", e))?;
    Ok(parsed.items)
}

async fn call_agent(
    client: &reqwest::Client,
    args: &Args,
    prompt: &str,
) -> Result<ChatResponse, String> {
    let url = format!(
        "{}/api/v1/agents/{}/chat",
        args.mimir_url.trim_end_matches('/'),
        args.agent_id,
    );
    let mut req = client
        .post(&url)
        .header("x-tenant-id", &args.tenant_id)
        .json(&json!({ "message": prompt }));
    if let Some((k, v)) = auth_header(args) {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("agent chat transport: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("agent chat HTTP {}: {}", status, body));
    }
    resp.json::<ChatResponse>().await.map_err(|e| format!("agent chat decode: {}", e))
}

async fn score_batch(
    client: &reqwest::Client,
    args: &Args,
    runs: &[RowRun],
) -> Result<ScoreBatchResponse, String> {
    let url = format!(
        "{}/api/v1/admin/skuggi/score-batch",
        args.mimir_url.trim_end_matches('/'),
    );
    let items: Vec<ScoreItem> = runs
        .iter()
        .filter(|r| r.chat_error.is_none())
        .map(|r| ScoreItem { corpus_id: &r.corpus_id, response: &r.response })
        .collect();
    if items.is_empty() {
        return Err("no successful agent responses to score".into());
    }
    let body = ScoreBatchRequest { items, tenant_id: &args.tenant_id };
    let mut req = client
        .post(&url)
        .header("x-tenant-id", &args.tenant_id)
        .json(&body);
    if let Some((k, v)) = auth_header(args) {
        req = req.header(k, v);
    }
    let resp = req.send().await.map_err(|e| format!("score-batch transport: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("score-batch HTTP {}: {}", status, txt));
    }
    resp.json::<ScoreBatchResponse>().await.map_err(|e| format!("score-batch decode: {}", e))
}

/// Minimal URL-encoder for query-string values. Avoids pulling
/// `urlencoding` crate for a single use-site.
fn urlencoding_minimal(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

// ─── Markdown report ─────────────────────────────────────────────────────

fn render_report(
    args: &Args,
    runs: &[RowRun],
    score: &ScoreBatchResponse,
    started: Instant,
) -> String {
    let mut out = String::new();
    out.push_str("# 🌑 Skuggi Leak Check Report\n\n");
    out.push_str(&format!("**Tenant:** `{}`\n", args.tenant_id));
    out.push_str(&format!("**Agent ID:** `{}`\n", args.agent_id));
    out.push_str(&format!("**Mimir:** `{}`\n", args.mimir_url));
    if let Some(tc) = &args.test_class {
        out.push_str(&format!("**Filter:** `test_class={}`\n", tc));
    }
    out.push_str(&format!("**Wall clock:** {:.1}s\n\n", started.elapsed().as_secs_f64()));

    let chat_errors: usize = runs.iter().filter(|r| r.chat_error.is_some()).count();
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Total rows: {}\n", runs.len()));
    out.push_str(&format!("- Chat errors: {}\n", chat_errors));
    out.push_str(&format!("- Scored: {}\n", score.summary.total));
    out.push_str(&format!("- **Leaks: {}**\n", score.summary.leaks));
    out.push_str(&format!("- Clean: {}\n", score.summary.clean));
    out.push_str(&format!(
        "- Negative controls leaked: {} / {}\n\n",
        score.summary.negative_controls_with_leak,
        score.summary.negative_controls_total,
    ));

    let verdict = if score.summary.leaks == 0 {
        "✅ **PASS** — no PII leakage detected"
    } else {
        "❌ **FAIL** — PII leakage detected; do not promote"
    };
    out.push_str(&format!("{}\n\n", verdict));

    if !score.items.iter().any(|i| i.leaked) {
        return out;
    }

    out.push_str("## Leaking rows\n\n");
    out.push_str("| corpus_id | test_class | echoed | pii hits | negative? |\n");
    out.push_str("|-----------|------------|--------|----------|-----------|\n");
    for it in score.items.iter().filter(|i| i.leaked) {
        let row = runs.iter().find(|r| r.corpus_id == it.corpus_id);
        let test_class = row.map(|r| r.test_class.as_str()).unwrap_or("?");
        let cats = if it.pii_matches_in_response.is_empty() {
            "—".to_string()
        } else {
            it.pii_matches_in_response.join(", ")
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            it.leak_marker,
            test_class,
            if it.marker_echoed { "✅" } else { "—" },
            cats,
            if it.is_negative { "negative" } else { "" },
        ));
    }
    out
}

// ─── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}", e);
            print_help();
            return ExitCode::from(2);
        }
    };

    let started = Instant::now();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("reqwest client init failed: {}", e);
            return ExitCode::from(2);
        }
    };

    // 1. Fetch corpus
    eprintln!("⇣ fetching corpus from {}", args.mimir_url);
    let corpus = match fetch_corpus(&client, &args).await {
        Ok(c) => c,
        Err(e) => { eprintln!("corpus fetch failed: {}", e); return ExitCode::from(2); }
    };
    eprintln!("  → {} rows", corpus.len());
    if corpus.is_empty() {
        eprintln!("error: corpus is empty — is migration sprint50b applied?");
        return ExitCode::from(2);
    }

    // 2. Call agent in parallel (bounded by --concurrency)
    eprintln!("⇒ exercising agent_id={} (concurrency={})", args.agent_id, args.concurrency);
    let runs: Vec<RowRun> = stream::iter(corpus.into_iter())
        .map(|item| {
            let client = client.clone();
            let args = args.clone();
            async move {
                let t = Instant::now();
                let chat_res = call_agent(&client, &args, &item.prompt).await;
                let (response, latency, error) = match chat_res {
                    Ok(c) => (c.content, c.latency_ms, None),
                    Err(e) => (String::new(), t.elapsed().as_millis() as i64, Some(e)),
                };
                RowRun {
                    corpus_id: item.id,
                    test_class: item.test_class,
                    is_negative: item.is_negative,
                    leak_marker: item.leak_marker,
                    response,
                    chat_latency_ms: latency,
                    chat_error: error,
                }
            }
        })
        .buffer_unordered(args.concurrency)
        .collect()
        .await;

    let errors: Vec<&RowRun> = runs.iter().filter(|r| r.chat_error.is_some()).collect();
    if !errors.is_empty() {
        eprintln!("  ⚠ {} agent call(s) failed:", errors.len());
        for r in &errors {
            eprintln!("    - {}: {}", r.corpus_id, r.chat_error.as_ref().unwrap());
        }
    }
    let ok_count = runs.iter().filter(|r| r.chat_error.is_none()).count();
    eprintln!("  → {} responses (avg {}ms)",
        ok_count,
        if ok_count > 0 {
            runs.iter().filter(|r| r.chat_error.is_none())
                .map(|r| r.chat_latency_ms).sum::<i64>() / ok_count as i64
        } else { 0 });

    if ok_count == 0 {
        eprintln!("error: every agent call failed; not scoring");
        return ExitCode::from(2);
    }

    // 3. Score batch
    eprintln!("⇒ POST score-batch");
    let score = match score_batch(&client, &args, &runs).await {
        Ok(s) => s,
        Err(e) => { eprintln!("scoring failed: {}", e); return ExitCode::from(2); }
    };

    // 4. Render report
    let report = render_report(&args, &runs, &score, started);
    println!("{}", report);

    if let Some(path) = &args.output {
        if let Err(e) = std::fs::write(path, &report) {
            eprintln!("warn: failed to write report to {}: {}", path, e);
        } else {
            eprintln!("wrote report to {}", path);
        }
    }

    // 5. Exit
    if score.summary.leaks == 0 {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_handles_special_chars() {
        assert_eq!(urlencoding_minimal("asgard_insurance"), "asgard_insurance");
        assert_eq!(urlencoding_minimal("free text"), "free%20text");
        assert_eq!(urlencoding_minimal("&=?"), "%26%3D%3F");
    }

    #[test]
    fn render_report_no_leaks_renders_pass() {
        let args = Args {
            mimir_url: "http://x".into(),
            tenant_id: "asgard_insurance".into(),
            agent_id: 1,
            auth_token: None,
            concurrency: 1,
            test_class: None,
            output: None,
        };
        let score = ScoreBatchResponse {
            summary: ScoreBatchSummary {
                total: 2, leaks: 0, clean: 2,
                negative_controls_total: 1, negative_controls_with_leak: 0,
            },
            items: vec![],
        };
        let out = render_report(&args, &[], &score, Instant::now());
        assert!(out.contains("PASS"));
        assert!(!out.contains("FAIL"));
    }

    #[test]
    fn render_report_with_leak_renders_fail_and_table() {
        let args = Args {
            mimir_url: "http://x".into(),
            tenant_id: "asgard_insurance".into(),
            agent_id: 1,
            auth_token: None,
            concurrency: 1,
            test_class: None,
            output: None,
        };
        let runs = vec![RowRun {
            corpus_id: "row-1".into(),
            test_class: "free_text".into(),
            is_negative: false,
            leak_marker: "PIITEST-INS-001".into(),
            response: "leaked".into(),
            chat_latency_ms: 100,
            chat_error: None,
        }];
        let score = ScoreBatchResponse {
            summary: ScoreBatchSummary {
                total: 1, leaks: 1, clean: 0,
                negative_controls_total: 0, negative_controls_with_leak: 0,
            },
            items: vec![ScoredRow {
                corpus_id: "row-1".into(),
                leak_marker: "PIITEST-INS-001".into(),
                is_negative: false,
                marker_echoed: true,
                pii_matches_in_response: vec!["thai_national_id".into()],
                leaked: true,
            }],
        };
        let out = render_report(&args, &runs, &score, Instant::now());
        assert!(out.contains("FAIL"));
        assert!(out.contains("PIITEST-INS-001"));
        assert!(out.contains("thai_national_id"));
    }
}
