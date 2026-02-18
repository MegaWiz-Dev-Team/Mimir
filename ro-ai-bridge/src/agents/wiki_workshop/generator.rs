use rig::agent::Agent;
use rig::completion::Prompt;
use rig::providers::openai;
use rig::extractor::Extractor;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, QAPair};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QAPairList {
    pub pairs: Vec<QAPair>,
}

// Struct removed to avoid trait bound errors as we use the function directly.

// Improving the implementation to be more practical using rig's traits directly or just functions if we build agents on the fly.
// Let's define a builder or a runner.

pub async fn generate_qa(
    client: &openai::Client, 
    model: &str,
    chunk: &WikiChunk
) -> Result<Vec<QAPair>> {
    // Use extractor pattern for structured output
    let extractor = client.extractor::<QAPairList>(model)
        .preamble("You are an expert Game Wiki content analyzer. Your task is to generate high-quality Question & Answer pairs from the provided text.")
        .build();

    let prompt = format!(
        "Generate 3-5 distinct Question and Answer pairs based ONLY on the following text. \
        Focus on facts, mechanics, and key details.\n\n\
        Text:\n{}\n\n\
        Output valid JSON.", 
        chunk.content
    );

    let response: QAPairList = extractor.extract(&prompt).await?;

    Ok(response.pairs)
}
