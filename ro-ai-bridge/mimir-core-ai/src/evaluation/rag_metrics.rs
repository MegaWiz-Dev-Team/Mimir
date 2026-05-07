//! Sprint 47 — Mimir RAG Eval (Rust-native RAGAS-equivalent)
//!
//! 6 metrics in 2 categories:
//!
//! **LLM-as-judge (RAGAS-style, no gold labels needed):**
//!   - Faithfulness        — does the answer cite from retrieved context?
//!   - AnswerRelevancy     — does the answer address the question?
//!   - ContextPrecision    — fraction of retrieved chunks that are relevant
//!   - ContextRecall       — fraction of gold relevant info captured (needs gold)
//!
//! **Pure-Rust retrieval metrics (need gold relevant chunk_ids):**
//!   - RecallAtK / MeanReciprocalRank / NormalizedDcgAtK / HitRateAtK
//!
//! All judge metrics reuse the existing `gemini_helper::call_text` LLM client
//! (so we get the multi-tenant key resolution + cost tracking + retries that
//! the rest of the eval pipeline uses).
//!
//! Production deploy plan (Sprint 47):
//!   B-47a ✅ DB migration (eval_scores extension + rag_benchmark_items)
//!   B-47b ⏳ this file — trait + first metric (Faithfulness)
//!   B-47b cont. — AnswerRelevancy / ContextPrecision / ContextRecall
//!   B-47c    — wire retrieved_chunk_ids capture from agent invocation
//!   B-47d    — pure-Rust retrieval metrics

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::services::gemini_helper::{self, GeminiCallConfig};

/// Default judge for RAGAS metrics. Pinned per the Sprint 47 design (single
/// judge v0; multi-judge is opt-in via opt_in flag — see B-47b extension).
pub const DEFAULT_RAG_JUDGE_MODEL: &str = "gemini-3.1-flash-lite-preview";

/// Inputs to a RAG metric scorer. Borrowed-slice form — fast path for
/// per-row scoring; not Serializable (constructed in-memory, not from JSON).
#[derive(Debug, Clone)]
pub struct RagMetricInput<'a> {
    pub question: &'a str,
    /// Retrieved chunks (already concatenated upstream is fine, but pass as
    /// slice so per-chunk metrics — like ContextPrecision — can iterate).
    pub context: &'a [String],
    pub answer: &'a str,
    /// Ground-truth answer text — used by ContextRecall. None for the
    /// other 3 RAGAS metrics.
    pub gold_answer: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagMetricOutput {
    pub score: f64,
    pub reasoning: Option<String>,
    /// Raw judge response (for audit + re-judge support).
    pub raw_judge: Option<String>,
    pub judge_model: String,
}

#[async_trait]
pub trait RagMetric: Send + Sync {
    fn name(&self) -> &'static str;
    async fn score(&self, input: &RagMetricInput<'_>) -> Result<RagMetricOutput>;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn truncate_for_prompt(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars { s.to_string() }
    else { format!("{}…[truncated]", &s[..max_chars]) }
}

fn join_context(chunks: &[String], max_total_chars: usize) -> String {
    let per_chunk_budget = (max_total_chars / chunks.len().max(1)).min(2048);
    let mut out = String::with_capacity(max_total_chars);
    for (i, c) in chunks.iter().enumerate() {
        out.push_str(&format!("\n--- chunk {} ---\n", i + 1));
        out.push_str(&truncate_for_prompt(c, per_chunk_budget));
    }
    out
}

/// Parse a `{"score": float, "reasoning": str}` JSON response from a judge LLM.
/// Tolerant of code-fence wrapping, leading text, etc.
fn parse_judge_score(raw: &str) -> Result<(f64, Option<String>)> {
    // Strip code fences ```json ... ```
    let stripped = raw.trim();
    let cleaned = if let Some(start) = stripped.find('{') {
        if let Some(end) = stripped.rfind('}') {
            &stripped[start..=end]
        } else { stripped }
    } else { stripped };

    let v: JsonValue = serde_json::from_str(cleaned)
        .map_err(|e| anyhow!("judge returned non-JSON: {} (raw: {:.200})", e, raw))?;

    let score = v.get("score")
        .and_then(|x| x.as_f64())
        .ok_or_else(|| anyhow!("judge JSON missing 'score' float field: {}", cleaned))?;
    // Clamp to [0,1] — defensive against rare judge-misformat
    let score = score.clamp(0.0, 1.0);
    let reasoning = v.get("reasoning")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    Ok((score, reasoning))
}

async fn run_judge(
    judge_model: &str,
    prompt: &str,
) -> Result<(f64, Option<String>, String)> {
    let cfg = GeminiCallConfig {
        temperature: 0.0,
        max_output_tokens: 512,
        force_json: true,
        timeout_secs: 60,
    };
    let resp = gemini_helper::call_text(judge_model, prompt, &cfg).await?;
    let raw = resp.text.clone();
    let (score, reasoning) = parse_judge_score(&raw)?;
    Ok((score, reasoning, raw))
}

// ─── Metric: Faithfulness ───────────────────────────────────────────────────

const FAITHFULNESS_PROMPT_TEMPLATE: &str = r#"You are scoring whether an AI medical assistant's answer is FAITHFUL to its retrieved context.

Faithfulness = the answer makes claims that ARE supported by the retrieved context. A faithful answer doesn't fabricate facts that aren't in the context. Hallucination drops the score.

Question:
{question}

Retrieved context:
{context}

Answer to score:
{answer}

Scoring rubric (0.0 - 1.0):
  1.0  Every factual claim in the answer is directly supported by the retrieved context.
  0.7  Most claims supported; minor unsupported elaboration is acceptable.
  0.5  Half-supported, half-fabricated.
  0.3  Most claims are not in context; partial overlap only.
  0.0  Answer is fabricated; no support in context.

Common sense + safety hedging language ("consult a clinician", "this is general info") does NOT count as fabrication. Score only the factual medical claims.

Return JSON ONLY (no markdown, no preamble):
{"score": <float 0-1>, "reasoning": "<one short sentence>"}"#;

pub struct Faithfulness {
    pub judge_model: String,
}

impl Default for Faithfulness {
    fn default() -> Self {
        Self { judge_model: DEFAULT_RAG_JUDGE_MODEL.to_string() }
    }
}

#[async_trait]
impl RagMetric for Faithfulness {
    fn name(&self) -> &'static str { "faithfulness" }

    async fn score(&self, input: &RagMetricInput<'_>) -> Result<RagMetricOutput> {
        let context = join_context(input.context, 6000);
        let prompt = FAITHFULNESS_PROMPT_TEMPLATE
            .replace("{question}", &truncate_for_prompt(input.question, 2000))
            .replace("{context}", &context)
            .replace("{answer}", &truncate_for_prompt(input.answer, 4000));

        let (score, reasoning, raw) = run_judge(&self.judge_model, &prompt).await?;

        Ok(RagMetricOutput {
            score,
            reasoning,
            raw_judge: Some(raw),
            judge_model: self.judge_model.clone(),
        })
    }
}

// ─── Metric: Answer Relevancy ───────────────────────────────────────────────

const ANSWER_RELEVANCY_PROMPT_TEMPLATE: &str = r#"You are scoring how well an AI medical assistant's answer ADDRESSES the question.

Answer Relevancy = the answer actually addresses what the user asked. An off-topic, evasive, or generic answer drops the score.

Question:
{question}

Answer to score:
{answer}

Scoring rubric (0.0 - 1.0):
  1.0  Answer directly addresses the question with the right level of detail.
  0.7  Mostly addresses; minor digressions.
  0.5  Partial — answers half the question or stays too generic.
  0.3  Tangential — talks about adjacent topic but misses the point.
  0.0  Completely off-topic or refuses without justification.

Disclaimers like "consult a physician" do NOT lower the score if the answer is otherwise relevant. Score only relevance to the question, not factual correctness.

Return JSON ONLY (no markdown, no preamble):
{"score": <float 0-1>, "reasoning": "<one short sentence>"}"#;

pub struct AnswerRelevancy {
    pub judge_model: String,
}

impl Default for AnswerRelevancy {
    fn default() -> Self { Self { judge_model: DEFAULT_RAG_JUDGE_MODEL.to_string() } }
}

#[async_trait]
impl RagMetric for AnswerRelevancy {
    fn name(&self) -> &'static str { "answer_relevancy" }

    async fn score(&self, input: &RagMetricInput<'_>) -> Result<RagMetricOutput> {
        let prompt = ANSWER_RELEVANCY_PROMPT_TEMPLATE
            .replace("{question}", &truncate_for_prompt(input.question, 2000))
            .replace("{answer}", &truncate_for_prompt(input.answer, 4000));

        let (score, reasoning, raw) = run_judge(&self.judge_model, &prompt).await?;
        Ok(RagMetricOutput {
            score, reasoning, raw_judge: Some(raw),
            judge_model: self.judge_model.clone(),
        })
    }
}

// ─── Metric: Context Precision ──────────────────────────────────────────────

const CONTEXT_PRECISION_PROMPT_TEMPLATE: &str = r#"You are scoring CONTEXT PRECISION — what fraction of the retrieved chunks are RELEVANT to the question.

A chunk is relevant if its content can plausibly help answer the question. Tangential chunks lower the score; outright irrelevant chunks lower it more.

Question:
{question}

Retrieved chunks (numbered):
{context}

Scoring rubric (0.0 - 1.0):
  1.0  Every retrieved chunk is on-topic and useful.
  0.75 Most chunks relevant; 1-2 noisy.
  0.5  Half-and-half — relevant + irrelevant mixed.
  0.25 Only a few chunks are useful; mostly noise.
  0.0  None of the chunks are relevant to the question.

Return JSON ONLY (no markdown, no preamble):
{"score": <float 0-1>, "reasoning": "<one short sentence>"}"#;

pub struct ContextPrecision {
    pub judge_model: String,
}

impl Default for ContextPrecision {
    fn default() -> Self { Self { judge_model: DEFAULT_RAG_JUDGE_MODEL.to_string() } }
}

#[async_trait]
impl RagMetric for ContextPrecision {
    fn name(&self) -> &'static str { "context_precision" }

    async fn score(&self, input: &RagMetricInput<'_>) -> Result<RagMetricOutput> {
        let context = join_context(input.context, 6000);
        let prompt = CONTEXT_PRECISION_PROMPT_TEMPLATE
            .replace("{question}", &truncate_for_prompt(input.question, 2000))
            .replace("{context}", &context);
        let (score, reasoning, raw) = run_judge(&self.judge_model, &prompt).await?;
        Ok(RagMetricOutput {
            score, reasoning, raw_judge: Some(raw),
            judge_model: self.judge_model.clone(),
        })
    }
}

// ─── Metric: Context Recall (needs gold answer) ─────────────────────────────

const CONTEXT_RECALL_PROMPT_TEMPLATE: &str = r#"You are scoring CONTEXT RECALL — does the retrieved context contain enough information to support the gold answer?

For each fact in the gold answer, check whether the retrieved chunks contain or imply it. Higher score = more facts supported.

Question:
{question}

Retrieved chunks:
{context}

Gold answer (ground truth):
{gold_answer}

Scoring rubric (0.0 - 1.0):
  1.0  All key facts in the gold answer are supported by the retrieved chunks.
  0.7  Most facts supported; minor gaps.
  0.5  Half supported, half missing from context.
  0.3  Only a few facts are recoverable from the context.
  0.0  Context contains none of the facts the gold answer requires.

Return JSON ONLY (no markdown, no preamble):
{"score": <float 0-1>, "reasoning": "<one short sentence>"}"#;

pub struct ContextRecall {
    pub judge_model: String,
}

impl Default for ContextRecall {
    fn default() -> Self { Self { judge_model: DEFAULT_RAG_JUDGE_MODEL.to_string() } }
}

#[async_trait]
impl RagMetric for ContextRecall {
    fn name(&self) -> &'static str { "context_recall" }

    async fn score(&self, input: &RagMetricInput<'_>) -> Result<RagMetricOutput> {
        let gold = input.gold_answer.ok_or_else(||
            anyhow!("ContextRecall requires gold_answer; got None"))?;
        let context = join_context(input.context, 6000);
        let prompt = CONTEXT_RECALL_PROMPT_TEMPLATE
            .replace("{question}", &truncate_for_prompt(input.question, 2000))
            .replace("{context}", &context)
            .replace("{gold_answer}", &truncate_for_prompt(gold, 3000));
        let (score, reasoning, raw) = run_judge(&self.judge_model, &prompt).await?;
        Ok(RagMetricOutput {
            score, reasoning, raw_judge: Some(raw),
            judge_model: self.judge_model.clone(),
        })
    }
}

// ─── Pure-Rust retrieval metrics (B-47d) ────────────────────────────────────
//
// Need gold rag_benchmark_items.relevant_chunk_ids. Computed locally, no LLM.
//
// Inputs:
//   retrieved: Vec<String>  — chunk IDs returned by RAG (in rank order)
//   relevant:  Vec<String>  — gold relevant chunk IDs (set, order-agnostic)

/// Recall@k = |retrieved[..k] ∩ relevant| / |relevant|
pub fn recall_at_k(retrieved: &[String], relevant: &[String], k: usize) -> f64 {
    if relevant.is_empty() { return 0.0; }
    use std::collections::HashSet;
    let rel: HashSet<&String> = relevant.iter().collect();
    let hits = retrieved.iter().take(k).filter(|r| rel.contains(*r)).count();
    hits as f64 / relevant.len() as f64
}

/// MRR = 1 / rank_of_first_relevant_retrieved (0 if none).
pub fn mean_reciprocal_rank(retrieved: &[String], relevant: &[String]) -> f64 {
    use std::collections::HashSet;
    let rel: HashSet<&String> = relevant.iter().collect();
    for (i, r) in retrieved.iter().enumerate() {
        if rel.contains(r) {
            return 1.0 / (i + 1) as f64;
        }
    }
    0.0
}

/// Hit rate @ k = 1 if any relevant chunk in retrieved[..k], else 0.
pub fn hit_rate_at_k(retrieved: &[String], relevant: &[String], k: usize) -> f64 {
    use std::collections::HashSet;
    let rel: HashSet<&String> = relevant.iter().collect();
    if retrieved.iter().take(k).any(|r| rel.contains(r)) { 1.0 } else { 0.0 }
}

/// NDCG@k with binary relevance (1 if in `relevant`, 0 otherwise).
pub fn ndcg_at_k(retrieved: &[String], relevant: &[String], k: usize) -> f64 {
    use std::collections::HashSet;
    if relevant.is_empty() { return 0.0; }
    let rel: HashSet<&String> = relevant.iter().collect();

    // DCG: sum of (rel_i / log2(i+2)) for i in 0..k
    let dcg: f64 = retrieved.iter().take(k).enumerate().map(|(i, r)| {
        let r_i = if rel.contains(r) { 1.0 } else { 0.0 };
        r_i / ((i + 2) as f64).log2()
    }).sum();

    // Ideal DCG: relevance vector all 1s for first min(k, |relevant|) positions.
    let n_rel = relevant.len().min(k);
    let idcg: f64 = (0..n_rel).map(|i| 1.0 / ((i + 2) as f64).log2()).sum();

    if idcg == 0.0 { 0.0 } else { dcg / idcg }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_judge_score_clean_json() {
        let raw = r#"{"score": 0.85, "reasoning": "All claims grounded"}"#;
        let (s, r) = parse_judge_score(raw).unwrap();
        assert!((s - 0.85).abs() < 1e-9);
        assert_eq!(r.as_deref(), Some("All claims grounded"));
    }

    #[test]
    fn parse_judge_score_markdown_wrapped() {
        let raw = "```json\n{\"score\": 0.5, \"reasoning\": \"partial\"}\n```";
        let (s, _) = parse_judge_score(raw).unwrap();
        assert!((s - 0.5).abs() < 1e-9);
    }

    #[test]
    fn parse_judge_score_clamps() {
        let raw = r#"{"score": 1.5, "reasoning": "over"}"#;
        let (s, _) = parse_judge_score(raw).unwrap();
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_judge_score_no_score_field_errors() {
        let raw = r#"{"reasoning": "no score"}"#;
        assert!(parse_judge_score(raw).is_err());
    }

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn recall_at_k_basic() {
        let retrieved = s(&["c1", "c2", "c3", "c4", "c5"]);
        let relevant  = s(&["c2", "c5", "c99"]);
        assert!((recall_at_k(&retrieved, &relevant, 5) - 2.0/3.0).abs() < 1e-9);
        assert!((recall_at_k(&retrieved, &relevant, 2) - 1.0/3.0).abs() < 1e-9);
        assert_eq!(recall_at_k(&retrieved, &[], 5), 0.0);
    }

    #[test]
    fn mrr_first_hit_at_position() {
        assert!((mean_reciprocal_rank(&s(&["a","b","c"]), &s(&["c"])) - 1.0/3.0).abs() < 1e-9);
        assert_eq!(mean_reciprocal_rank(&s(&["a","b","c"]), &s(&["x"])), 0.0);
        assert_eq!(mean_reciprocal_rank(&s(&["a","b","c"]), &s(&["a"])), 1.0);
    }

    #[test]
    fn hit_rate_simple() {
        assert_eq!(hit_rate_at_k(&s(&["a","b"]), &s(&["b"]), 5), 1.0);
        assert_eq!(hit_rate_at_k(&s(&["a","b"]), &s(&["c"]), 5), 0.0);
        assert_eq!(hit_rate_at_k(&s(&["a","b","c"]), &s(&["c"]), 2), 0.0); // c at rank 3, k=2
    }

    #[test]
    fn ndcg_perfect_match_is_one() {
        // All relevant retrieved at top → ideal DCG / DCG = 1.0
        let retrieved = s(&["x","y","z"]);
        let relevant  = s(&["x","y","z"]);
        let v = ndcg_at_k(&retrieved, &relevant, 3);
        assert!((v - 1.0).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn ndcg_no_overlap_is_zero() {
        let retrieved = s(&["a","b"]);
        let relevant  = s(&["x","y"]);
        assert_eq!(ndcg_at_k(&retrieved, &relevant, 5), 0.0);
    }
}
