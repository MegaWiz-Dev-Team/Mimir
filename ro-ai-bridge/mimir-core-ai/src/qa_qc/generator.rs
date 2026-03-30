use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, QAPair};
use anyhow::Result;
use tracing::info;
use crate::services::llm_router::UniversalClient;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct QAPairList {
    pub pairs: Vec<QAPair>,
}

pub struct QAGeneratorAgent;

// Improving the implementation to be more practical using rig's traits directly or just functions if we build agents on the fly.
// Let's define a builder or a runner.

pub async fn generate_qa(
    client: &UniversalClient, 
    model: &str,
    chunk: &WikiChunk,
    count: usize
) -> Result<Vec<QAPair>> {
    info!("      Generating {} Q/A pairs for chunk via {} (Agent approach)...", count, model);
    
    let preamble = "You are a helpful assistant that generates high-quality Q/A pairs from the provided text. \
                   Return ONLY a JSON object with a 'pairs' key containing an array of {question, answer} objects. \
                   Do NOT include any markdown formatting or preamble.";
                   
    let prompt_text = format!(
        "Generate {} distinct Question and Answer pairs based ONLY on the following text. \
        Focus on facts, mechanics, and key details.\n\n\
        Text:\n{}\n\n\
        Output JSON: {{\"pairs\": [{{ \"question\": \"...\", \"answer\": \"...\" }}]}}", 
        count,
        chunk.content
    );

    let raw_res = client.prompt(model, preamble, &prompt_text, 4096, 0.7).await?;
    
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

pub async fn generate_missing_qa(
    client: &UniversalClient, 
    model: &str,
    chunk: &WikiChunk,
    missing_facts: &[String],
    count: usize
) -> Result<Vec<QAPair>> {
    info!("      Generating {} missing Q/A pairs for chunk via {}...", count, model);
    
    let preamble = "You are an expert knowledge extractor that strictly follows instructions. \
                   Return ONLY a JSON object with a 'pairs' key containing an array of {\"question\": \"...\", \"answer\": \"...\"} objects. \
                   Do NOT include any markdown formatting or preamble.";
    
    let missing_str = missing_facts.join("\n- ");
                   
    let prompt_text = format!(
        "The following facts are missing from our current Q/A database: \n- {}\n\n\
        Generate up to {} Question and Answer pairs based ONLY on the following source text that specifically address these missing facts. \
        Do not generate general questions, focus ONLY on closing these knowledge gaps.\n\n\
        Source Text:\n{}\n\n\
        Output JSON: {{\"pairs\": [{{ \"question\": \"...\", \"answer\": \"...\" }}]}}", 
        missing_str,
        count,
        chunk.content
    );

    let raw_res = client.prompt(model, preamble, &prompt_text, 4096, 0.7).await?;
    
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
