//! Finetune dataset exporter.
//!
//! Turns scored eval items (`eval_scores`) into a supervised-finetuning (SFT)
//! dataset. The key idea is **rejection sampling**: we only keep examples whose
//! judge (or human-override) scores clear configurable thresholds, and —
//! optionally — whose answer actually matches the benchmark ground truth. This
//! is what prevents training on plausible-but-wrong answers (the failure mode an
//! LLM self-`Yes/No` verifier alone can't catch).
//!
//! The module is split into a **pure core** (`build_jsonl`, `classify`, filter
//! helpers — all unit-testable without a DB) and a thin DB layer
//! (`export_run_to_jsonl`) that reads `eval_scores` and delegates to the core.
//!
//! Default output format is OpenAI-style chat messages JSONL, the de-facto SFT
//! interchange format (consumable by MLX-LM LoRA, llama-factory, axolotl, …).

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::services::db::DbPool;

/// One source row read from `eval_scores`, carrying just the columns the
/// exporter needs. Column names match `SELECT` aliases so `sqlx::FromRow`
/// maps by name.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EvalScoreRow {
    pub question: String,
    pub expected_answer: Option<String>,
    pub actual_answer: Option<String>,
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    pub safety_score: Option<i8>,
    pub human_accuracy_score: Option<i8>,
    pub human_completeness_score: Option<i8>,
    pub human_relevance_score: Option<i8>,
    /// JSON array of retrieval chunks: `[{chunk_id, source, title, score, content_preview}]`.
    pub retrieval_chunks: Option<String>,
    pub agent_name: String,
    pub model_id: String,
}

/// Output record shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// `{"messages":[{role,content}...]}` — OpenAI chat SFT format (default).
    OpenAiMessages,
    /// `{"prompt":..., "completion":...}` — flat prompt/completion pairs.
    PromptCompletion,
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::OpenAiMessages
    }
}

/// Rejection-sampling + formatting knobs.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Minimum effective accuracy/completeness/relevance (1-5). A required
    /// score that is `NULL` is treated as a failure (we can't certify quality).
    pub min_accuracy: i8,
    pub min_completeness: i8,
    pub min_relevance: i8,
    /// If `Some(m)`, effective safety score must be present and `>= m`.
    /// `None` ignores safety entirely (most non-HealthBench runs).
    pub min_safety: Option<i8>,
    /// If true, also require the answer to match `expected_answer` (heuristic,
    /// see [`answer_matches`]). Rows with no `expected_answer` are dropped.
    pub require_ground_truth_match: bool,
    /// Fold retrieved context into the user turn (open-book SFT).
    pub include_context: bool,
    /// Cap on context chunks included per example.
    pub max_context_chunks: usize,
    /// Optional system prompt prepended to every example (e.g. the agent's
    /// system_prompt snapshot).
    pub system_prompt: Option<String>,
    pub format: ExportFormat,
}

impl Default for ExportOptions {
    fn default() -> Self {
        // Conservative defaults: keep only clearly-good examples.
        Self {
            min_accuracy: 4,
            min_completeness: 4,
            min_relevance: 4,
            min_safety: None,
            require_ground_truth_match: false,
            include_context: false,
            max_context_chunks: 5,
            system_prompt: None,
            format: ExportFormat::default(),
        }
    }
}

/// Why a row was dropped (for transparent stats — no silent truncation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// No usable assistant answer (`NULL`/empty `actual_answer`).
    NoAnswer,
    /// One or more required scores missing or below threshold.
    LowScore,
    /// `require_ground_truth_match` set and answer didn't match expected.
    GroundTruthMismatch,
}

/// Per-export accounting. Serializable so an HTTP route can return it.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct ExportStats {
    pub total: usize,
    pub kept: usize,
    pub dropped_no_answer: usize,
    pub dropped_low_score: usize,
    pub dropped_gt_mismatch: usize,
}

/// Prefer a human-reviewed score over the judge's when present.
fn effective(human: Option<i8>, judge: Option<i8>) -> Option<i8> {
    human.or(judge)
}

/// Lowercase, drop punctuation, collapse whitespace.
fn normalize(s: &str) -> String {
    let cleaned: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn token_set(s: &str) -> HashSet<String> {
    normalize(s).split_whitespace().map(|t| t.to_string()).collect()
}

/// Heuristic ground-truth match. True when the normalized expected answer is
/// contained in the actual answer, OR when the actual answer covers at least
/// 60% of the expected answer's tokens. Tuned for short factoid / MCQ golds;
/// intentionally conservative (favours precision of the kept set).
pub fn answer_matches(actual: &str, expected: &str) -> bool {
    let na = normalize(actual);
    let ne = normalize(expected);
    if ne.is_empty() {
        return false;
    }
    if na.contains(&ne) {
        return true;
    }
    let te = token_set(expected);
    if te.is_empty() {
        return false;
    }
    let ta = token_set(actual);
    let covered = te.iter().filter(|t| ta.contains(*t)).count() as f32;
    covered / (te.len() as f32) >= 0.6
}

/// Parse the `retrieval_chunks` JSON, returning up to `max` context snippets
/// (`content_preview`, falling back to `title`).
fn extract_context(retrieval_chunks: Option<&str>, max: usize) -> Vec<String> {
    let Some(raw) = retrieval_chunks else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|c| {
            c.get("content_preview")
                .and_then(|v| v.as_str())
                .or_else(|| c.get("title").and_then(|v| v.as_str()))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .take(max)
        .collect()
}

/// Build the user-turn text, optionally prefixed with retrieved context.
fn user_content(row: &EvalScoreRow, opts: &ExportOptions) -> String {
    if !opts.include_context {
        return row.question.clone();
    }
    let ctx = extract_context(row.retrieval_chunks.as_deref(), opts.max_context_chunks);
    if ctx.is_empty() {
        return row.question.clone();
    }
    let bullets = ctx
        .iter()
        .map(|c| format!("- {c}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Context:\n{bullets}\n\nQuestion: {}", row.question)
}

/// Classify a single row: `Ok(record)` if it survives rejection sampling, or
/// `Err(reason)` explaining the drop.
pub fn classify(row: &EvalScoreRow, opts: &ExportOptions) -> std::result::Result<Value, DropReason> {
    let answer = match row.actual_answer.as_deref().map(str::trim) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return Err(DropReason::NoAnswer),
    };

    let acc = effective(row.human_accuracy_score, row.accuracy_score);
    let comp = effective(row.human_completeness_score, row.completeness_score);
    let rel = effective(row.human_relevance_score, row.relevance_score);

    let meets = |score: Option<i8>, min: i8| score.map(|s| s >= min).unwrap_or(false);
    if !meets(acc, opts.min_accuracy)
        || !meets(comp, opts.min_completeness)
        || !meets(rel, opts.min_relevance)
    {
        return Err(DropReason::LowScore);
    }
    if let Some(min_safety) = opts.min_safety {
        if !meets(row.safety_score, min_safety) {
            return Err(DropReason::LowScore);
        }
    }

    if opts.require_ground_truth_match {
        match row.expected_answer.as_deref() {
            Some(exp) if answer_matches(&answer, exp) => {}
            _ => return Err(DropReason::GroundTruthMismatch),
        }
    }

    Ok(format_record(row, &answer, opts))
}

/// Render a surviving row into the chosen output format.
fn format_record(row: &EvalScoreRow, answer: &str, opts: &ExportOptions) -> Value {
    let user = user_content(row, opts);
    match opts.format {
        ExportFormat::OpenAiMessages => {
            let mut messages = Vec::new();
            if let Some(sys) = &opts.system_prompt {
                messages.push(json!({"role": "system", "content": sys}));
            }
            messages.push(json!({"role": "user", "content": user}));
            messages.push(json!({"role": "assistant", "content": answer}));
            json!({
                "messages": messages,
                // Provenance — handy for dataset auditing / per-agent slicing.
                "meta": {"agent_name": row.agent_name, "model_id": row.model_id},
            })
        }
        ExportFormat::PromptCompletion => {
            let prompt = match &opts.system_prompt {
                Some(sys) => format!("{sys}\n\n{user}"),
                None => user,
            };
            json!({
                "prompt": prompt,
                "completion": answer,
                "meta": {"agent_name": row.agent_name, "model_id": row.model_id},
            })
        }
    }
}

/// Pure core: map rows → JSONL string + accounting. One record per line.
pub fn build_jsonl(rows: &[EvalScoreRow], opts: &ExportOptions) -> (String, ExportStats) {
    let mut stats = ExportStats {
        total: rows.len(),
        ..Default::default()
    };
    let mut lines = Vec::new();
    for row in rows {
        match classify(row, opts) {
            Ok(record) => {
                stats.kept += 1;
                // serde_json::to_string on a Value never fails.
                lines.push(record.to_string());
            }
            Err(DropReason::NoAnswer) => stats.dropped_no_answer += 1,
            Err(DropReason::LowScore) => stats.dropped_low_score += 1,
            Err(DropReason::GroundTruthMismatch) => stats.dropped_gt_mismatch += 1,
        }
    }
    (lines.join("\n"), stats)
}

/// DB layer: read every scored item for `run_id` (scoped to `tenant_id`) and
/// export it. Tenant scoping is mandatory — eval_scores is multi-tenant and a
/// dataset must never leak rows across tenants.
pub async fn export_run_to_jsonl(
    pool: &DbPool,
    run_id: &str,
    tenant_id: &str,
    opts: &ExportOptions,
) -> Result<(String, ExportStats)> {
    let rows: Vec<EvalScoreRow> = sqlx::query_as(
        "SELECT question, expected_answer, actual_answer, \
                accuracy_score, completeness_score, relevance_score, safety_score, \
                human_accuracy_score, human_completeness_score, human_relevance_score, \
                retrieval_chunks, agent_name, model_id \
         FROM eval_scores WHERE run_id = ? AND tenant_id = ?",
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .context("query eval_scores for finetune export")?;

    Ok(build_jsonl(&rows, opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> EvalScoreRow {
        EvalScoreRow {
            question: "What is the first-line treatment for X?".into(),
            expected_answer: Some("Drug A".into()),
            actual_answer: Some("The first-line treatment is Drug A.".into()),
            accuracy_score: Some(5),
            completeness_score: Some(4),
            relevance_score: Some(5),
            safety_score: Some(5),
            human_accuracy_score: None,
            human_completeness_score: None,
            human_relevance_score: None,
            retrieval_chunks: None,
            agent_name: "eir".into(),
            model_id: "gemma-4-26b".into(),
        }
    }

    #[test]
    fn keeps_high_scoring_row() {
        let (jsonl, stats) = build_jsonl(&[row()], &ExportOptions::default());
        assert_eq!(stats.kept, 1);
        assert_eq!(stats.total, 1);
        let v: Value = serde_json::from_str(&jsonl).unwrap();
        let msgs = v["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2); // no system prompt by default
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"], "The first-line treatment is Drug A.");
    }

    #[test]
    fn drops_low_score() {
        let mut r = row();
        r.accuracy_score = Some(2);
        let (jsonl, stats) = build_jsonl(&[r], &ExportOptions::default());
        assert_eq!(stats.kept, 0);
        assert_eq!(stats.dropped_low_score, 1);
        assert!(jsonl.is_empty());
    }

    #[test]
    fn missing_required_score_is_a_drop_not_a_pass() {
        let mut r = row();
        r.accuracy_score = None; // unknown quality must NOT slip through
        let (_, stats) = build_jsonl(&[r], &ExportOptions::default());
        assert_eq!(stats.kept, 0);
        assert_eq!(stats.dropped_low_score, 1);
    }

    #[test]
    fn human_override_beats_judge() {
        let mut r = row();
        r.accuracy_score = Some(1); // judge failed it
        r.human_accuracy_score = Some(5); // reviewer rescued it
        let (_, stats) = build_jsonl(&[r], &ExportOptions::default());
        assert_eq!(stats.kept, 1);
    }

    #[test]
    fn drops_empty_answer() {
        let mut r = row();
        r.actual_answer = Some("   ".into());
        let (_, stats) = build_jsonl(&[r], &ExportOptions::default());
        assert_eq!(stats.dropped_no_answer, 1);
        assert_eq!(stats.kept, 0);
    }

    #[test]
    fn ground_truth_match_gate() {
        let mut r = row();
        r.actual_answer = Some("It is Drug B, definitely.".into()); // wrong
        let opts = ExportOptions {
            require_ground_truth_match: true,
            ..Default::default()
        };
        let (_, stats) = build_jsonl(&[r.clone()], &opts);
        assert_eq!(stats.dropped_gt_mismatch, 1);

        // Correct answer passes the same gate.
        let (_, stats2) = build_jsonl(&[row()], &opts);
        assert_eq!(stats2.kept, 1);
    }

    #[test]
    fn answer_matches_is_containment_or_coverage() {
        assert!(answer_matches("the answer is Drug A", "Drug A"));
        assert!(answer_matches("Drug A", "drug a")); // case/space-insensitive
        assert!(!answer_matches("Drug B", "Drug A"));
        // coverage: expected tokens largely present
        assert!(answer_matches(
            "acute myocardial infarction treatment",
            "acute myocardial infarction"
        ));
        assert!(!answer_matches("", "Drug A"));
        assert!(!answer_matches("Drug A", "")); // empty gold never matches
    }

    #[test]
    fn safety_gate_optional() {
        let mut r = row();
        r.safety_score = Some(-3);
        // default ignores safety
        assert_eq!(build_jsonl(&[r.clone()], &ExportOptions::default()).1.kept, 1);
        // opt-in safety floor drops it
        let opts = ExportOptions {
            min_safety: Some(0),
            ..Default::default()
        };
        assert_eq!(build_jsonl(&[r], &opts).1.dropped_low_score, 1);
    }

    #[test]
    fn context_folded_into_user_turn() {
        let mut r = row();
        r.retrieval_chunks = Some(
            json!([
                {"chunk_id": "c1", "content_preview": "Drug A is indicated for X."},
                {"chunk_id": "c2", "title": "Fallback Title"}
            ])
            .to_string(),
        );
        let opts = ExportOptions {
            include_context: true,
            ..Default::default()
        };
        let (jsonl, stats) = build_jsonl(&[r], &opts);
        assert_eq!(stats.kept, 1);
        let v: Value = serde_json::from_str(&jsonl).unwrap();
        let user = v["messages"][0]["content"].as_str().unwrap();
        assert!(user.contains("Drug A is indicated for X."));
        assert!(user.contains("Fallback Title")); // title fallback when no preview
        assert!(user.contains("Question: What is the first-line treatment for X?"));
    }

    #[test]
    fn system_prompt_and_prompt_completion_format() {
        let opts = ExportOptions {
            system_prompt: Some("You are Eir.".into()),
            format: ExportFormat::PromptCompletion,
            ..Default::default()
        };
        let (jsonl, _) = build_jsonl(&[row()], &opts);
        let v: Value = serde_json::from_str(&jsonl).unwrap();
        assert!(v["prompt"].as_str().unwrap().starts_with("You are Eir."));
        assert_eq!(v["completion"], "The first-line treatment is Drug A.");
        assert_eq!(v["meta"]["agent_name"], "eir");
    }

    #[test]
    fn stats_account_for_every_row() {
        let mut low = row();
        low.relevance_score = Some(1);
        let mut empty = row();
        empty.actual_answer = None;
        let rows = vec![row(), low, empty];
        let (_, stats) = build_jsonl(&rows, &ExportOptions::default());
        assert_eq!(stats.total, 3);
        assert_eq!(
            stats.kept + stats.dropped_low_score + stats.dropped_no_answer + stats.dropped_gt_mismatch,
            stats.total
        );
    }
}
