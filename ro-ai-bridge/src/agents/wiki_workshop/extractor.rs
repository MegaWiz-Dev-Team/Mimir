use rig::agent::Agent;
use rig::completion::Prompt;
use rig::providers::openai; // Assuming we use OpenAI-compatible client for simplicity or specific provider if available
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, AtomicFact};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FactList {
    pub facts: Vec<AtomicFact>,
}

pub struct ACUExtractorAgent;

pub async fn extract_acus(
    client: &openai::Client, 
    model: &str,
    chunk: &WikiChunk
) -> Result<Vec<AtomicFact>> {
    let extractor = client.extractor::<FactList>(model)
        .preamble("You are a meticulous fact checker. Your task is to decompose the provided text into a list of Atomic Content Units (ACUs).")
        .build();

    let prompt = format!(
        "Extract all atomic, independent facts from the text below. \
        Each fact must be completely self-contained (replace pronouns with nouns, clarify context).\n\n\
        Text:\n{}\n\n\
        Output valid JSON.", 
        chunk.content
    );

    let response: FactList = extractor.extract(&prompt).await?;

    Ok(response.facts)
}
