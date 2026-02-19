# Wiki Q/A Pipeline Monitoring System Implement Plan

## 1. Overview
The **Wiki Q/A Pipeline Monitoring System** provides a robust, database-backed infrastructure to track, control, and audit the ingestion and processing of data from Wiki sources. Moving from a CLI-based execution to an API-driven architecture allows for real-time monitoring, failure recovery, and better integration with frontend dashboards.

## 2. Architecture

### 2.1 Backend Layer
- **Language**: Rust
- **Framework**: Axum (Web Server) + Tokio (Async Runtime)
- **Database**: MariaDB 11 (via `sqlx-mysql`)
- **JSON Serialization**: `serde` + `serde_json`

### 2.2 Database Schema (MariaDB)
The system uses 4 core tables to track the pipeline lifecycle.

#### `pipeline_runs`
Master table for each execution session.
```sql
CREATE TABLE pipeline_runs (
    id VARCHAR(36) PRIMARY KEY, -- UUID
    status VARCHAR(20) NOT NULL, -- RUNNING, COMPLETED, FAILED
    provider VARCHAR(50) NOT NULL, -- ollama, gemini
    model VARCHAR(50) NOT NULL,
    test_run BOOLEAN DEFAULT FALSE,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    INDEX idx_status (status),
    INDEX idx_started_at (started_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

#### `pipeline_steps`
Tracks individual processing steps (files or chunks).
```sql
CREATE TABLE pipeline_steps (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    chunk_index INT DEFAULT 0,
    status VARCHAR(20) NOT NULL, -- PENDING, IN_PROGRESS, COMPLETED, FAILED
    step_type VARCHAR(20) NOT NULL, -- EXTRACT, GENERATE, VERIFY
    error_message TEXT,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    FOREIGN KEY (run_id) REFERENCES pipeline_runs(id) ON DELETE CASCADE,
    INDEX idx_run_id (run_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

#### `qa_results`
Stores the generated Q/A pairs.
```sql
CREATE TABLE qa_results (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    context TEXT, -- The chunk used
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES pipeline_runs(id) ON DELETE CASCADE,
    INDEX idx_run_id (run_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

#### `evaluation_reports`
Stores verification results and coverage scores.
```sql
CREATE TABLE evaluation_reports (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    atomic_facts JSON, -- MariaDB supports JSON column
    coverage_score FLOAT,
    reasoning TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES pipeline_runs(id) ON DELETE CASCADE,
    INDEX idx_run_id (run_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

## 3. API Endpoints (Axum)

| Method | Endpoint                  | Description                                                                                                          |
| :----- | :------------------------ | :------------------------------------------------------------------------------------------------------------------- |
| `POST` | `/api/pipeline/run`       | Trigger a new pipeline run. Accepts JSON: `{ "provider": "gemini", "model": "gemini-2.0-flash", "test_run": false }` |
| `GET`  | `/api/pipeline/runs`      | List recent pipeline runs with status summaries.                                                                     |
| `GET`  | `/api/pipeline/runs/{id}` | Get detailed info for a specific run, including step statuses and failure reasons.                                   |

## 4. Implementation Steps

1.  **Infrastructure Setup**:
    -   Ensure MariaDB container is running in `docker-compose.yml`.
    -   Create database `ro_landverse` if not exists.

2.  **Migration**:
    -   Create SQL migration scripts in `migrations/`.
    -   Run migrations using `sqlx migrate`.

3.  **Application Logic**:
    -   Update `Cargo.toml` to use `sqlx` with `mysql` feature.
    -   Refactor `src/services/db.rs` to connect to MariaDB.
    -   Update `pipeline.rs` to use MySQL bindings (`?` syntax).
    -   Update `monitor.rs` API handlers.

4.  **Verification**:
    -   Run `cargo run --bin monitor`.
    -   Trigger test run via `curl`.
    -   Verify data in MariaDB using client or CLI.
