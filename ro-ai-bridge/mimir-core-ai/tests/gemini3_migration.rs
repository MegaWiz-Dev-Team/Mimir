//! Gemini 2.5 is retired (Vertex + Gemini API: public access ends 2026-10-20; shutdown 2027-01-28
//! for 2.5 Flash-Lite, 2027-03-31 for 2.5 Flash and Pro). Gemini 3.5 Flash-Lite is Google's
//! like-for-like successor to 2.5 Flash at the same price. Gemini 3 responses may carry
//! `thought: true` summary parts and a `thoughtSignature` on the answer part; the parser must
//! ignore both. (Integration test on purpose: the crate's in-tree unit tests do not compile on
//! main for unrelated reasons.)

use mimir_core_ai::services::gemini_helper::{
    build_body, extract_text, GeminiCallConfig, DEFAULT_GEMINI_MODEL, DEFAULT_INSIGHT_MODEL, DEFAULT_JUDGE_MODEL,
};
use mimir_core_ai::services::model_sync::default_models;

#[test]
fn judge_and_insight_defaults_are_gemini_3_5_flash_lite() {
    assert_eq!(DEFAULT_JUDGE_MODEL, "gemini-3.5-flash-lite");
    assert_eq!(DEFAULT_INSIGHT_MODEL, "gemini-3.5-flash-lite");
    assert_eq!(DEFAULT_GEMINI_MODEL, DEFAULT_JUDGE_MODEL);
}

#[test]
fn build_body_is_one_user_turn_and_honours_force_json() {
    let cfg = GeminiCallConfig { temperature: 0.2, max_output_tokens: 99, force_json: true, timeout_secs: 1 };
    let body = build_body("hi", &cfg);
    assert_eq!(body["contents"].as_array().map(Vec::len), Some(1));
    // Vertex rejects a turn without a role ("Please use a valid role: user, model"); AI Studio
    // merely tolerates its absence. Always send it so one body serves both routes.
    assert_eq!(body["contents"][0]["role"], "user");
    assert_eq!(body["contents"][0]["parts"][0]["text"], "hi");
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 99);
    assert_eq!(body["generationConfig"]["response_mime_type"], "application/json");

    let plain = build_body("hi", &GeminiCallConfig { force_json: false, ..cfg });
    assert!(plain["generationConfig"].get("response_mime_type").is_none());
}

#[test]
fn extract_text_keeps_signed_answer_and_skips_thought_parts() {
    // Shape observed from gemini-3.5-flash-lite on 2026-09-15: the answer part carries a signature.
    let signed = serde_json::json!({"candidates":[{"content":{"parts":[
        {"text":"{\"score\":4}","thoughtSignature":"CqUB..."}]}}]});
    assert_eq!(extract_text(&signed), "{\"score\":4}");

    let with_thoughts = serde_json::json!({"candidates":[{"content":{"parts":[
        {"text":"let me think","thought":true},
        {"text":"{\"score\":"},
        {"text":"4}"}]}}]});
    assert_eq!(extract_text(&with_thoughts), "{\"score\":4}");

    let only_thought = serde_json::json!({"candidates":[{"content":{"parts":[{"text":"x","thought":true}]}}]});
    assert_eq!(extract_text(&only_thought), "");
}

/// The fallback catalog (used when Heimdall is unreachable) must only offer models that will still
/// answer, under their GA ids.
#[test]
fn default_cloud_catalog_offers_no_retired_gemini_25() {
    let google: Vec<&str> = default_models().iter().filter(|(p, _, _)| *p == "google").map(|(_, m, _)| *m).collect();
    assert!(google.iter().all(|m| !m.contains("gemini-2.5")), "retired model in catalog: {google:?}");
    for expected in ["gemini-3.1-flash-lite", "gemini-3.5-flash-lite", "gemini-3.5-flash"] {
        assert!(google.contains(&expected), "missing {expected} in {google:?}");
    }
    assert!(!google.contains(&"gemini-3.1-flash-lite-preview"), "3.1 Flash-Lite is GA; drop the preview id");
}
