use serde_json::{json, Value};
use sqlx::mysql::MySqlPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    // Hardcode connection string if missing for standalone dev tests
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "mysql://mimir:REDACTED-PW@127.0.0.1:3307/mimir".to_string());
    
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    println!("Connected to Database. Scanning for duplicated entities...");

    // Find all names that appear more than once within the same tenant
    let duplicate_clusters: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT tenant_id, LOWER(name) as lower_name, COUNT(*) as cnt 
         FROM kg_entities 
         GROUP BY tenant_id, lower_name 
         HAVING cnt > 1"
    )
    .fetch_all(&pool)
    .await?;

    if duplicate_clusters.is_empty() {
        println!("No duplicates found!");
        return Ok(());
    }

    println!("Found {} names with duplicates.", duplicate_clusters.len());
    let mut total_remapped = 0;
    let mut total_deleted = 0;

    for (tenant_id, lower_name, count) in duplicate_clusters {
        // Fetch all IDs and chunk_ids for this duplicate cluster
        let records: Vec<(i64, Option<i64>, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT id, chunk_id, properties 
             FROM kg_entities 
             WHERE tenant_id = ? AND LOWER(name) = ?
             ORDER BY id ASC"
        )
        .bind(&tenant_id)
        .bind(&lower_name)
        .fetch_all(&pool)
        .await?;

        if records.len() < 2 {
            continue; // Safety check
        }

        // 1st record is our primary survivor
        let primary_id = records[0].0;
        let mut primary_props: Value = records[0].2.as_ref()
            .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            .and_then(|p| serde_json::from_str(&p).ok())
            .unwrap_or(json!({}));

        // Collect all unique chunk IDs
        let mut chunks = Vec::new();
        for (_, c_id, _) in &records {
            if let Some(c) = c_id {
                if !chunks.contains(c) {
                    chunks.push(*c);
                }
            }
        }
        
        // Merge into properties
        if let Some(obj) = primary_props.as_object_mut() {
            obj.insert("found_in_chunks".to_string(), json!(chunks));
        }

        let new_props_str = primary_props.to_string();
        
        // Update the primary entity with the new properties
        sqlx::query("UPDATE kg_entities SET properties = ? WHERE id = ?")
            .bind(&new_props_str)
            .bind(primary_id)
            .execute(&pool)
            .await?;

        // Process duplicates
        for i in 1..records.len() {
            let dup_id = records[i].0;

            // Remap relations FROM duplicate
            let remap_from = sqlx::query("UPDATE kg_relations SET from_entity_id = ? WHERE from_entity_id = ?")
                .bind(primary_id)
                .bind(dup_id)
                .execute(&pool)
                .await?;
            total_remapped += remap_from.rows_affected();

            // Remap relations TO duplicate
            let remap_to = sqlx::query("UPDATE kg_relations SET to_entity_id = ? WHERE to_entity_id = ?")
                .bind(primary_id)
                .bind(dup_id)
                .execute(&pool)
                .await?;
            total_remapped += remap_to.rows_affected();

            // Delete the duplicate
            let del = sqlx::query("DELETE FROM kg_entities WHERE id = ?")
                .bind(dup_id)
                .execute(&pool)
                .await?;
            total_deleted += del.rows_affected();
        }

        println!("Resolved '{}' -> Kept ID {}, Deleted {} duplicates, Remapped {} relations", 
            lower_name, primary_id, records.len() - 1, total_remapped);
    }

    println!("\n=== Cleanup Complete ===");
    println!("Total duplicates deleted: {}", total_deleted);
    println!("Total relations remapped: {}", total_remapped);

    Ok(())
}
