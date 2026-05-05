use crate::services::db::DbPool;
use crate::services::llm_router::LlmRouter;
use crate::services::neo4j::Neo4jService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct GodNode {
    pub entity_id: i64,
    pub name: String,
    pub entity_type: String,
    pub degree_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct SurprisingConnection {
    pub from_name: String,
    pub to_name: String,
    pub relation_type: String,
    pub from_source_id: i64,
    pub to_source_id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphQuestion {
    pub question_type: String,
    pub question: String,
    pub why: String,
}

/// Identifies "God Nodes" (Entities with the highest connection count).
/// Routes to Neo4j when USE_NEO4J_GRAPH=true and neo4j service is available.
pub async fn get_god_nodes(pool: &DbPool, tenant_id: &str, limit: i64) -> Result<Vec<GodNode>, String> {
    if std::env::var("USE_NEO4J_GRAPH").as_deref() == Ok("true") {
        let config = crate::services::neo4j::Neo4jConfig::from_env();
        if let Some(neo4j) = Neo4jService::try_new(&config).await {
            let rows = neo4j.get_god_nodes(tenant_id, limit).await
                .map_err(|e| format!("Neo4j god nodes failed: {}", e))?;
            return Ok(rows.into_iter().map(|(name, entity_type, degree)| GodNode {
                entity_id: 0,
                name,
                entity_type,
                degree_count: degree,
            }).collect());
        }
    }

    let nodes: Vec<GodNode> = sqlx::query_as(
        r#"
        SELECT
            e.id as entity_id,
            e.name,
            e.entity_type,
            (SELECT COUNT(*) FROM kg_relations r WHERE r.from_entity_id = e.id OR r.to_entity_id = e.id) as degree_count
        FROM kg_entities e
        WHERE e.tenant_id = ?
        HAVING degree_count > 0
        ORDER BY degree_count DESC
        LIMIT ?
        "#
    )
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch god nodes: {}", e))?;

    Ok(nodes)
}

/// Identifies "Surprising Connections" (Relations crossing source document boundaries).
/// Routes to Neo4j when USE_NEO4J_GRAPH=true and neo4j service is available.
pub async fn get_surprising_connections(pool: &DbPool, tenant_id: &str, limit: i64) -> Result<Vec<SurprisingConnection>, String> {
    if std::env::var("USE_NEO4J_GRAPH").as_deref() == Ok("true") {
        let config = crate::services::neo4j::Neo4jConfig::from_env();
        if let Some(neo4j) = Neo4jService::try_new(&config).await {
            let rows = neo4j.get_surprising_connections(tenant_id, limit).await
                .map_err(|e| format!("Neo4j surprising connections failed: {}", e))?;
            return Ok(rows.into_iter().map(|(from_name, to_name, relation_type, from_source_id, to_source_id)| SurprisingConnection {
                from_name,
                to_name,
                relation_type,
                from_source_id,
                to_source_id,
            }).collect());
        }
    }

    let connections: Vec<SurprisingConnection> = sqlx::query_as(
        r#"
        SELECT
            e1.name as from_name,
            e2.name as to_name,
            r.relation_type,
            e1.source_id as from_source_id,
            e2.source_id as to_source_id
        FROM kg_relations r
        JOIN kg_entities e1 ON r.from_entity_id = e1.id
        JOIN kg_entities e2 ON r.to_entity_id = e2.id
        WHERE r.tenant_id = ?
            AND e1.source_id IS NOT NULL
            AND e2.source_id IS NOT NULL
            AND e1.source_id != e2.source_id
        LIMIT ?
        "#
    )
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch surprising connections: {}", e))?;

    Ok(connections)
}

/// Clean LLM JSON to strip `<think>` tags (from entity_extractor module)
fn clean_json_for_insights(raw: &str) -> String {
    let mut text = raw.to_string();
    while let Some(start) = text.find("<think>") {
        if let Some(end) = text.find("</think>") {
            text = format!("{}{}", &text[..start], &text[end + 8..]);
        } else {
            text = text[..start].to_string();
            break;
        }
    }
    let text = text.trim();
    let text = text.strip_prefix("```json").unwrap_or(text);
    let text = text.strip_prefix("```").unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text);
    let text = text.trim();

    if !text.starts_with('{') && !text.starts_with('[') {
        if let Some(pos) = text.find('{').or_else(|| text.find('[')) {
            return text[pos..].to_string();
        }
    }
    text.to_string()
}

/// Uses Graph Analytics to generate intelligent question suggestions
pub async fn generate_graph_insights(
    pool: &DbPool,
    tenant_id: &str,
    router: &LlmRouter,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) -> Result<Vec<GraphQuestion>, String> {
    let god_nodes = get_god_nodes(pool, tenant_id, 5).await.unwrap_or_default();
    let connections = get_surprising_connections(pool, tenant_id, 5).await.unwrap_or_default();

    if god_nodes.is_empty() && connections.is_empty() {
        return Ok(vec![GraphQuestion {
            question_type: "no_signal".into(),
            question: "Not enough graph data available yet.".into(),
            why: "Extract more documents to populate the Knowledge Graph.".into(),
        }]);
    }

    let god_nodes_json = serde_json::to_string_pretty(&god_nodes).unwrap_or_default();
    let connections_json = serde_json::to_string_pretty(&connections).unwrap_or_default();

    let system_prompt = "You are an expert Graph Intelligence Analyzer. Analyze the provided knowledge graph metrics—'God Nodes' (highly connected core concepts) and 'Surprising Connections' (structural bridges between different documents)—and generate 3-5 insightful questions that users can explore in the Knowledge Base.

Return EXACTLY a JSON array of objects with this schema:
[
  {
    \"question_type\": \"cross_boundary\" | \"god_node_verification\" | \"isolated_concept\",
    \"question\": \"The actual question to ask?\",
    \"why\": \"Explanation of why this question is structurally interesting based on betweenness or high connectivity\"
  }
]";

    let user_prompt = format!(
        "Graph Metrics for Tenant:\n\nGod Nodes:\n{}\n\nSurprising Cross-Document Connections:\n{}\n\nGenerate the JSON array of questions now.",
        god_nodes_json, connections_json
    );
    
    let (client, model) = match router.resolve_client_with_overrides("graph_analyzer", provider_override, model_override) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to resolve LLM client: {}", e)),
    };

    let result = client.prompt(&model, system_prompt, &user_prompt, 2048, 0.7).await;

    match result {
        Ok(resp) => {
            let clean = clean_json_for_insights(&resp);
            if let Ok(qs) = serde_json::from_str::<Vec<GraphQuestion>>(&clean) {
                Ok(qs)
            } else {
                Err("Failed to parse JSON response from LLM".to_string())
            }
        }
        Err(e) => {
            tracing::error!("Graph Analytics Generation Error: {}", e);
            Err(format!("LLM Generation failed for Graph Insights: {}", e))
        }
    }
}
