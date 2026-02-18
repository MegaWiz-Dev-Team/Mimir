# Implementation Plan: Axum Cron Job Integration (Sprint 1.4)

> [!NOTE]
> This plan is deferred for implementation on the Mac mini environment.

This plan outlines the steps to integrate a background cron job into the Axum server to periodically sync data from the GitBook MCP server.

## Proposed Changes

### Dependencies
#### [MODIFY] `ro-ai-bridge/Cargo.toml`
- Add `tokio-cron-scheduler = "0.13.0"`

### Services
#### [NEW] `ro-ai-bridge/src/services/cron.rs`
- Implement `init` function to start the scheduler.
- Define a job that runs every 4 hours (configurable).
- The job will:
    1. Initialize `McpClient`.
    2. Connect to MCP.
    3. Fetch resources.
    4. Provide logging/status updates.
- **Note**: For MVP, the job will log found resources. Full DB ingestion logic will be added when the Database layer (Sprint 1.3) is fully ready.

#### [MODIFY] `ro-ai-bridge/src/services/mod.rs`
- Expose `cron` module.

### Main Application
#### [MODIFY] `ro-ai-bridge/src/main.rs`
- Initialize the cron scheduler before starting the Axum server.

## Code Snippets

### `src/services/cron.rs`
```rust
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, error};
use std::time::Duration;
use crate::config::Config;
// use crate::services::mcp_client::McpClient;

pub async fn init(config: &Config) -> anyhow::Result<()> {
    let sched = JobScheduler::new().await?;
    
    // Example: Run every 4 hours
    // Cron format: sec min hour day_of_month month day_of_week year
    let cron_expression = "0 0 */4 * * *"; 

    sched.add(
        Job::new_async(cron_expression, |uuid, mut l| {
            Box::pin(async move {
                info!("Triggering scheduled MCP Sync job...");
                // Wrapper to handle errors
                if let Err(e) = run_sync_job().await {
                    error!("MCP Sync Job failed: {:?}", e);
                }
                info!("MCP Sync job finished.");
            })
        })?
    ).await?;

    sched.start().await?;
    info!("Cron scheduler started.");
    
    Ok(())
}

async fn run_sync_job() -> anyhow::Result<()> {
    // let client = McpClient::new(&std::env::var("MCP_URL")?);
    // client.connect().await?;
    // let resources = client.fetch_resources().await?;
    // info!("Synced {} resources from MCP", resources.len());
    Ok(())
}
```

### `src/main.rs` initialization
```rust
// ... inside main ...
// Start Cron Scheduler
if let Err(e) = services::cron::init(&config).await {
    error!("Failed to initialize cron scheduler: {:?}", e);
    // Decide if this is fatal or not. Usually not fatal for the server itself.
}
```

## Verification Plan

### Manual Verification
1.  **Modify Cron Schedule for Testing**: Temporarily set the cron schedule to run every 10 seconds (e.g., `1/10 * * * * *`).
2.  **Run Server**: `cargo run`
3.  **Check Logs**: Verify that "Triggering scheduled MCP Sync job..." appears in the logs.
4.  **Revert Schedule**: Set back to production schedule.
