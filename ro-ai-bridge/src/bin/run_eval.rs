//! Evaluation Runner — Executes Agent × Model matrix evaluation
//!
//! Usage:
//!   cargo run --bin run_eval
//!   TEST_RUN=1 cargo run --bin run_eval    # Single combo only
//!
//! Reads Q/A dataset, runs each question through each compatible agent-model
//! combination, scores with LLM-as-Judge, and stores results in MariaDB.

use anyhow::Result;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::fs;
use tracing::{info, warn, error};
use uuid::Uuid;

use ro_ai_bridge::services::db;
use ro_ai_bridge::agents::eval::{
    evaluate_agent, judge_response, available_agents, is_compatible,
};

#[derive(Debug, Deserialize)]
struct QAPair {
    question: String,
    answer: String,
}

#[derive(Debug, Serialize)]
struct RunConfig {
    dataset_file: String,
    dataset_size: usize,
    judge_model: String,
    rubric: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let is_test_run = env::var("TEST_RUN").unwrap_or_default() == "1";
    let judge_model = env::var("JUDGE_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());

    // ─── 1. Load Q/A Dataset ───────────────────────────────────────────
    let dataset_path = "data/qa_dataset.json";
    info!("📂 Loading Q/A dataset from {}", dataset_path);
    let dataset_raw = fs::read_to_string(dataset_path).await?;
    let dataset: Vec<QAPair> = serde_json::from_str(&dataset_raw)?;
    info!("✅ Loaded {} Q/A pairs", dataset.len());

    if dataset.is_empty() {
        error!("❌ Dataset is empty! Run generate_qa first.");
        return Ok(());
    }

    // ─── 2. Connect to Database ────────────────────────────────────────
    info!("🔌 Connecting to database...");
    let pool = db::init_db().await?;
    info!("✅ Database connected");

    // ─── 3. Load Active LLM Models ────────────────────────────────────
    let models = db::get_active_llm_models(&pool).await?;
    info!("📋 Found {} active LLM models:", models.len());
    for m in &models {
        info!("   • {} ({})", m.model_id, m.provider);
    }

    let agents = available_agents();
    info!("🤖 Agents to evaluate: {:?}", agents);

    // ─── 4. Create Evaluation Run ──────────────────────────────────────
    let run_id = Uuid::new_v4().to_string();
    let run_name = format!(
        "Eval Run {} ({})",
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        if is_test_run { "TEST" } else { "FULL" }
    );

    let config = RunConfig {
        dataset_file: dataset_path.to_string(),
        dataset_size: dataset.len(),
        judge_model: judge_model.clone(),
        rubric: "accuracy(1-5), completeness(1-5), relevance(1-5)".to_string(),
    };

    // Count compatible combinations
    let mut total_combos = 0;
    for agent in &agents {
        for model in &models {
            if is_compatible(agent, &model.model_id) {
                total_combos += 1;
            }
        }
    }
    let total_evals = total_combos * dataset.len();

    sqlx::query(
        "INSERT INTO eval_runs (id, name, status, total_combinations, config) VALUES (?, ?, 'RUNNING', ?, ?)"
    )
    .bind(&run_id)
    .bind(&run_name)
    .bind(total_evals as i32)
    .bind(serde_json::to_string(&config)?)
    .execute(&pool)
    .await?;

    info!("🚀 Started run: {} ({})", run_name, run_id);
    info!("   Total evaluations: {} agents × {} models × {} questions = {}",
        agents.len(), models.len(), dataset.len(), total_evals
    );

    // ─── 5. Run Evaluations ────────────────────────────────────────────
    let mut completed = 0;
    let mut errors = 0;

    for agent_name in &agents {
        for model in &models {
            if !is_compatible(agent_name, &model.model_id) {
                info!("⏭️  Skipping incompatible: {} × {}", agent_name, model.model_id);
                continue;
            }

            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            info!("🧪 Evaluating: {} × {}", agent_name, model.model_id);

            for (qi, qa) in dataset.iter().enumerate() {
                info!("   📝 Q{}/{}: {:.60}...", qi + 1, dataset.len(),
                    qa.question.chars().take(60).collect::<String>());

                // Run the agent
                let eval_result = match evaluate_agent(
                    agent_name,
                    &model.model_id,
                    &qa.question,
                    Some(&pool),
                    None, // Qdrant: will use default
                ).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!("   ❌ Agent error: {}", e);
                        errors += 1;
                        
                        // Insert error row
                        sqlx::query(
                            "INSERT INTO eval_scores (run_id, agent_name, model_id, question, expected_answer, actual_answer) VALUES (?, ?, ?, ?, ?, ?)"
                        )
                        .bind(&run_id)
                        .bind(agent_name)
                        .bind(&model.model_id)
                        .bind(&qa.question)
                        .bind(&qa.answer)
                        .bind(format!("[ERROR] {}", e))
                        .execute(&pool)
                        .await?;
                        
                        continue;
                    }
                };

                info!("   ⏱️  Latency: {}ms", eval_result.latency_ms);
                info!("   📖 Answer: {:.80}...",
                    eval_result.answer.chars().take(80).collect::<String>());

                // LLM-as-Judge scoring
                let (accuracy, completeness, relevance, judge_reasoning) = 
                    match judge_response(&qa.question, &qa.answer, &eval_result.answer, &judge_model).await {
                        Ok(scores) => {
                            info!("   🎯 Scores: acc={} comp={} rel={}",
                                scores.accuracy, scores.completeness, scores.relevance);
                            (Some(scores.accuracy), Some(scores.completeness), Some(scores.relevance), Some(scores.reasoning))
                        }
                        Err(e) => {
                            warn!("   ⚠️  Judge failed: {} — scores will be NULL", e);
                            (None, None, None, None)
                        }
                    };

                // Store result
                sqlx::query(
                    r#"INSERT INTO eval_scores 
                        (run_id, agent_name, model_id, question, expected_answer, actual_answer,
                         accuracy_score, completeness_score, relevance_score, latency_ms,
                         judge_model, judge_reasoning)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&run_id)
                .bind(agent_name)
                .bind(&model.model_id)
                .bind(&qa.question)
                .bind(&qa.answer)
                .bind(&eval_result.answer)
                .bind(accuracy)
                .bind(completeness)
                .bind(relevance)
                .bind(eval_result.latency_ms as i32)
                .bind(&judge_model)
                .bind(&judge_reasoning)
                .execute(&pool)
                .await?;

                completed += 1;

                // Update progress
                sqlx::query("UPDATE eval_runs SET completed_combinations = ? WHERE id = ?")
                    .bind(completed)
                    .bind(&run_id)
                    .execute(&pool)
                    .await?;
            }

            if is_test_run {
                info!("🧪 TEST_RUN: Stopping after first compatible combination");
                break;
            }
        }

        if is_test_run {
            break;
        }
    }

    // ─── 6. Compute Summaries ──────────────────────────────────────────
    info!("📊 Computing evaluation summaries...");
    
    sqlx::query(
        r#"INSERT INTO eval_summary (run_id, agent_name, model_id, total_questions, avg_accuracy, avg_completeness, avg_relevance, avg_latency_ms, overall_score)
        SELECT 
            run_id, agent_name, model_id,
            COUNT(*) as total_questions,
            AVG(accuracy_score) as avg_accuracy,
            AVG(completeness_score) as avg_completeness,
            AVG(relevance_score) as avg_relevance,
            AVG(latency_ms) as avg_latency_ms,
            (AVG(accuracy_score) * 0.4 + AVG(completeness_score) * 0.3 + AVG(relevance_score) * 0.3) as overall_score
        FROM eval_scores
        WHERE run_id = ? AND accuracy_score IS NOT NULL
        GROUP BY run_id, agent_name, model_id"#
    )
    .bind(&run_id)
    .execute(&pool)
    .await?;

    // ─── 7. Finalize ───────────────────────────────────────────────────
    sqlx::query("UPDATE eval_runs SET status = 'COMPLETED', finished_at = NOW() WHERE id = ?")
        .bind(&run_id)
        .execute(&pool)
        .await?;

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("✨ Evaluation Complete!");
    info!("   Run ID: {}", run_id);
    info!("   Completed: {} | Errors: {}", completed, errors);
    info!("   Results stored in eval_scores & eval_summary tables");

    Ok(())
}
