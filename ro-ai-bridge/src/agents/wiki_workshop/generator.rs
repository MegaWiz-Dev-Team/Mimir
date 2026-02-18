use rig::providers::{ollama, gemini};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, QAPair};
use anyhow::Result;
use tracing::info;
use rig::completion::Prompt;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct QAPairList {
    pub pairs: Vec<QAPair>,
}

pub enum GeneratorClient {
    Ollama(ollama::Client),
    Gemini(gemini::Client),
}

pub struct QAGeneratorAgent;

// Improving the implementation to be more practical using rig's traits directly or just functions if we build agents on the fly.
// Let's define a builder or a runner.

pub async fn generate_qa(
    client: &GeneratorClient, 
    model: &str,
    chunk: &WikiChunk
) -> Result<Vec<QAPair>> {
    info!("      Generating Q/A pairs for chunk via {} (Agent approach)...", model);
    
    let preamble = "You are a helpful assistant that generates high-quality Q/A pairs from the provided text. \
                   Return ONLY a JSON object with a 'pairs' key containing an array of {question, answer} objects. \
                   Do NOT include any markdown formatting or preamble.";
                   
    let prompt_text = format!(
        "Generate 3 distinct Question and Answer pairs based ONLY on the following text. \
        Focus on facts, mechanics, and key details.\n\n\
        Text:\n{}\n\n\
        Output JSON: {{\"pairs\": [{{ \"question\": \"...\", \"answer\": \"...\" }}]}}", 
        chunk.content
    );

    let raw_res = match client {
        GeneratorClient::Ollama(c) => {
            let agent = c.agent(model).preamble(preamble).build();
            agent.prompt(prompt_text.as_str()).await?
        },
        GeneratorClient::Gemini(c) => {
            let agent = c.agent(model).preamble(preamble).build();
            agent.prompt(prompt_text.as_str()).await?
        }
    };
    
    // Clean markdown if present
    let clean_json = raw_res.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: QAPairList = serde_json::from_str(clean_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}. Raw: {}", e, raw_res))?;
    
    Ok(parsed.pairs)
}
