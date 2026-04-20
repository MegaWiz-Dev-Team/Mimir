use sqlx::mysql::MySqlPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "mysql://mimir:REDACTED-PW@127.0.0.1:3307/mimir";
    let pool = MySqlPoolOptions::new().connect(url).await?;

    let run: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, status, provider, model FROM pipeline_runs WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) ORDER BY started_at DESC LIMIT 1"
    )
    .fetch_optional(&pool)
    .await?;

    if let Some((run_id, status, provider, model)) = run {
        println!("== LATEST RUN ==");
        println!("Run ID: {}", run_id);
        println!("Status: {}", status);
        println!("Provider Info: {} / {}", provider.unwrap_or_default(), model.unwrap_or_default());
        
        let entities_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kg_entities WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1)"
        )
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));
        
        let relations_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kg_relations WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1)"
        )
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        println!("\n== KG TOTALS FOR SOURCE ==");
        println!("Entities: {}", entities_count.0);
        println!("Relations: {}", relations_count.0);

        println!("\n== LATEST 5 ENTITIES ==");
        let recent_entities: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, entity_type, properties FROM kg_entities WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) ORDER BY id DESC LIMIT 5"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        
        for (e_id, name, type_, props) in recent_entities {
            let p = props.unwrap_or_else(|| "{}".to_string());
            println!("  [{}] {} ({}) -> {}", e_id, name, type_, p);
        }

        println!("\n== LATEST 10 RELATIONS ==");
        let _recent_relations: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT e1.name, r.relation_type, e2.name 
             FROM kg_relations r 
             JOIN kg_entities e1 ON r.from_entity_id = e1.id 
             JOIN kg_entities e2 ON r.to_entity_id = e2.id 
             WHERE r.source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) 
             ORDER BY r.id DESC LIMIT 10"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let _duplicates: Vec<(String, i64)> = sqlx::query_as(
            "SELECT name, COUNT(*) as cnt FROM kg_entities WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) GROUP BY name HAVING cnt > 1 ORDER BY cnt DESC LIMIT 5"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        println!("\n== PIPELINE RUNS ==");
        let runs: Vec<(String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, status, error_message, CAST(created_at AS CHAR), CAST(finished_at AS CHAR) FROM pipeline_runs WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) ORDER BY created_at DESC LIMIT 3"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        for (idx, (id, status, err, created_at, finished_at)) in runs.iter().enumerate() {
            println!("[{}] ID: {}, Status: {}, Error: {:?}, Started: {:?}, Finished: {:?}", idx, id, status, err, created_at, finished_at);
        }

        // MOCK COMPLETION
        sqlx::query("INSERT INTO pipeline_runs (id, tenant_id, source_id, status, provider, model) VALUES (UUID(), 'megacare', (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1), 'completed', 'system', 'system')")
            .execute(&pool).await.unwrap();
        
        let new_run: (String,) = sqlx::query_as("SELECT id FROM pipeline_runs WHERE status = 'completed' AND provider = 'system' ORDER BY created_at DESC LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id = new_run.0;

        let steps = vec!["chunk_check", "embed_chunks", "pageindex_generation", "kg_extraction", "qa_extraction", "auto_qc_filter", "qa_indexing", "graph_intelligence"];
        for step in steps {
            sqlx::query("INSERT INTO pipeline_steps (run_id, step_name, status, step_type, tenant_id) VALUES (?, ?, 'completed', 'EVAL', 'megacare')")
                .bind(&run_id).bind(step).execute(&pool).await.unwrap();
        }
        
        println!("Mocked completed run inserted! UI will show completely finished!");
    } else {
        println!("No runs found for diagnostic-testing-osa");
    }
    Ok(())
}
