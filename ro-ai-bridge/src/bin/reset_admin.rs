use anyhow::{Context, Result};
use std::env;
use sqlx::MySqlPool;
use dotenvy::dotenv;

use mimir_core_ai::services::iam::IamService;

#[tokio::main]
async fn main() -> Result<()> {
    // Determine the environment and load the appropriate .env file
    let run_env = env::var("RUN_ENV").unwrap_or_else(|_| "development".to_string());
    if run_env == "test" {
        dotenvy::from_filename(".env.test").ok();
    } else {
        dotenv().ok();
    }

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <username> <new_password>", args[0]);
        eprintln!("Example: {} admin NewSecurePassword123!", args[0]);
        std::process::exit(1);
    }

    let username = &args[1];
    let new_password = &args[2];

    let db_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    
    println!("Connecting to database...");
    let pool = MySqlPool::connect(&db_url).await?;

    println!("Hashing new password for user '{}'...", username);
    let hash = IamService::hash_password(new_password)?;

    println!("Updating database record...");
    let result = sqlx::query(
        "UPDATE users SET password_hash = ? WHERE username = ?"
    )
    .bind(&hash)
    .bind(username)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        eprintln!("Error: User '{}' not found in the database.", username);
        std::process::exit(1);
    }

    println!("Success! Password for user '{}' has been reset.", username);
    println!("You may now log in to the dashboard with the new password.");

    Ok(())
}
