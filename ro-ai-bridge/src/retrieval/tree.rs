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
    /// LLM-assessed confidence score (0.0 - 1.0) for this result.
    pub confidence: f32,
}

// ── Trait ──────────────────────────────────────────────

/// Trait for tree-based (PageIndex) retrieval engines.
#[async_trait]
pub trait TreeRetriever: Send + Sync {
    /// Search across multiple documents in parallel.
    async fn search_parallel(
        &self,
        router: &mimir_core_ai::services::llm_router::LlmRouter,
        docs: &[(String, String, String)], // (title, content, tree_json)
        question: &str,
    ) -> Vec<TreeSearchResult>;
}

// ── NativeTreeRetriever ────────────────────────────────

use mimir_core_ai::services::llm_router::UniversalClient;

/// Production retriever that calls Native LLM in parallel.
pub struct NativeTreeRetriever {
    /// Max concurrent requests to prevent overload.
    pub concurrency_limit: usize,
}

impl Default for NativeTreeRetriever {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeTreeRetriever {
    pub fn new() -> Self {
        Self {
            concurrency_limit: 10,
        }
    }

    pub fn with_concurrency(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }

    /// Call LLM natively for a single document's tree.
    async fn search_one(
        &self,
        client: &UniversalClient,
        model: &str,
        title: &str,
        tree_json: &str,
        content: &str,
        question: &str,
    ) -> Result<TreeSearchResult, String> {
        let tree_index: Value =
            serde_json::from_str(tree_json).map_err(|e| format!("Invalid tree JSON: {}", e))?;

        let system_prompt = "You are a document search agent. Given a tree index of a document, find the most relevant sections for the user's question. Return exactly a valid JSON with fields: 'answer' (string or null), 'relevant_sections' (list of strings representing node titles), 'confidence' (float 0.0-1.0 representing how confident you are this document answers the question), 'reasoning' (string).";
        
        let mut truncated_tree = tree_json;
        if tree_json.len() > 8000 {
            let mut end = 8000;
            while end > 0 && !tree_json.is_char_boundary(end) {
                end -= 1;
            }
            truncated_tree = &tree_json[..end];
        }
        let mut truncated_content = content;
        if content.len() > 12000 {
            let mut end = 12000;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            truncated_content = &content[..end];
        }

        let user_prompt = format!(
            "## Document Title: {}\n\n## Tree Index:\n```json\n{}\n```\n\n## Partial Document Content:\n{}\n\n## Question:\n{}\n\nPlease find the answer using the tree index to locate relevant sections.",
            title, truncated_tree, truncated_content, question
        );

        let response_text = client.prompt(model, system_prompt, &user_prompt, 2048, 0.1)
            .await
            .map_err(|e| format!("LLM generation failed: {}", e))?;

        // Cleanup potential markdown codeblock wrapping
        let clean_json = response_text.trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let result: Value = serde_json::from_str(clean_json).unwrap_or(json!({
            "answer": null,
            "relevant_sections": [],
            "reasoning": "Failed to parse JSON"
        }));

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

        let confidence = result
            .get("confidence")
            .and_then(|c| c.as_f64())
            .map(|c| (c as f32).clamp(0.1, 1.0))
            .unwrap_or_else(|| {
                // Heuristic fallback: confidence based on sections found
                if sections.is_empty() { 0.2 } else { 0.5 + (sections.len() as f32 * 0.1).min(0.4) }
            });

        let parent_context = extract_parent_context(&tree_index, &sections);

        Ok(TreeSearchResult {
            document_title: title.to_string(),
            answer,
            relevant_sections: sections,
            parent_context,
            confidence,
        })
    }
}

#[async_trait]
impl TreeRetriever for NativeTreeRetriever {
    async fn search_parallel(
        &self,
        router: &mimir_core_ai::services::llm_router::LlmRouter,
        docs: &[(String, String, String)],
        question: &str,
    ) -> Vec<TreeSearchResult> {

        let (client, model) = match router.resolve_client("generation") {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Failed to resolve generation client: {}", e);
                return vec![];
            }
        };

        let concurrency = self.concurrency_limit;
        tracing::info!(
            docs = docs.len(),
            concurrency = concurrency,
            "🌲 Tree search: processing {} docs with concurrency {}",
            docs.len(),
            concurrency
        );

        // Execute with bounded concurrency via buffer_unordered.
        // Clone owned copies to avoid lifetime generalization errors with async closures.
        let mut results = Vec::new();
        let chunks: Vec<_> = docs.iter().enumerate().collect();
        for chunk in chunks.chunks(concurrency) {
            let mut handles = Vec::new();
            for &(i, (title, content, tree_json)) in chunk {
                let client_c = client.clone();
                let model_c = model.clone();
                let title_c = title.clone();
                let content_c = content.clone();
                let tree_json_c = tree_json.clone();
                let question_c = question.to_string();
                handles.push(tokio::spawn(async move {
                    let retriever = NativeTreeRetriever::new();
                    match retriever.search_one(&client_c, &model_c, &title_c, &tree_json_c, &content_c, &question_c).await {
                        Ok(result) => Some(result),
                        Err(e) => {
                            tracing::warn!("Native tree search failed for doc {} ({}): {}", i, title_c, e);
                            None
                        }
                    }
                }));
            }
            for handle in handles {
                if let Ok(Some(result)) = handle.await {
                    results.push(result);
                }
            }
        }

        results
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
                score: tr.confidence, // Dynamic LLM confidence score
                source_type: "tree".to_string(),
                metadata: json!({
                    "parent_context": tr.parent_context,
                    "section_count": tr.relevant_sections.len(),
                }),
            })
        })
        .collect()
}

// ── Native Tree Builder ─────────────────────────────

pub fn build_native_tree(content: &str, title: &str) -> Value {
    let mut root = json!({
        "title": title,
        "children": [],
        "level": 0
    });
    
    let mut stack: Vec<(usize, Value)> = vec![(0, root)];

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let heading = trimmed.trim_start_matches('#').trim().to_string();
            
            if !heading.is_empty() {
                let node = json!({
                    "title": heading,
                    "content": "",
                    "children": [],
                    "level": level
                });
                
                while stack.len() > 1 && stack.last().unwrap().0 >= level {
                    let (_, popped_node) = stack.pop().unwrap();
                    let parent = stack.last_mut().unwrap();
                    parent.1.get_mut("children").unwrap().as_array_mut().unwrap().push(popped_node);
                }
                
                stack.push((level, node));
            }
        }
    }
    
    while stack.len() > 1 {
        let (_, popped_node) = stack.pop().unwrap();
        let parent = stack.last_mut().unwrap();
        parent.1.get_mut("children").unwrap().as_array_mut().unwrap().push(popped_node);
    }
    
    stack.pop().unwrap().1
}

pub fn count_nodes(tree: &Value) -> i64 {
    let mut count = 0i64;
    if let Some(children) = tree.get("children").and_then(|n| n.as_array()) {
        count += children.len() as i64;
        for node in children {
            count += count_nodes(node);
        }
    }
    count
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
            confidence: 0.8,
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
            confidence: 0.5,
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
            confidence: 0.8,
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
            confidence: 0.6,
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
            confidence: 0.2,
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
            confidence: 0.9,
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
