use rig::providers::gemini;
use super::{WikiChunk, QAPair, AtomicFact, CoverageReport};
use anyhow::Result;
use rig::completion::Prompt;
use tracing::info;

pub struct CoverageVerifierAgent;

pub async fn verify_coverage(
    client: &gemini::Client, 
    gemini_model: &str,
    _chunk: &WikiChunk,
    facts: &[AtomicFact],
    qa_pairs: &[QAPair]
) -> Result<CoverageReport> {
    info!("      Verifying Coverage (Agent approach)...");

    let agent = client.agent(gemini_model)
        .preamble("You are a strict QA Verifier. Analyze if the Q/A pairs cover the important Atomic Facts. \
                   Return ONLY a valid JSON object matching the schema. \
                   Do NOT include any markdown formatting or preamble.")
        .build();

    let facts_str = serde_json::to_string(facts)?;
    let qa_str = serde_json::to_string(qa_pairs)?;

    let prompt = format!(
        "Analyze the coverage of these Atomic Facts by the Q/A Pairs.
        
        Atomic Facts (Target):
        {}
        
        Generated Q/A Pairs (Candidate):
        {}
        
        Task:
        1. Check if each Atomic Fact is answered or inferred by at least one Q/A pair.
        2. Calculate `coverage_score` as (Covered Facts / Total Facts).
        3. List any `missing_facts` that are NOT covered (maximum 30 items to prevent output truncation).
        4. Provide a brief `reasoning`.
        
        Output valid JSON adhering to the `CoverageReport` schema.",
        facts_str, qa_str
    );

    let raw_res = agent.prompt(prompt.as_str()).await?;

    // Clean markdown
    let clean_json = raw_res.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let report: CoverageReport = serde_json::from_str(clean_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse Report JSON: {}. Raw: {}", e, raw_res))?;

    Ok(report)
}
