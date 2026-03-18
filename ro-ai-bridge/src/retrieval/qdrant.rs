use async_trait::async_trait;
use mimir_core_ai::services::qdrant::QdrantService;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Models ────────────────────────────────────────────

/// A single retrieval result from any vector search engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalResult {
    /// Text content of the retrieved chunk/document.
    pub content: String,
    /// Title or identifier of the source document.
    pub title: String,
    /// Similarity score (0.0–1.0).
    pub score: f32,
    /// Which retrieval source produced this ("vector", "graph", "tree").
    pub source_type: String,
    /// Additional metadata from Qdrant payload.
    pub metadata: Value,
}

// ── Trait ──────────────────────────────────────────────

/// Trait for vector-similarity retrieval engines.
/// Designed to be object-safe for easy mocking in tests.
#[async_trait]
pub trait VectorRetriever: Send + Sync {
    /// Search for documents semantically similar to `query` within a tenant scope.
    async fn search(
        &self,
        query: &str,
        tenant_id: &str,
        limit: usize,
    ) -> Result<Vec<RetrievalResult>, String>;
}

// ── QdrantRetriever ───────────────────────────────────

/// Production retriever that embeds queries and searches Qdrant.
pub struct QdrantRetriever {
    qdrant: QdrantService,
    embedding_model: String,
    collection: String,
}

impl QdrantRetriever {
    /// Create a new retriever targeting a specific Qdrant collection.
    pub fn new(qdrant: QdrantService, embedding_model: String, collection: String) -> Self {
        Self {
            qdrant,
            embedding_model,
            collection,
        }
    }

    /// Parse Qdrant search response JSON into `Vec<RetrievalResult>`.
    ///
    /// Qdrant response format:
    /// ```json
    /// { "result": [ { "id": 1, "score": 0.95, "payload": { "question": "...", "answer": "...", "tenant_id": "..." } } ] }
    /// ```
    pub fn parse_qdrant_response(response: &Value) -> Vec<RetrievalResult> {
        let results = response
            .get("result")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        results
            .iter()
            .map(|hit| {
                let payload = hit.get("payload").cloned().unwrap_or(Value::Null);
                let score = hit
                    .get("score")
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) as f32;

                // Try to extract content from various payload fields
                let content = payload
                    .get("answer")
                    .or_else(|| payload.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let title = payload
                    .get("question")
                    .or_else(|| payload.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                RetrievalResult {
                    content,
                    title,
                    score,
                    source_type: "vector".to_string(),
                    metadata: payload,
                }
            })
            .collect()
    }
}

#[async_trait]
impl VectorRetriever for QdrantRetriever {
    async fn search(
        &self,
        query: &str,
        tenant_id: &str,
        limit: usize,
    ) -> Result<Vec<RetrievalResult>, String> {
        // Step 1: Embed query via Heimdall/Ollama
        let vectors =
            crate::routes::vector::embed_texts(&[query.to_string()], &self.embedding_model)
                .await?;

        let vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| "No embedding vector returned".to_string())?;

        if vector.is_empty() {
            return Err("Empty embedding vector".to_string());
        }

        // Step 2: Search Qdrant with tenant filter
        let response = self
            .qdrant
            .search(&self.collection, vector, limit, tenant_id, false)
            .await
            .map_err(|e| format!("Qdrant search failed: {}", e))?;

        // Step 3: Parse response into RetrievalResults
        Ok(Self::parse_qdrant_response(&response))
    }
}

// ── Tests (TDD RED Phase) ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── RetrievalResult struct tests ──────────────────

    #[test]
    fn test_retrieval_result_serialization() {
        let result = RetrievalResult {
            content: "Aspirin is used for pain relief".to_string(),
            title: "Drug Info: Aspirin".to_string(),
            score: 0.95,
            source_type: "vector".to_string(),
            metadata: json!({"source_id": 42}),
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: RetrievalResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(result, deserialized);
        assert_eq!(deserialized.score, 0.95);
        assert_eq!(deserialized.source_type, "vector");
    }

    #[test]
    fn test_retrieval_result_default_metadata() {
        let result = RetrievalResult {
            content: "test".to_string(),
            title: "test".to_string(),
            score: 0.0,
            source_type: "vector".to_string(),
            metadata: Value::Null,
        };

        assert_eq!(result.metadata, Value::Null);
    }

    // ── parse_qdrant_response tests ───────────────────

    #[test]
    fn test_parse_qdrant_response_with_qa_payload() {
        // Mock Qdrant response with wiki_qa collection format
        let qdrant_response = json!({
            "result": [
                {
                    "id": 12345,
                    "version": 1,
                    "score": 0.92,
                    "payload": {
                        "question": "What is Aspirin?",
                        "answer": "Aspirin is a nonsteroidal anti-inflammatory drug (NSAID).",
                        "tenant_id": "medical_tenant",
                        "source_id": 1,
                        "is_active": true
                    }
                },
                {
                    "id": 12346,
                    "version": 1,
                    "score": 0.87,
                    "payload": {
                        "question": "Aspirin dosage guidelines",
                        "answer": "Standard dosage is 325-650mg every 4-6 hours.",
                        "tenant_id": "medical_tenant",
                        "source_id": 2,
                        "is_active": true
                    }
                }
            ],
            "status": "ok",
            "time": 0.001
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);

        assert_eq!(results.len(), 2);

        // First result
        assert_eq!(results[0].title, "What is Aspirin?");
        assert!(results[0].content.contains("NSAID"));
        assert_eq!(results[0].score, 0.92);
        assert_eq!(results[0].source_type, "vector");
        assert_eq!(results[0].metadata["tenant_id"], "medical_tenant");

        // Second result
        assert_eq!(results[1].title, "Aspirin dosage guidelines");
        assert!(results[1].content.contains("325-650mg"));
        assert_eq!(results[1].score, 0.87);
    }

    #[test]
    fn test_parse_qdrant_response_with_chunk_payload() {
        // Mock Qdrant response with source_chunks collection format
        let qdrant_response = json!({
            "result": [
                {
                    "id": 99,
                    "score": 0.88,
                    "payload": {
                        "content": "Patient records must be stored securely.",
                        "title": "Security Policy v2",
                        "chunk_id": 99,
                        "source_id": 5,
                        "tenant_id": "compliance_tenant",
                        "is_active": true
                    }
                }
            ],
            "status": "ok",
            "time": 0.002
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Patient records must be stored securely.");
        assert_eq!(results[0].title, "Security Policy v2");
        assert_eq!(results[0].score, 0.88);
    }

    #[test]
    fn test_parse_qdrant_response_empty() {
        let qdrant_response = json!({
            "result": [],
            "status": "ok",
            "time": 0.0
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_qdrant_response_missing_result_key() {
        let qdrant_response = json!({
            "status": "ok"
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_qdrant_response_missing_payload_fields() {
        // Gracefully handle missing optional fields
        let qdrant_response = json!({
            "result": [
                {
                    "id": 1,
                    "score": 0.5,
                    "payload": {}
                }
            ]
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "");
        assert_eq!(results[0].title, "Unknown");
        assert_eq!(results[0].score, 0.5);
    }

    #[test]
    fn test_parse_qdrant_response_no_score() {
        let qdrant_response = json!({
            "result": [
                {
                    "id": 1,
                    "payload": {
                        "answer": "some content",
                        "question": "some title"
                    }
                }
            ]
        });

        let results = QdrantRetriever::parse_qdrant_response(&qdrant_response);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 0.0);
    }

    // ── Trait tests ───────────────────────────────────

    #[test]
    fn test_vector_retriever_trait_is_object_safe() {
        // Compile-time proof that VectorRetriever can be used as a trait object.
        fn _accept_trait_object(_r: &dyn VectorRetriever) {}
    }

    #[test]
    fn test_retrieval_result_score_ordering() {
        let mut results = vec![
            RetrievalResult {
                content: "low".to_string(),
                title: "low".to_string(),
                score: 0.3,
                source_type: "vector".to_string(),
                metadata: Value::Null,
            },
            RetrievalResult {
                content: "high".to_string(),
                title: "high".to_string(),
                score: 0.95,
                source_type: "vector".to_string(),
                metadata: Value::Null,
            },
            RetrievalResult {
                content: "mid".to_string(),
                title: "mid".to_string(),
                score: 0.7,
                source_type: "vector".to_string(),
                metadata: Value::Null,
            },
        ];

        // Sort descending by score (as reranker would)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        assert_eq!(results[0].content, "high");
        assert_eq!(results[1].content, "mid");
        assert_eq!(results[2].content, "low");
    }
}
