pub mod generator;
pub mod extractor;
pub mod verifier;
pub mod indexer;
pub mod pipeline;
pub mod clustering;

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
    #[serde(deserialize_with = "deserialize_missing_facts")]
    pub missing_facts: Vec<AtomicFact>,
    pub reasoning: String,
}

fn deserialize_missing_facts<'de, D>(deserializer: D) -> Result<Vec<AtomicFact>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
    
    match v {
        serde_json::Value::Array(arr) => {
            let mut facts = Vec::new();
            for item in arr {
                if let Some(s) = item.as_str() {
                    facts.push(AtomicFact { fact: s.to_string() });
                } else if let Ok(f) = serde_json::from_value::<AtomicFact>(item.clone()) {
                    facts.push(f);
                } else if let Some(obj) = item.as_object() {
                     // Fallback for "fact" field if strict parsing fails
                     if let Some(f) = obj.get("fact").and_then(|val| val.as_str()) {
                         facts.push(AtomicFact { fact: f.to_string() });
                     }
                }
            }
            Ok(facts)
        },
        _ => Ok(vec![]),
    }
}
