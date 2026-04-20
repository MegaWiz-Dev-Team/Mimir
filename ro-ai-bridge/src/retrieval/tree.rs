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
    /// Language Model's reasoning step or failure explanation.
    pub reasoning: Option<String>,
}

// ── Trait ──────────────────────────────────────────────

#[async_trait]
pub trait TreeRetriever: Send + Sync {
    /// Search across multiple documents using local vector routing.
    async fn search_parallel(
        &self,
        embed_model: &str,
        docs: &[(String, String, String)], // (title, content, tree_json)
        question: &str,
    ) -> Vec<TreeSearchResult>;
}

// ── NativeTreeRetriever ────────────────────────────────


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

#[derive(Debug, Clone)]
pub struct ExtractedNode {
    pub title: String,
    pub text: String,
    pub doc_title: String,
    pub doc_idx: usize,
    pub parent_context: Vec<String>,
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

    /// Recursively flattens the JSON tree into a list of ExtractNodes representing sections and their summaries.
    fn flatten_tree(
        node: &Value,
        doc_title: &str,
        doc_idx: usize,
        current_path: &[String],
        nodes: &mut Vec<ExtractedNode>
    ) {
        let title = node.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let mut new_path = current_path.to_vec();
        if !title.is_empty() {
            new_path.push(title.clone());
        }

        let summary = node.get("summary").and_then(|t| t.as_str()).unwrap_or("");
        let content = node.get("content").and_then(|t| t.as_str()).unwrap_or("");

        let mut combined_text = String::new();
        if !title.is_empty() {
            combined_text.push_str(&format!("Section: {}\n", title));
        }
        if !summary.is_empty() {
            combined_text.push_str(&format!("Summary: {}\n", summary));
        } else if !content.is_empty() {
            // Fallback to content if no summary exists, capped at ~500 chars to avoid massive text blocks
            combined_text.push_str(&format!("Content preview: {}\n", content.chars().take(500).collect::<String>()));
        }

        if !combined_text.trim().is_empty() {
            nodes.push(ExtractedNode {
                title: title.clone(),
                text: combined_text,
                doc_title: doc_title.to_string(),
                doc_idx,
                parent_context: new_path.clone(),
            });
        }

        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                Self::flatten_tree(child, doc_title, doc_idx, &new_path, nodes);
            }
        }
    }
}

/// Compute cosine similarity between two f32 vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (va, vb) in a.iter().zip(b.iter()) {
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[async_trait]
impl TreeRetriever for NativeTreeRetriever {
    async fn search_parallel(
        &self,
        embed_model: &str,
        docs: &[(String, String, String)],
        question: &str,
    ) -> Vec<TreeSearchResult> {
        tracing::info!(
            docs = docs.len(),
            "🌲 Tree search: Pure vector routing across {} docs",
            docs.len(),
        );

        if docs.is_empty() {
            return vec![];
        }

        // 1. Flatten all trees into extractable nodes.
        let mut all_nodes = Vec::new();
        for (idx, (doc_title, _content, tree_json_str)) in docs.iter().enumerate() {
            if let Ok(tree_index) = serde_json::from_str::<Value>(tree_json_str) {
                Self::flatten_tree(&tree_index, doc_title, idx, &[], &mut all_nodes);
            }
        }

        if all_nodes.is_empty() {
            return vec![];
        }

        // Prepare texts to embed: Question is first, followed by all node texts.
        let mut texts_to_embed = vec![question.to_string()];
        for node in &all_nodes {
            texts_to_embed.push(node.text.clone());
        }

        // 2. Batch embed nodes (+ question) locally via Heimdall
        let vectors = match crate::routes::vector::embed_texts(&texts_to_embed, embed_model).await {
            Ok(vecs) if vecs.len() == texts_to_embed.len() => vecs,
            Ok(_) | Err(_) => {
                tracing::warn!("Failed to embed tree nodes for vector routing.");
                return vec![];
            }
        };

        let q_vec = &vectors[0];
        let node_vecs = &vectors[1..];

        // 3. Compute similarities and group by document index
        // We will keep track of the best nodes for each document.
        let mut best_nodes_per_doc: std::collections::HashMap<usize, Vec<(&ExtractedNode, f32)>> = std::collections::HashMap::new();

        for (i, node) in all_nodes.iter().enumerate() {
            let sim = cosine_similarity(q_vec, &node_vecs[i]);
            if sim > 0.40 {
                best_nodes_per_doc.entry(node.doc_idx).or_default().push((node, sim));
            }
        }

        // 4. Construct TreeSearchResults for each document that had matching nodes.
        let mut final_results = Vec::new();

        for (doc_idx, mut matched_nodes) in best_nodes_per_doc {
            // Sort by similarity descending
            matched_nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            // Take top 3 best matching sections from this tree
            matched_nodes.truncate(3);

            let best_score = matched_nodes.first().map(|n| n.1).unwrap_or(0.0);
            
            let relevant_sections: Vec<String> = matched_nodes.iter().map(|n| n.0.title.clone()).collect();
            // Aggregate parent context
            let mut parent_context = std::collections::HashSet::new();
            for n in &matched_nodes {
                for p in &n.0.parent_context {
                    parent_context.insert(p.clone());
                }
            }

            let doc_title = docs[doc_idx].0.clone();

            final_results.push(TreeSearchResult {
                document_title: doc_title,
                answer: Some(format!("Routed via semantic vector matching against tree nodes.")),
                relevant_sections,
                parent_context: parent_context.into_iter().collect(),
                confidence: best_score, // Extracted pure vector cosine similarity
                reasoning: Some("Identified relevant nodes exclusively using pure semantic routing.".to_string()),
            });
        }

        final_results
    }
}

/// Convert TreeSearchResults to standard RetrievalResults for ensemble.
pub fn tree_to_retrieval_results(tree_results: &[TreeSearchResult]) -> Vec<RetrievalResult> {
    tree_results
        .iter()
        .filter_map(|tr| {
            let mut content = tr
                .answer
                .clone()
                .unwrap_or_else(|| tr.relevant_sections.join("\n"));
            
            if content.is_empty() {
                if let Some(reasoning) = &tr.reasoning {
                    if !reasoning.is_empty() {
                        content = format!("[LLM Reasoning] Failed to find answer: {}", reasoning);
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
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
    let root = json!({
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
            reasoning: None,
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
            reasoning: None,
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
            reasoning: None,
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
            reasoning: None,
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
            reasoning: None,
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
            reasoning: None,
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
