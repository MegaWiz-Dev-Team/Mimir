//! Live round-trip of Mimir's Gemini request/response code against Vertex AI. Ignored by default
//! (network + a short-lived token); nothing is stored:
//!
//! ```text
//! VERTEX_PROJECT=<gcp-project> VERTEX_ACCESS_TOKEN="$(gcloud auth print-access-token)" \
//!   cargo test -p mimir-core-ai --test gemini3_vertex_live -- --ignored --nocapture
//! ```
//!
//! Proves the body `build_body` produces is accepted by a Gemini 3 model and that `extract_text`
//! reads its answer (Gemini 3 attaches `thoughtSignature` to the answer part). Vertex is also the
//! route the reseller discount applies to, so this doubles as the smoke test for moving
//! `gemini_helper` off the AI Studio key.

use mimir_core_ai::services::gemini_helper::{build_body, extract_json_object, extract_text, GeminiCallConfig, DEFAULT_JUDGE_MODEL};

#[tokio::test]
#[ignore = "needs VERTEX_PROJECT + VERTEX_ACCESS_TOKEN"]
async fn judge_style_call_round_trips_through_vertex() {
    let project = std::env::var("VERTEX_PROJECT").expect("VERTEX_PROJECT");
    let token = std::env::var("VERTEX_ACCESS_TOKEN").expect("VERTEX_ACCESS_TOKEN");
    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_JUDGE_MODEL.to_string());
    let url = format!(
        "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/google/models/{model}:generateContent"
    );

    let cfg = GeminiCallConfig { temperature: 0.2, max_output_tokens: 256, force_json: true, timeout_secs: 60 };
    let body = build_body(
        "Score this answer 1-5 for accuracy. Question: What is 2+2? Answer: 4. Reply as JSON {\"score\": n}.",
        &cfg,
    );
    let resp = reqwest::Client::new().post(&url).bearer_auth(&token).json(&body).send().await.expect("request");
    let status = resp.status();
    let json: serde_json::Value = resp.json().await.expect("json");
    assert!(status.is_success(), "Vertex {status}: {json}");

    let answer_part = &json["candidates"][0]["content"]["parts"][0];
    println!("model={} finish={} part_keys={:?}", json["modelVersion"], json["candidates"][0]["finishReason"],
        answer_part.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()));

    let text = extract_text(&json);
    let scored = extract_json_object(&text).expect("JSON object in answer");
    assert_eq!(scored["score"], 5, "unexpected judge output: {text}");
}
