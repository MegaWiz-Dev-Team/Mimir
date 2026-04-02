use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let model = "gemma:2b";
    let prompt = "Hi, are you ready for a speed test?";
    let url = "http://localhost:11434/api/generate";

    println!("🚀 Starting Latency Test for model: {}", model);
    println!("📡 Sending request to: {}", url);

    let start = Instant::now();

    let res = client
        .post(url)
        .json(&json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await?;

    let duration = start.elapsed();
    let status = res.status();

    if status.is_success() {
        let body: serde_json::Value = res.json().await?;
        let response_text = body["response"].as_str().unwrap_or("No response");

        println!("✅ Response received!");
        println!("⏱️  Duration: {:.2?}", duration);
        println!("📝 Output: {}", response_text.trim());

        // --- Save Result to File ---
        let target = std::time::Duration::from_millis(1800);
        let result_dir = "../tests/results/phase_1/sprint_1.1_latency";
        std::fs::create_dir_all(result_dir)?;

        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
        let result_filename = format!(
            "{}/{}_m3_{}.json",
            result_dir,
            timestamp,
            model.replace(":", "_")
        );

        let result_json = json!({
            "test_id": "latency_test",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "phase": "phase_1",
            "sprint": "sprint_1.1",
            "environment": {
                "hardware": "M3",
                "model": model,
                "provider": "Ollama"
            },
            "metrics": {
                "duration_ms": duration.as_millis(),
                "target_ms": 1800,
                "status": if duration <= target { "PASS" } else { "WARN" }
            }
        });

        std::fs::write(
            &result_filename,
            serde_json::to_string_pretty(&result_json)?,
        )?;
        println!("💾 Result saved to: {}", result_filename);
    } else {
        println!("❌ Request failed with status: {}", status);
    }

    Ok(())
}
