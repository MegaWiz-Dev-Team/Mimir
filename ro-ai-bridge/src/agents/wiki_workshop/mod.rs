pub mod generator;
pub mod extractor;
pub mod verifier;

use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WikiChunk {
    pub source_file: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QAPair {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AtomicFact {
    pub fact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoverageReport {
    pub coverage_score: f32,
    pub missing_facts: Vec<String>,
    pub reasoning: String,
}
