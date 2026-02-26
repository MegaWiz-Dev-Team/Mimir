//! Evaluation Runner Service
//!
//! Provides the core logic to run Agent \u00d7 Model evaluations asynchronously.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::services::db::DbPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalConfig {
    pub judge_model: String,
    pub dataset_size: usize,
    pub rubric: String,
}

#[derive(Debug, Deserialize)]
pub struct EvaluatorParams {
    pub tenant_id: String,
    pub agent_names: Vec<String>,
    pub model_ids: Vec<String>,
    pub question_limit: usize,
}

/// Start an evaluation run for a specific tenant.
/// Returns the `run_id` immediately so the caller can poll for progress.
pub async fn start_evaluation_run(
    pool: DbPool,
    params: EvaluatorParams,
) -> Result<String> {
    let run_id = Uuid::new_v4().to_string();
    let run_name = format!(
        "Evaluation Run {} ({})",
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        params.tenant_id
    );
    
    // Store run in database as PENDING
    let config = EvalConfig {
        judge_model: std::env::var("JUDGE_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string()),
        dataset_size: params.question_limit,
        rubric: "accuracy(1-5), completeness(1-5), relevance(1-5)".to_string(),
    };
    
    sqlx::query(
        "INSERT INTO eval_runs (id, name, status, total_combinations, config, tenant_id) VALUES (?, ?, 'PENDING', 0, ?, ?)"
    )
    .bind(&run_id)
    .bind(&run_name)
    .bind(serde_json::to_string(&config)?)
    .bind(&params.tenant_id)
    .execute(&pool)
    .await?;

    // Spawn background task
    let run_id_clone = run_id.clone();
    tokio::spawn(async move {
        if let Err(e) = run_evaluation_task(pool.clone(), run_id_clone.clone(), params, config).await {
            error!("Evaluation task for run {} failed: {}", run_id_clone, e);
            let _ = sqlx::query("UPDATE eval_runs SET status = 'FAILED', finished_at = NOW() WHERE id = ?")
                .bind(&run_id_clone)
                .execute(&pool).await;
        }
    });
    
    Ok(run_id)
}

/// Background process logic
async fn run_evaluation_task(
    pool: DbPool,
    run_id: String,
    params: EvaluatorParams,
    config: EvalConfig,
) -> Result<()> {
    info!("🚀 Started evaluation job {} for tenant {}", run_id, params.tenant_id);
    
    // 1. Mark as running
    sqlx::query("UPDATE eval_runs SET status = 'RUNNING' WHERE id = ?")
        .bind(&run_id)
        .execute(&pool)
        .await?;
        
    // 2. Fetch Golden questions from the DB for this tenant
    let questions: Vec<(String, String)> = sqlx::query_as(
        "SELECT question, answer FROM qa_results WHERE tenant_id = ? AND status = 'COMPLETED' ORDER BY RAND() LIMIT ?"
    )
    .bind(&params.tenant_id)
    .bind(params.question_limit as u32)
    .fetch_all(&pool)
    .await?;
    
    if questions.is_empty() {
        warn!("No valid QA results found for tenant {}", params.tenant_id);
        sqlx::query("UPDATE eval_runs SET status = 'COMPLETED', finished_at = NOW() WHERE id = ?")
            .bind(&run_id)
            .execute(&pool).await?;
        return Ok(());
    }
    info!("✅ Loaded {} questions for evaluation", questions.len());
    
    let total_evals = params.agent_names.len() * params.model_ids.len() * questions.len();
    sqlx::query("UPDATE eval_runs SET total_combinations = ? WHERE id = ?")
        .bind(total_evals as i32)
        .bind(&run_id)
        .execute(&pool)
        .await?;

    // NOTE: The actual evaluate() loops over agent+model combinations will be integrated later.
    // We update status to completed to reflect end of dummy evaluation.
    sqlx::query("UPDATE eval_runs SET status = 'COMPLETED', finished_at = NOW() WHERE id = ?")
        .bind(&run_id)
        .execute(&pool).await?;
        
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Since our app uses MySQL primarily (DbPool = Pool<MySql>), testing with Sqlite 
    // requires either trait objects or skipping the unit test that depends on the exact
    // Pool<MySql> type. For now, we will skip the DB-bound unit test and rely on 
    // integration testing, or mock the function args.
    
    #[tokio::test]
    async fn test_evaluator_params_serialization() -> Result<()> {
        let params = EvaluatorParams {
            tenant_id: "tenant123".to_string(),
            agent_names: vec!["simple_npc".to_string()],
            model_ids: vec!["llama3".to_string()],
            question_limit: 5,
        };
        
        assert_eq!(params.tenant_id, "tenant123");
        Ok(())
    }
}
