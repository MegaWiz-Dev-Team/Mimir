//! Promotion path: Bifrost memvid session → Mimir Well artifacts.
//!
//! ADR-011 §D5. Bifrost calls POST /mimir/well/promote at session end (or
//! explicit `commit_to_well` MCP tool). This module:
//!   1. Receives session frames + span tree + outcome
//!   2. Runs each frame through a local tier classifier (gemma-4-1b)
//!   3. Drops noise, emits WriteRequests for episodic/semantic/procedural

use crate::model::*;

/// Tier classifier output. `Drop` for system/noise frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameClassification {
    /// Drop — system message, intermediate scratch, no value.
    Drop,
    /// Promote to episodic.
    Episodic,
    /// Promote to semantic.
    Semantic,
    /// Promote to procedural (chain-of-spans).
    Procedural,
}

impl FrameClassification {
    /// Convert to [`Tier`] for promotion. None for `Drop`.
    pub fn to_tier(self) -> Option<Tier> {
        match self {
            Self::Drop => None,
            Self::Episodic => Some(Tier::Episodic),
            Self::Semantic => Some(Tier::Semantic),
            Self::Procedural => Some(Tier::Procedural),
        }
    }
}

/// A single promotion candidate from Bifrost.
#[derive(Debug, Clone)]
pub struct PromotionFrame {
    /// Frame text content from memvid `.mv2`.
    pub content: String,
    /// Optional title.
    pub title: Option<String>,
    /// Owning trace_id:span_id (becomes prov_generated_by).
    pub span_ref: String,
}

use crate::{Result, WellError};
use crate::writer::{WellWriter, WriteRequest};
use crate::model::{Kind, Surface};

/// Report from one promotion session run.
#[derive(Debug, Default, Clone)]
pub struct PromotionReport {
    /// Frames classified as drop (no write).
    pub dropped: u64,
    /// Episodic artifacts written.
    pub episodic: u64,
    /// Semantic artifacts written.
    pub semantic: u64,
    /// Procedural artifacts written.
    pub procedural: u64,
    /// Frames that errored during classify (treated as drop).
    pub errored: u64,
}

/// Promote a Bifrost session: classify each frame and write non-drop ones.
///
/// Errors from the classifier are treated as `Drop` (logged + counted) so a
/// flaky LLM never blocks promotion — better to lose a frame than crash.
pub async fn promote_session(
    frames: &[PromotionFrame],
    session_id: &str,
    tenant_id: &str,
    agent_id: &str,
    classifier: &dyn TierClassifier,
    writer: &WellWriter,
) -> Result<PromotionReport> {
    use sha2::{Digest, Sha256};
    let mut report = PromotionReport::default();

    for frame in frames {
        let classification = match classifier.classify(frame).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    span_ref = frame.span_ref,
                    error = %e,
                    "promote_session: classifier errored, treating as drop"
                );
                report.errored += 1;
                continue;
            }
        };
        let Some(tier) = classification.to_tier() else {
            report.dropped += 1;
            continue;
        };

        let mut h = Sha256::new();
        h.update(frame.content.as_bytes());
        let content_hash = format!("{:x}", h.finalize());

        let req = WriteRequest {
            tenant_id: tenant_id.into(),
            agent_id: agent_id.into(),
            case_id: None,
            kind: Kind::Observation,
            tier,
            surface: Surface::from(tier),
            content_hash,
            content: serde_json::json!({
                "text": frame.content,
                "title": frame.title,
            }),
            embedding: None,
            prov_used: None,
            prov_generated_by: frame.span_ref.clone(),
            confidence: None,
            promoted_from: Some(session_id.into()),
        };

        match writer.write(req).await {
            Ok(_) => match classification {
                FrameClassification::Episodic => report.episodic += 1,
                FrameClassification::Semantic => report.semantic += 1,
                FrameClassification::Procedural => report.procedural += 1,
                FrameClassification::Drop => unreachable!("drop handled above"),
            },
            Err(e) => {
                tracing::warn!(
                    span_ref = frame.span_ref,
                    error = %e,
                    "promote_session: write failed"
                );
                report.errored += 1;
            }
        }
    }
    Ok(report)
}

/// Trait for classifying a single Bifrost frame into a promotion tier.
#[async_trait::async_trait]
pub trait TierClassifier: Send + Sync {
    /// Classify a frame.
    async fn classify(&self, frame: &PromotionFrame) -> Result<FrameClassification>;
}

/// Stub classifier — returns a configured decision regardless of input.
/// Useful for tests and as a circuit-breaker fallback.
pub struct StubTierClassifier {
    /// Decision to return.
    pub decision: FrameClassification,
}

#[async_trait::async_trait]
impl TierClassifier for StubTierClassifier {
    async fn classify(&self, _: &PromotionFrame) -> Result<FrameClassification> {
        Ok(self.decision)
    }
}

/// Heimdall-backed classifier — calls Heimdall's OpenAI-compatible
/// `/v1/chat/completions`. Local model (gemma family) per Asgard local-first
/// rule.
pub struct HeimdallTierClassifier {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl HeimdallTierClassifier {
    /// Construct.
    ///
    /// `base_url` example: `http://localhost:8080/v1`
    /// `model` example:    `mlx-community/gemma-4-26b-a4b-it-4bit`
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Build the classifier prompt. Single-shot: instruction + frame, expects
    /// JSON `{"tier":"drop|episodic|semantic|procedural"}`.
    pub(crate) fn build_prompt(frame: &PromotionFrame) -> String {
        format!(
            "You classify Bifrost session frames for Mimir Well promotion. \
             Reply with ONLY a JSON object like {{\"tier\":\"X\"}} where X is \
             exactly one of: drop, episodic, semantic, procedural.\n\
             - drop:       system noise, intermediate scratch, no future value\n\
             - episodic:   a specific event tied to time/case (\"on date X, case Y did Z\")\n\
             - semantic:   a tenant-level fact, preference, or learned rule\n\
             - procedural: a multi-step decision sequence worth replaying\n\n\
             Frame title: {}\n\
             Frame content:\n{}\n",
            frame.title.as_deref().unwrap_or("(no title)"),
            frame.content
        )
    }

    /// Parse model output (expecting a JSON tier object) into [`FrameClassification`].
    pub(crate) fn parse_response(content: &str) -> Result<FrameClassification> {
        // Tolerate surrounding noise — locate first `{` ... `}` block.
        let start = content.find('{').ok_or_else(|| {
            WellError::Upstream(format!("no JSON object in classifier reply: {content:?}"))
        })?;
        let end = content[start..].find('}').ok_or_else(|| {
            WellError::Upstream(format!("unclosed JSON object: {content:?}"))
        })?;
        let json = &content[start..=start + end];
        let parsed: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| WellError::Upstream(format!("classifier json: {e} from {json:?}")))?;
        let tier = parsed["tier"].as_str().ok_or_else(|| {
            WellError::Upstream(format!("missing 'tier' field in {json}"))
        })?;
        match tier {
            "drop" => Ok(FrameClassification::Drop),
            "episodic" => Ok(FrameClassification::Episodic),
            "semantic" => Ok(FrameClassification::Semantic),
            "procedural" => Ok(FrameClassification::Procedural),
            other => Err(WellError::Upstream(format!(
                "unknown tier value: {other:?}"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl TierClassifier for HeimdallTierClassifier {
    async fn classify(&self, frame: &PromotionFrame) -> Result<FrameClassification> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "user", "content": Self::build_prompt(frame) }
            ],
            "max_tokens": 32,
            "temperature": 0.0,
        });
        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url.trim_end_matches('/')))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(WellError::Upstream(format!(
                "heimdall returned status {}",
                resp.status()
            )));
        }
        let payload: serde_json::Value = resp.json().await?;
        let content = payload["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                WellError::Upstream(format!(
                    "missing choices[0].message.content in {payload}"
                ))
            })?;
        Self::parse_response(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_to_tier_mapping() {
        assert_eq!(FrameClassification::Drop.to_tier(), None);
        assert_eq!(
            FrameClassification::Episodic.to_tier(),
            Some(Tier::Episodic)
        );
        assert_eq!(
            FrameClassification::Semantic.to_tier(),
            Some(Tier::Semantic)
        );
        assert_eq!(
            FrameClassification::Procedural.to_tier(),
            Some(Tier::Procedural)
        );
    }

    #[test]
    fn parse_response_handles_clean_json() {
        let r = HeimdallTierClassifier::parse_response(r#"{"tier":"semantic"}"#).unwrap();
        assert_eq!(r, FrameClassification::Semantic);
    }

    #[test]
    fn parse_response_tolerates_surrounding_noise() {
        // Some local models prepend "```json" / trailing newlines / extra text.
        let r = HeimdallTierClassifier::parse_response(
            "```json\n{\"tier\":\"procedural\"}\n```",
        )
        .unwrap();
        assert_eq!(r, FrameClassification::Procedural);
    }

    #[test]
    fn parse_response_handles_all_four_tiers() {
        for (tier_str, expected) in [
            ("drop", FrameClassification::Drop),
            ("episodic", FrameClassification::Episodic),
            ("semantic", FrameClassification::Semantic),
            ("procedural", FrameClassification::Procedural),
        ] {
            let s = format!(r#"{{"tier":"{tier_str}"}}"#);
            assert_eq!(
                HeimdallTierClassifier::parse_response(&s).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn parse_response_rejects_unknown_tier() {
        let r = HeimdallTierClassifier::parse_response(r#"{"tier":"invalid"}"#);
        assert!(matches!(r, Err(crate::WellError::Upstream(_))));
    }

    #[test]
    fn build_prompt_includes_all_four_tier_labels() {
        let frame = PromotionFrame {
            content: "Patient said hello.".into(),
            title: Some("hello".into()),
            span_ref: "trace:span".into(),
        };
        let p = HeimdallTierClassifier::build_prompt(&frame);
        for label in ["drop", "episodic", "semantic", "procedural"] {
            assert!(p.contains(label), "prompt missing label {label}");
        }
        assert!(p.contains("Patient said hello."));
    }

    #[tokio::test]
    async fn stub_classifier_returns_configured_value() {
        let stub = StubTierClassifier {
            decision: FrameClassification::Semantic,
        };
        let r = stub
            .classify(&PromotionFrame {
                content: "anything".into(),
                title: None,
                span_ref: "x".into(),
            })
            .await
            .unwrap();
        assert_eq!(r, FrameClassification::Semantic);
    }
}
