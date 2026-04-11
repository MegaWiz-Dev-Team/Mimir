use sqlx::mysql::MySqlPoolOptions;

#[tokio::main]
async fn main() {
    let pool = MySqlPoolOptions::new()
        .connect("mysql://mimir:mimir123@127.0.0.1:3306/mimir")
        .await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM qa_clusters").fetch_one(&pool).await.unwrap();
    println!("Total clusters: {}", count.0);

    // Let's also check how many are scanned
    let scanned: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM qa_results WHERE qc_scanned = TRUE").fetch_one(&pool).await.unwrap();
    println!("Total QA scanned: {}", scanned.0);
}
