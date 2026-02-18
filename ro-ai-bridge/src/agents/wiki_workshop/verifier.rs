use rig::agent::Agent;
use rig::completion::Prompt;
use rig::providers::openai;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use super::{WikiChunk, QAPair, AtomicFact, CoverageReport};
use anyhow::Result;

pub struct CoverageVerifierAgent;

pub async fn verify_coverage(
    client: &openai::Client, 
    model: &str,
    chunk: &WikiChunk,
    facts: &[AtomicFact],
    qa_pairs: &[QAPair]
) -> Result<CoverageReport> {
    // Construct the context
    let facts_str = serde_json::to_string_pretty(facts)?;
    let qa_str = serde_json::to_string_pretty(qa_pairs)?;

    let extractor = client.extractor::<CoverageReport>(model)
        .preamble("You are a strict QA auditor. Your task is to verify if the generated Q/A pairs cover all key facts from the source text.")
        .build();

    let prompt = format!(
        "Analyze the coverage of these Atomic Facts by the Q/A Pairs.\n\n\
        Source Chunk:\n{}\n\n\
        Atomic Facts (Target):\n{}\n\n\
        Generated Q/A Pairs (Candidate):\n{}\n\n\
        Task:\n\
        1. Check if each Atomic Fact is answered or inferred by at least one Q/A pair.\n\
        2. Calculate `coverage_score` as (Covered Facts / Total Facts).\n\
        3. List any `missing_facts` that are NOT covered.\n\
        4. Provide a brief `reasoning`.\n\n\
        Output valid JSON adhering to the `CoverageReport` schema.", 
        chunk.content, facts_str, qa_str
    );

    let report: CoverageReport = extractor.extract(&prompt).await?;

    Ok(report)
}
