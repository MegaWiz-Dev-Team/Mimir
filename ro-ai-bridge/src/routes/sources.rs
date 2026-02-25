use axum::{
    routing::{get, post, put, delete},
    Router, Json, extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use tokio::time::sleep;
use std::time::Duration;
use futures::stream::{self, Stream};
use std::convert::Infallible;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::sources::{DataSource, CreateDataSourceRequest, UpdateDataSourceRequest};
use serde_json::{json, Value};
use tracing::info;

pub fn sources_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_sources).post(create_source))
        .route("/:id", put(update_source).delete(delete_source))
        .route("/:id/sync", post(sync_source))
        .route("/:id/logs", get(stream_logs))
}

async fn list_sources(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<DataSource>>, (StatusCode, Json<Value>)> {
    // Note: To truly support multi-tenancy, we should extract the tenant_id from the Auth token middleware
    // We'll mock it here temporarily or retrieve from Extension if added by middleware
    let tenant_id = "default_tenant"; 
    
    let sources = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(sources))
}

async fn create_source(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateDataSourceRequest>,
) -> Result<(StatusCode, Json<DataSource>), (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant"; // Future: get from auth token
    
    let result = sqlx::query!(
        r#"
        INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
        VALUES (?, ?, ?, ?, ?)
        "#,
        tenant_id,
        payload.name,
        payload.source_type,
        payload.config_json,
        payload.schedule
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let new_source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ?"
    )
    .bind(result.last_insert_id())
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok((StatusCode::CREATED, Json(new_source)))
}

async fn update_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDataSourceRequest>,
) -> Result<Json<DataSource>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    // Check if source exists
    let existing = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if existing.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))));
    }
    
    let current = existing.unwrap();
    let updated_name = payload.name.unwrap_or(current.name);
    let updated_config = payload.config_json.unwrap_or(current.config_json);
    let updated_schedule = payload.schedule.or(current.schedule);

    sqlx::query!(
        "UPDATE data_sources SET name = ?, config_json = ?, schedule = ? WHERE id = ? AND tenant_id = ?",
        updated_name,
        updated_config,
        updated_schedule,
        id,
        tenant_id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let updated_source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(updated_source))
}

async fn delete_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    let result = sqlx::query!(
        "DELETE FROM data_sources WHERE id = ? AND tenant_id = ?",
        id,
        tenant_id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found or access denied"}))));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn sync_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    // Check if source exists
    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if source.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))));
    }
    
    // Update status to RUNNING
    sqlx::query!(
        "UPDATE data_sources SET last_sync_status = 'RUNNING' WHERE id = ?",
        id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    // In a real application, we would spawn a background task or send to a message queue here.
    // We will simulate it picking up the job asynchronously for now.
    info!("Triggered sync for source id {}", id);

    Ok((StatusCode::ACCEPTED, Json(json!({
        "message": "Sync job triggered successfully",
        "source_id": id
    }))))
}

// Simulated SSE stream for real-time logs
async fn stream_logs(
    Path(id): Path<i64>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("Client connected to log stream for source {}", id);
    
    // In a real application, you'd subscribe to a broadcast channel or access a log file/database.
    // This is a simple mock stream that yields logging messages every second.
    let stream = async_stream::stream! {
        yield Ok(Event::default().data(format!("> Connected to log stream for source #{}", id)));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> Initializing ingress workers..."));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> Fetching data source configuration..."));
        sleep(Duration::from_secs(2)).await;
        yield Ok(Event::default().data("> Processing data elements..."));
        sleep(Duration::from_secs(2)).await;
        yield Ok(Event::default().data("> Adding to Vector Space..."));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> COMPLETED. Worker shutting down."));
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive-text"),
    )
}
