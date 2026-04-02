use super::{AtomicFact, CoverageReport, QAPair, WikiChunk};
use crate::services::llm_router::UniversalClient;
use anyhow::Result;
use tracing::info;

pub struct CoverageVerifierAgent;

pub async fn verify_coverage(
    client: &UniversalClient,
    model: &str,
    _chunk: &WikiChunk,
    facts: &[AtomicFact],
    qa_pairs: &[QAPair],
) -> Result<CoverageReport> {
    info!("      Verifying Coverage (Agent approach)...");

    let preamble = "You are a strict QA Verifier. Analyze if the Q/A pairs cover the important Atomic Facts. \
                   Return ONLY a valid JSON object matching the schema. \
                   Do NOT include any markdown formatting or preamble.";

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
        2. Calculate `coverage_score` as a decimal between 0.0 and 1.0 (e.g. 0.18 for 18%).
        3. List any `missing_facts` that are NOT covered (maximum 30 items to prevent output truncation).
        4. Provide a brief `reasoning`.
        
        Output valid JSON adhering to the `CoverageReport` schema.",
        facts_str, qa_str
    );

    let raw_res = client.prompt(model, preamble, &prompt, 4096, 0.2).await?;

    // Clean markdown
    let clean_json = raw_res
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let report: CoverageReport = serde_json::from_str(clean_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse Report JSON: {}. Raw: {}", e, raw_res))?;

    Ok(report)
}
