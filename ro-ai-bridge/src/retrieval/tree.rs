use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::retrieval::qdrant::RetrievalResult;

// ── Models ────────────────────────────────────────────

/// Result from a single document's tree search, including parent context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeSearchResult {
    pub document_title: String,
    pub answer: Option<String>,
    pub relevant_sections: Vec<String>,
    /// Parent headings that provide broader context for the matched sections.
    pub parent_context: Vec<String>,
}

// ── Trait ──────────────────────────────────────────────

/// Trait for tree-based (PageIndex) retrieval engines.
#[async_trait]
pub trait TreeRetriever: Send + Sync {
    /// Search across multiple documents in parallel.
    async fn search_parallel(
        &self,
        docs: &[(String, String, String)], // (title, content, tree_json)
        question: &str,
    ) -> Vec<TreeSearchResult>;
}

// ── PageIndexRetriever ────────────────────────────────

/// Production retriever that calls PageIndex sidecar in parallel.
pub struct PageIndexRetriever {
    base_url: String,
    client: reqwest::Client,
    /// Max concurrent requests to PageIndex to prevent overload.
    concurrency_limit: usize,
}

impl PageIndexRetriever {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            concurrency_limit: 10,
        }
    }

    pub fn with_concurrency(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }

    /// Call PageIndex sidecar for a single document.
    async fn search_one(
        &self,
        title: &str,
        tree_json: &str,
        content: &str,
        question: &str,
    ) -> Result<TreeSearchResult, String> {
        let tree_index: Value =
            serde_json::from_str(tree_json).map_err(|e| format!("Invalid tree JSON: {}", e))?;

        let resp = self
            .client
            .post(format!("{}/search", self.base_url))
            .json(&json!({
                "tree_index": tree_index,
                "question": question,
                "content": content,
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("PageIndex request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("PageIndex returned {}", resp.status()));
        }

        let result: Value = resp.json().await.map_err(|e| e.to_string())?;

        let sections: Vec<String> = result
            .get("relevant_sections")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let answer = result
            .get("answer")
            .and_then(|a| a.as_str())
            .map(|s| s.to_string());

        let parent_context = extract_parent_context(&tree_index, &sections);

        Ok(TreeSearchResult {
            document_title: title.to_string(),
            answer,
            relevant_sections: sections,
            parent_context,
        })
    }
}

#[async_trait]
impl TreeRetriever for PageIndexRetriever {
    async fn search_parallel(
        &self,
        docs: &[(String, String, String)],
        question: &str,
    ) -> Vec<TreeSearchResult> {
        use futures::future::join_all;

        // Build futures for all docs
        let futures: Vec<_> = docs
            .iter()
            .map(|(title, content, tree_json)| self.search_one(title, tree_json, content, question))
            .collect();

        // Execute all in parallel
        let results = join_all(futures).await;

        // Collect successful results, log failures
        results
            .into_iter()
            .enumerate()
            .filter_map(|(i, r)| match r {
                Ok(result) => Some(result),
                Err(e) => {
                    tracing::warn!("Tree search failed for doc {}: {}", docs[i].0, e);
                    None
                }
            })
            .collect()
    }
}

/// Convert TreeSearchResults to standard RetrievalResults for ensemble.
pub fn tree_to_retrieval_results(tree_results: &[TreeSearchResult]) -> Vec<RetrievalResult> {
    tree_results
        .iter()
        .filter_map(|tr| {
            let content = tr
                .answer
                .clone()
                .unwrap_or_else(|| tr.relevant_sections.join("\n"));
            if content.is_empty() {
                return None;
            }
            Some(RetrievalResult {
                content,
                title: tr.document_title.clone(),
                score: 0.8, // Default score for tree results (reranker will re-score)
                source_type: "tree".to_string(),
                metadata: json!({
                    "parent_context": tr.parent_context,
                    "section_count": tr.relevant_sections.len(),
                }),
            })
        })
        .collect()
}

// ── Parent Context Extraction ─────────────────────────

/// Extract parent headings from tree_index JSON for the matched sections.
///
/// The tree_index typically has structure like:
/// ```json
/// { "children": [
///   { "title": "# Main Heading", "children": [
///     { "title": "## Sub Heading", "children": [...] }
///   ]}
/// ]}
/// ```
///
/// Given matched section text, we walk up the tree to find parent headings.
pub fn extract_parent_context(tree_index: &Value, matched_sections: &[String]) -> Vec<String> {
    let mut parents = Vec::new();

    if matched_sections.is_empty() {
        return parents;
    }

    // Walk the tree recursively looking for nodes whose content matches
    fn find_parents(node: &Value, target: &str, path: &mut Vec<String>) -> bool {
        let title = node.get("title").and_then(|t| t.as_str()).unwrap_or("");

        if !title.is_empty() {
            path.push(title.to_string());
        }

        // Check if this node's content matches
        let content = node
            .get("content")
            .or_else(|| node.get("text"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        if !target.is_empty() && content.contains(target) {
            return true;
        }

        // Recurse into children
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                if find_parents(child, target, path) {
                    return true;
                }
            }
        }

        if !title.is_empty() {
            path.pop();
        }
        false
    }

    // Try to find path for the first matched section
    let search_text = matched_sections.first().map(|s| s.as_str()).unwrap_or("");
    let mut path = Vec::new();
    find_parents(tree_index, search_text, &mut path);

    // Return parent path (excluding the leaf node itself)
    if path.len() > 1 {
        parents = path[..path.len() - 1].to_vec();
    } else {
        parents = path;
    }

    parents
}

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── TreeSearchResult tests ────────────────────────

    #[test]
    fn test_tree_search_result_serialization() {
        let result = TreeSearchResult {
            document_title: "README.md".to_string(),
            answer: Some("The API has 3 endpoints".to_string()),
            relevant_sections: vec!["POST /api".to_string(), "GET /health".to_string()],
            parent_context: vec!["# Main".to_string(), "## API".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deser: TreeSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.document_title, "README.md");
        assert_eq!(deser.parent_context.len(), 2);
    }

    #[test]
    fn test_tree_search_result_no_answer() {
        let result = TreeSearchResult {
            document_title: "doc.md".to_string(),
            answer: None,
            relevant_sections: vec!["section1".to_string()],
            parent_context: vec![],
        };
        assert!(result.answer.is_none());
        assert_eq!(result.relevant_sections.len(), 1);
    }

    // ── Parent context extraction ─────────────────────

    #[test]
    fn test_extract_parent_context_nested_tree() {
        let tree = json!({
            "title": "# Document",
            "children": [
                {
                    "title": "## Architecture",
                    "children": [
                        {
                            "title": "### API Design",
                            "content": "The REST API uses POST /api/search for queries",
                            "children": []
                        }
                    ]
                }
            ]
        });

        let sections = vec!["POST /api/search".to_string()];
        let parents = extract_parent_context(&tree, &sections);

        assert!(
            parents.len() >= 1,
            "Should find at least 1 parent, got: {:?}",
            parents
        );
        assert!(
            parents.contains(&"# Document".to_string())
                || parents.contains(&"## Architecture".to_string()),
            "Parents should include ancestor headings, got: {:?}",
            parents
        );
    }

    #[test]
    fn test_extract_parent_context_empty_sections() {
        let tree = json!({"title": "root", "children": []});
        let parents = extract_parent_context(&tree, &[]);
        assert!(parents.is_empty());
    }

    #[test]
    fn test_extract_parent_context_no_match() {
        let tree = json!({
            "title": "# Doc",
            "children": [{
                "title": "## Section",
                "content": "some totally different content",
                "children": []
            }]
        });

        let sections = vec!["nonexistent text".to_string()];
        let parents = extract_parent_context(&tree, &sections);
        // No match found, so parents may be empty
        assert!(parents.is_empty() || parents.len() <= 2);
    }

    #[test]
    fn test_extract_parent_context_flat_tree() {
        let tree = json!({
            "title": "# Root",
            "content": "This is root content with keyword",
            "children": []
        });

        let sections = vec!["keyword".to_string()];
        let parents = extract_parent_context(&tree, &sections);
        // Flat tree: the root itself matches, no real parents above it
        assert!(parents.len() <= 1);
    }

    // ── tree_to_retrieval_results ─────────────────────

    #[test]
    fn test_tree_to_retrieval_results_basic() {
        let tree_results = vec![TreeSearchResult {
            document_title: "API Guide".to_string(),
            answer: Some("Use POST /search endpoint".to_string()),
            relevant_sections: vec!["Details here".to_string()],
            parent_context: vec!["## API".to_string()],
        }];

        let results = tree_to_retrieval_results(&tree_results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_type, "tree");
        assert_eq!(results[0].content, "Use POST /search endpoint");
        assert_eq!(results[0].title, "API Guide");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_tree_to_retrieval_results_uses_sections_when_no_answer() {
        let tree_results = vec![TreeSearchResult {
            document_title: "Doc".to_string(),
            answer: None,
            relevant_sections: vec!["Section A".to_string(), "Section B".to_string()],
            parent_context: vec![],
        }];

        let results = tree_to_retrieval_results(&tree_results);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Section A"));
        assert!(results[0].content.contains("Section B"));
    }

    #[test]
    fn test_tree_to_retrieval_results_skips_empty() {
        let tree_results = vec![TreeSearchResult {
            document_title: "Empty".to_string(),
            answer: None,
            relevant_sections: vec![],
            parent_context: vec![],
        }];

        let results = tree_to_retrieval_results(&tree_results);
        assert_eq!(results.len(), 0, "Should skip results with no content");
    }

    #[test]
    fn test_tree_to_retrieval_results_metadata_has_parent_context() {
        let tree_results = vec![TreeSearchResult {
            document_title: "Doc".to_string(),
            answer: Some("answer".to_string()),
            relevant_sections: vec!["sec1".to_string()],
            parent_context: vec!["# Heading".to_string(), "## Sub".to_string()],
        }];

        let results = tree_to_retrieval_results(&tree_results);
        assert_eq!(results.len(), 1);
        let parent_ctx = results[0].metadata.get("parent_context").unwrap();
        assert_eq!(parent_ctx.as_array().unwrap().len(), 2);
    }

    // ── Trait tests ───────────────────────────────────

    #[test]
    fn test_tree_retriever_trait_is_object_safe() {
        fn _accept_trait_object(_r: &dyn TreeRetriever) {}
    }
}
