use rig::completion::Prompt;
use rig::providers::gemini;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, AtomicFact};
use anyhow::Result;
use tracing::info;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct FactList {
    pub facts: Vec<AtomicFact>,
}

pub struct ACUExtractorAgent;

pub async fn extract_acus(
    client: &gemini::Client, 
    model: &str,
    chunk: &WikiChunk
) -> Result<Vec<AtomicFact>> {
    info!("      Extracting ACUs (Agent approach)...");

    let agent = client.agent(model)
        .preamble("You are an expert knowledge extractor. Extract atomic facts from the text. \
                   Return ONLY a valid JSON object with a 'facts' key containing a list of objects. \
                   Do NOT include any markdown formatting or preamble.")
        .build();

    let prompt = format!(
        "Extract every atomic fact from the following text as a list of objects. \
        Each object must have a 'fact' field (string). \
        \n\nText:\n{}\n\nOutput JSON: {{\"facts\": [ {{ \"fact\": \"...\" }} ]}}",
        chunk.content
    );

    let raw_res_future = async {
        agent.prompt(prompt.as_str()).await
    };
    
    let raw_res = tokio::time::timeout(std::time::Duration::from_secs(120), raw_res_future)
        .await
        .map_err(|_| anyhow::anyhow!("ACU Extraction timed out after 120s"))??;
    
    // Clean markdown
    let clean_json = raw_res.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: FactList = serde_json::from_str(clean_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse ACU JSON: {}. Raw: {}", e, raw_res))?;

    Ok(parsed.facts)
}
