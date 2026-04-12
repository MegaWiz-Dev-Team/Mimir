use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::retrieval::qdrant::RetrievalResult;

// ── Trait ──────────────────────────────────────────────

/// Trait for knowledge-graph-based retrieval.
#[async_trait]
pub trait GraphRetriever: Send + Sync {
    /// Search the knowledge graph for entities matching the question,
    /// then expand their neighborhood for context.
    async fn search(
        &self,
        question: &str,
        tenant_id: &str,
        limit: usize,
    ) -> Result<Vec<GraphSearchResult>, String>;
}

// ── Models ────────────────────────────────────────────

/// A result from the knowledge graph search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSearchResult {
    /// The matched entity name.
    pub entity_name: String,
    /// The entity type (e.g., Drug, Person, Concept).
    pub entity_type: String,
    /// Properties of the entity.
    pub properties: Option<Value>,
    /// Neighboring entities (1-hop) with their relation types.
    pub neighbors: Vec<GraphNeighbor>,
    /// Relevance score (keyword match ratio).
    pub score: f64,
}

/// A neighbor entity connected via a relation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNeighbor {
    pub name: String,
    pub entity_type: String,
    pub relation_type: String,
    pub direction: String, // "outgoing" or "incoming"
}

// ── Cypher-based GraphRetriever ───────────────────────

/// Production graph retriever that queries Neo4j via SQL bridge layer.
/// Uses the existing kg_entities / kg_relations tables for tenant-isolated
/// entity search + neighbor expansion.
pub struct SqlGraphRetriever {
    pool: mimir_core_ai::services::db::DbPool,
}

impl SqlGraphRetriever {
    pub fn new(pool: mimir_core_ai::services::db::DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GraphRetriever for SqlGraphRetriever {
    async fn search(
        &self,
        question: &str,
        tenant_id: &str,
        limit: usize,
    ) -> Result<Vec<GraphSearchResult>, String> {
        self.search_with_hops(question, tenant_id, limit, 2).await
    }
}

impl SqlGraphRetriever {
    pub async fn search_with_hops(
        &self,
        question: &str,
        tenant_id: &str,
        limit: usize,
        hop_limit: i32,
    ) -> Result<Vec<GraphSearchResult>, String> {
        // 1. Extract key terms from question for entity search
        let terms = extract_search_terms(question);
        tracing::info!("Graph search terms: {:?}", terms);
        if terms.is_empty() {
            return Ok(vec![]);
        }

        let mut results = Vec::new();

        for term in &terms {
            // Use FULLTEXT search for terms >= 3 chars, fallback to LIKE for shorter terms
            let entities: Vec<(i64, String, String, Option<Vec<u8>>)> = if term.len() >= 3 {
                // FULLTEXT MATCH/AGAINST with BOOLEAN MODE for prefix matching
                let ft_term = format!("{}*", term);
                sqlx::query_as(
                    "SELECT id, name, entity_type, properties \
                     FROM kg_entities \
                     WHERE tenant_id = ? AND (MATCH(name) AGAINST(? IN BOOLEAN MODE) OR LOWER(entity_type) LIKE LOWER(?)) \
                     LIMIT ?",
                )
                .bind(tenant_id)
                .bind(&ft_term)
                .bind(&format!("%{}%", term))
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .unwrap_or_else(|e| {
                    // Fallback to LIKE if FULLTEXT index doesn't exist yet
                    tracing::warn!("FULLTEXT search failed, will retry with LIKE: {}", e);
                    vec![]
                })
            } else {
                vec![]
            };

            // Fallback to LIKE if FULLTEXT returned nothing (index may not exist yet)
            let entities = if entities.is_empty() {
                let pattern = format!("%{}%", term);
                sqlx::query_as(
                    "SELECT id, name, entity_type, properties \
                     FROM kg_entities \
                     WHERE tenant_id = ? AND (LOWER(name) LIKE LOWER(?) OR LOWER(entity_type) LIKE LOWER(?)) \
                     LIMIT ?",
                )
                .bind(tenant_id)
                .bind(&pattern)
                .bind(&pattern)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| format!("Entity search failed: {}", e))?
            } else {
                entities
            };

            tracing::info!("Graph search entities found: {}", entities.len());

            for (entity_id, name, entity_type, props_raw) in &entities {
                // Convert BLOB bytes to string for JSON parsing
                let props = props_raw
                    .as_ref()
                    .and_then(|bytes| String::from_utf8(bytes.clone()).ok());
                
                // Get neighbors based on hop_limit
                let outgoing: Vec<(String, String, String, i32)> = if hop_limit >= 2 {
                    sqlx::query_as(
                        "SELECT e1.name, e1.entity_type, r1.relation_type, 1 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_entities e1 ON e1.id = r1.to_entity_id \
                         WHERE r1.from_entity_id = ? AND r1.tenant_id = ? \
                         UNION ALL \
                         SELECT e2.name, e2.entity_type, CONCAT(r1.relation_type, ' -> ', r2.relation_type) AS relation_type, 2 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_relations r2 ON r1.to_entity_id = r2.from_entity_id \
                         JOIN kg_entities e2 ON e2.id = r2.to_entity_id \
                         WHERE r1.from_entity_id = ? AND r1.tenant_id = ? AND r2.tenant_id = ? \
                         LIMIT 20",
                    )
                    .bind(entity_id).bind(tenant_id)
                    .bind(entity_id).bind(tenant_id).bind(tenant_id)
                    .fetch_all(&self.pool)
                    .await
                    .unwrap_or_default()
                } else {
                    sqlx::query_as(
                        "SELECT e1.name, e1.entity_type, r1.relation_type, 1 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_entities e1 ON e1.id = r1.to_entity_id \
                         WHERE r1.from_entity_id = ? AND r1.tenant_id = ? \
                         LIMIT 20",
                    )
                    .bind(entity_id).bind(tenant_id)
                    .fetch_all(&self.pool)
                    .await
                    .unwrap_or_default()
                };

                let incoming: Vec<(String, String, String, i32)> = if hop_limit >= 2 {
                    sqlx::query_as(
                        "SELECT e1.name, e1.entity_type, r1.relation_type, 1 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_entities e1 ON e1.id = r1.from_entity_id \
                         WHERE r1.to_entity_id = ? AND r1.tenant_id = ? \
                         UNION ALL \
                         SELECT e2.name, e2.entity_type, CONCAT(r2.relation_type, ' -> ', r1.relation_type) AS relation_type, 2 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_relations r2 ON r1.from_entity_id = r2.to_entity_id \
                         JOIN kg_entities e2 ON e2.id = r2.from_entity_id \
                         WHERE r1.to_entity_id = ? AND r1.tenant_id = ? AND r2.tenant_id = ? \
                         LIMIT 20",
                    )
                    .bind(entity_id).bind(tenant_id)
                    .bind(entity_id).bind(tenant_id).bind(tenant_id)
                    .fetch_all(&self.pool)
                    .await
                    .unwrap_or_default()
                } else {
                    sqlx::query_as(
                        "SELECT e1.name, e1.entity_type, r1.relation_type, 1 AS hop \
                         FROM kg_relations r1 \
                         JOIN kg_entities e1 ON e1.id = r1.from_entity_id \
                         WHERE r1.to_entity_id = ? AND r1.tenant_id = ? \
                         LIMIT 20",
                    )
                    .bind(entity_id).bind(tenant_id)
                    .fetch_all(&self.pool)
                    .await
                    .unwrap_or_default()
                };

                let mut neighbors = Vec::new();
                for (n, et, rt, hop) in outgoing {
                    let dir = if hop == 1 { "outgoing".to_string() } else { "outgoing_2hop".to_string() };
                    neighbors.push(GraphNeighbor {
                        name: n,
                        entity_type: et,
                        relation_type: rt,
                        direction: dir,
                    });
                }
                for (n, et, rt, hop) in incoming {
                    let dir = if hop == 1 { "incoming".to_string() } else { "incoming_2hop".to_string() };
                    neighbors.push(GraphNeighbor {
                        name: n,
                        entity_type: et,
                        relation_type: rt,
                        direction: dir,
                    });
                }

                let score = compute_match_score(name, term);
                let properties = props.as_ref().and_then(|p| serde_json::from_str(p).ok());

                results.push(GraphSearchResult {
                    entity_name: name.clone(),
                    entity_type: entity_type.clone(),
                    properties,
                    neighbors,
                    score,
                });
            }
        }

        // Sort by score descending, deduplicate by entity name
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.dedup_by(|a, b| a.entity_name == b.entity_name);
        results.truncate(limit);

        Ok(results)
    }
}

/// Convert graph search results to standard RetrievalResults for ensemble.
pub fn graph_to_retrieval_results(graph_results: &[GraphSearchResult]) -> Vec<RetrievalResult> {
    graph_results
        .iter()
        .map(|gr| {
            // Build a natural language description from graph data
            let mut content = format!("{} ({})", gr.entity_name, gr.entity_type);
            if !gr.neighbors.is_empty() {
                let relations: Vec<String> = gr
                    .neighbors
                    .iter()
                    .take(5)
                    .map(|n| {
                        if n.direction == "outgoing" {
                            format!("{} → {} ({})", gr.entity_name, n.name, n.relation_type)
                        } else {
                            format!("{} → {} ({})", n.name, gr.entity_name, n.relation_type)
                        }
                    })
                    .collect();
                content.push_str(&format!("\nRelations: {}", relations.join("; ")));
            }

            RetrievalResult {
                content,
                title: gr.entity_name.clone(),
                score: gr.score as f32,
                source_type: "graph".to_string(),
                metadata: json!({
                    "entity_type": gr.entity_type,
                    "neighbor_count": gr.neighbors.len(),
                    "properties": gr.properties,
                }),
            }
        })
        .collect()
}

// ── Helper functions ──────────────────────────────────

/// Extract meaningful search terms from a natural language question.
/// Strips common stop words and short words.
pub fn extract_search_terms(question: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "the",
        "a",
        "an",
        "is",
        "are",
        "was",
        "were",
        "what",
        "how",
        "who",
        "when",
        "where",
        "which",
        "does",
        "do",
        "did",
        "can",
        "will",
        "would",
        "should",
        "could",
        "has",
        "have",
        "had",
        "this",
        "that",
        "these",
        "those",
        "of",
        "in",
        "on",
        "at",
        "to",
        "for",
        "with",
        "by",
        "from",
        "about",
        "into",
        "and",
        "or",
        "not",
        "but",
        "if",
        "then",
        "than",
        "so",
        "it",
        "its",
        "my",
        "me",
        "we",
        "our",
        "you",
        "your",
        "they",
        "their",
        "him",
        "her",
        "his",
        "she",
        "he",
        "been",
        "being",
        // Thai stop words
        "คือ",
        "อะไร",
        "เป็น",
        "ไหม",
        "มี",
        "ของ",
        "ที่",
        "ใน",
        "จาก",
        "ให้",
        "ได้",
        "และ",
        "หรือ",
        "กับ",
        "นี้",
    ]
    .iter()
    .copied()
    .collect();

    question
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|w| w.len() >= 2 && !stop_words.contains(w.to_lowercase().as_str()))
        .map(|w| w.to_string())
        .collect()
}

/// Compute a simple match score between an entity name and a search term.
/// Returns 0.0-1.0.
pub fn compute_match_score(entity_name: &str, term: &str) -> f64 {
    let name_lower = entity_name.to_lowercase();
    let term_lower = term.to_lowercase();

    if name_lower == term_lower {
        1.0
    } else if name_lower.starts_with(&term_lower) || name_lower.ends_with(&term_lower) {
        0.9
    } else if name_lower.contains(&term_lower) {
        0.7
    } else {
        0.3
    }
}

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_search_terms ──────────────────────────

    #[test]
    fn test_extract_terms_basic() {
        let terms = extract_search_terms("What is Aspirin used for?");
        assert!(!terms.is_empty());
        assert!(
            terms.contains(&"Aspirin".to_string()),
            "Should keep 'Aspirin': {:?}",
            terms
        );
        assert!(
            !terms.contains(&"is".to_string()),
            "Should remove stop word 'is'"
        );
        assert!(
            !terms.contains(&"What".to_string()),
            "Should remove stop word 'What'"
        );
    }

    #[test]
    fn test_extract_terms_thai() {
        let terms = extract_search_terms("Aspirin คืออะไร");
        assert!(terms.contains(&"Aspirin".to_string()));
    }

    #[test]
    fn test_extract_terms_empty() {
        let terms = extract_search_terms("is the a");
        assert!(
            terms.is_empty(),
            "All stop words should produce empty: {:?}",
            terms
        );
    }

    #[test]
    fn test_extract_terms_preserves_long_words() {
        let terms = extract_search_terms("Tell me about Paracetamol interactions");
        assert!(terms.contains(&"Paracetamol".to_string()));
        assert!(terms.contains(&"interactions".to_string()));
    }

    // ── compute_match_score ───────────────────────────

    #[test]
    fn test_score_exact_match() {
        assert_eq!(compute_match_score("Aspirin", "Aspirin"), 1.0);
        assert_eq!(compute_match_score("Aspirin", "aspirin"), 1.0);
    }

    #[test]
    fn test_score_prefix_match() {
        let score = compute_match_score("Aspirin", "Asp");
        assert!(
            score > 0.8 && score <= 0.9,
            "Prefix should score ~0.9: {}",
            score
        );
    }

    #[test]
    fn test_score_contains_match() {
        let score = compute_match_score("Low-dose Aspirin", "Aspirin");
        assert!(score >= 0.7, "Contains should score >= 0.7: {}", score);
    }

    #[test]
    fn test_score_no_match() {
        let score = compute_match_score("Ibuprofen", "Aspirin");
        assert!(score < 0.5, "No match should score < 0.5: {}", score);
    }

    // ── GraphSearchResult ─────────────────────────────

    #[test]
    fn test_graph_search_result_serialization() {
        let result = GraphSearchResult {
            entity_name: "Aspirin".to_string(),
            entity_type: "Drug".to_string(),
            properties: Some(json!({"category": "NSAID"})),
            neighbors: vec![GraphNeighbor {
                name: "Headache".to_string(),
                entity_type: "Symptom".to_string(),
                relation_type: "treats".to_string(),
                direction: "outgoing".to_string(),
            }],
            score: 0.95,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["entity_name"], "Aspirin");
        assert_eq!(json["neighbors"][0]["name"], "Headache");
        assert_eq!(json["neighbors"][0]["relation_type"], "treats");
    }

    #[test]
    fn test_graph_search_result_empty_neighbors() {
        let result = GraphSearchResult {
            entity_name: "Orphan".to_string(),
            entity_type: "Concept".to_string(),
            properties: None,
            neighbors: vec![],
            score: 0.5,
        };
        assert!(result.neighbors.is_empty());
    }

    // ── graph_to_retrieval_results ────────────────────

    #[test]
    fn test_graph_to_retrieval_basic() {
        let graph_results = vec![GraphSearchResult {
            entity_name: "Aspirin".to_string(),
            entity_type: "Drug".to_string(),
            properties: None,
            neighbors: vec![GraphNeighbor {
                name: "Ibuprofen".to_string(),
                entity_type: "Drug".to_string(),
                relation_type: "interacts_with".to_string(),
                direction: "outgoing".to_string(),
            }],
            score: 0.95,
        }];

        let results = graph_to_retrieval_results(&graph_results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_type, "graph");
        assert_eq!(results[0].title, "Aspirin");
        assert!(results[0].content.contains("Aspirin"));
        assert!(results[0].content.contains("Ibuprofen"));
        assert_eq!(results[0].score, 0.95);
    }

    #[test]
    fn test_graph_to_retrieval_no_neighbors() {
        let graph_results = vec![GraphSearchResult {
            entity_name: "Orphan".to_string(),
            entity_type: "Concept".to_string(),
            properties: None,
            neighbors: vec![],
            score: 0.5,
        }];

        let results = graph_to_retrieval_results(&graph_results);
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].content.contains("Relations:"),
            "No neighbors = no relations line"
        );
    }

    #[test]
    fn test_graph_to_retrieval_metadata_has_entity_type() {
        let graph_results = vec![GraphSearchResult {
            entity_name: "Test".to_string(),
            entity_type: "Person".to_string(),
            properties: Some(json!({"role": "doctor"})),
            neighbors: vec![],
            score: 0.8,
        }];

        let results = graph_to_retrieval_results(&graph_results);
        assert_eq!(results[0].metadata["entity_type"], "Person");
        assert_eq!(results[0].metadata["neighbor_count"], 0);
    }

    // ── Trait tests ───────────────────────────────────

    #[test]
    fn test_graph_retriever_trait_is_object_safe() {
        fn _accept_trait_object(_r: &dyn GraphRetriever) {}
    }
}
