use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::llm_router::LlmRouter;
use mimir_core_ai::services::qdrant::QdrantService;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::retrieval::{
    graph::SqlGraphRetriever,
    qdrant::{QdrantRetriever, VectorRetriever},
    tree::{NativeTreeRetriever, TreeRetriever},
};

#[derive(Deserialize, Serialize)]
pub struct ExtractorArgs {
    pub query: String,
    pub tenant_id: String,
}

// -----------------------------------------------------------------------------
// 1. Vector Search Tool
// -----------------------------------------------------------------------------
pub struct VectorSearchTool {
    #[allow(dead_code)]
    db_pool: DbPool,
    qdrant: Arc<QdrantService>,
    #[allow(dead_code)]
    router: Arc<LlmRouter>,
    embedding_model: String,
    collection: String,
}

impl VectorSearchTool {
    pub fn new(
        db_pool: DbPool,
        qdrant: Arc<QdrantService>,
        router: Arc<LlmRouter>,
        embedding_model: String,
        collection: String,
    ) -> Self {
        Self {
            db_pool,
            qdrant,
            router,
            embedding_model,
            collection,
        }
    }
}

impl Tool for VectorSearchTool {
    const NAME: &'static str = "vector_search";
    type Error = std::io::Error;
    type Args = ExtractorArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search the dense and sparse embeddings vector database. Excellent for finding broad, semantic knowledge or matching general concepts. Use this as your primary search tool.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string" },
                    "query": { "type": "string" }
                },
                "required": ["tenant_id", "query"]
            })
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let _db_pool = self.db_pool.clone();
        let qdrant = self.qdrant.clone();
        let embedding_model = self.embedding_model.clone();
        let collection = self.collection.clone();
        let tenant_id = args.tenant_id.clone();
        let query = args.query.clone();

        let results = tokio::spawn(async move {
            let retriever = QdrantRetriever::new(
                (*qdrant).clone(),
                embedding_model,
                collection,
            );
            VectorRetriever::search(&retriever, &query, &tenant_id, 5).await
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .map_err(|e: String| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut output = String::new();
        for res in results {
            output.push_str(&format!("- [{}] {}\n", res.title, res.content));
        }
        
        if output.is_empty() {
            Ok("No semantic vector results found.".to_string())
        } else {
            Ok(output)
        }
    }
}

// -----------------------------------------------------------------------------
// 2. Graph Search Tool
// -----------------------------------------------------------------------------
pub struct GraphSearchTool {
    db_pool: DbPool,
}

impl GraphSearchTool {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
        }
    }
}

impl Tool for GraphSearchTool {
    const NAME: &'static str = "graph_search";
    type Error = std::io::Error;
    type Args = ExtractorArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search the knowledge graph database for interconnected entities and relationships. Use this when the query asks about how multiple things relate to each other, or complex entity maps.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string" },
                    "query": { "type": "string" }
                },
                "required": ["tenant_id", "query"]
            })
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_pool = self.db_pool.clone();
        let tenant_id = args.tenant_id.clone();
        let query = args.query.clone();

        let results = tokio::spawn(async move {
            let retriever = SqlGraphRetriever::new(db_pool);
            crate::retrieval::graph::GraphRetriever::search(&retriever, &query, &tenant_id, 3).await
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .map_err(|e: String| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let retrieval_results = crate::retrieval::graph::graph_to_retrieval_results(&results);

        let mut output = String::new();
        for res in retrieval_results {
            output.push_str(&format!("- [{}] {}\n", res.title, res.content));
        }

        if output.is_empty() {
            Ok("No graph relational results found.".to_string())
        } else {
            Ok(output)
        }
    }
}

// -----------------------------------------------------------------------------
// 3. Tree Search Tool
// -----------------------------------------------------------------------------
pub struct TreeSearchTool {
    db_pool: DbPool,
    router: Arc<LlmRouter>,
}

impl TreeSearchTool {
    pub fn new(db_pool: DbPool, router: Arc<LlmRouter>) -> Self {
        Self { db_pool, router }
    }
}

impl Tool for TreeSearchTool {
    const NAME: &'static str = "tree_search";
    type Error = std::io::Error;
    type Args = ExtractorArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search the hierarchical tree index. Use this tool when the query requires extracting information from highly structured documents with parent-child heading relationships.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string" },
                    "query": { "type": "string" }
                },
                "required": ["tenant_id", "query"]
            })
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_pool = self.db_pool.clone();
        let router = self.router.clone();
        let tenant_id = args.tenant_id.clone();
        let query = args.query.clone();

        let results = tokio::spawn(async move {
            let retriever = NativeTreeRetriever::new();

            // Load data sources with tree indexes
            let docs: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT id, name, CAST(raw_markdown AS CHAR), CAST(pageindex_tree AS CHAR) \
                 FROM data_sources WHERE tenant_id = ?",
            )
            .bind(&tenant_id)
            .fetch_all(&db_pool)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            let mut tree_docs = Vec::new();
            for (_, title, content_opt, tree_opt) in docs {
                if let (Some(content), Some(tree)) = (content_opt, tree_opt) {
                    tree_docs.push((title, content, tree));
                }
            }

            let (client, model) = match router.resolve_client("generation") {
                Ok(res) => res,
                Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
            };

            let res = retriever
                .search_parallel(&client, &model, &tree_docs, &query)
                .await;
            Ok::<Vec<crate::retrieval::tree::TreeSearchResult>, std::io::Error>(res)
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .map_err(|e: std::io::Error| e)?;

        let mut output = String::new();
        // Convert to text response
        for res in results {
            output.push_str(&format!(
                "- [{}]\nMatched section: {}\nRelevant context: {}\n---\n",
                res.document_title, res.relevant_sections.join(" > "), res.answer.unwrap_or_default()
            ));
        }

        if output.is_empty() {
            Ok("No structured tree results found.".to_string())
        } else {
            Ok(output)
        }
    }
}
