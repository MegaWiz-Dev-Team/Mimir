use super::{AtomicFact, WikiChunk};
use crate::services::llm_router::UniversalClient;
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct FactList {
    pub facts: Vec<AtomicFact>,
}

pub struct ACUExtractorAgent;

pub async fn extract_acus(
    client: &UniversalClient,
    model: &str,
    chunk: &WikiChunk,
) -> Result<Vec<AtomicFact>> {
    info!("      Extracting ACUs (Agent approach)...");

    let preamble = "You are an expert knowledge extractor. Extract atomic facts from the text. \
                   Return ONLY a valid JSON object with a 'facts' key containing a list of objects. \
                   Do NOT include any markdown formatting or preamble.";

    let prompt = format!(
        "Extract every atomic fact from the following text as a list of objects. \
        Each object must have a 'fact' field (string). \
        \n\nText:\n{}\n\nOutput JSON: {{\"facts\": [ {{ \"fact\": \"...\" }} ]}}",
        chunk.content
    );

    let raw_res = client.prompt(model, preamble, &prompt, 4096, 0.3).await?;

    // Clean markdown
    let clean_json = raw_res
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: FactList = serde_json::from_str(clean_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse ACU JSON: {}. Raw: {}", e, raw_res))?;

    Ok(parsed.facts)
}
