# RO-AI Bridge

A robust AI middleware and data pipeline for **Project Mimir**. This engine handles web scraping, automated Q/A generation, vector indexing, and exposes a monitoring API for the management dashboard.

## 🚀 Core Features

- **🌐 Wiki Scraper (`fetch_wiki`)**: 
  - Headless browser scraping using `chromiumoxide`.
  - Converts HTML structures into clean Markdown.
  - Removes navigation, scripts, and sidebars automatically.
- **🧠 Multi-Agent Q/A Pipeline (`generate_qa`)**:
  - **Generator Agent**: Creates high-quality Q/A pairs from content chunks (Ollama/Gemini).
  - **Extractor Agent**: Identifies Atomic Facts (ACUs) for verification.
  - **Verifier Agent**: Validates Q/A accuracy against facts, providing a coverage score.
- **⚡ Vector Indexer (`run_indexer`)**:
  - Syncs Q/A pairs from **MariaDB** to **Qdrant**.
  - Uses `nomic-embed-text` via Ollama for local high-performance embeddings.
  - Supports incremental indexing via `indexed_at` tracking.
- **🛠 Monitor API (`monitor`)**:
  - Built with **Axum** to provide real-time pipeline oversight.
  - Supports **Pipeline Resume**: Recover failed runs from the last successful step.
  - Vector management endpoints (Index status, Search preview).

## 🛠 Prerequisites

1.  **Rust Stack**: Latest stable version.
2.  **Infrastructure**: Ensure MariaDB and Qdrant are running (via root `docker-compose.yml`).
3.  **Local LLM**: Ollama with `llama3.2` and `nomic-embed-text`.
4.  **Cloud Access**: `GEMINI_API_KEY` in your `.env`.

## ⚙️ Configuration (`.env`)

```bash
DATABASE_URL=mysql://mimir:REDACTED-PW@localhost:3306/rathena
GEMINI_API_KEY=your_key_here
GEMINI_MODEL=gemini-2.0-flash
MONITOR_PORT=8080
```

## 📖 Usage

### 1. Data Ingestion
Scrape the wiki and prepare local markdown files:
```bash
cargo run --bin fetch_wiki
```

### 2. Q/A Generation
Process markdown files into the MariaDB database:
```bash
cargo run --bin generate_qa
```

### 3. Vector Sync
Index the generated Q/A pairs into Qdrant:
```bash
# Via CLI
cargo run --bin run_indexer
# Or via Monitor API (recommended)
curl -X POST http://localhost:8080/api/vector/index
```

### 4. Background Monitor
Start the management API for the dashboard:
```bash
cargo run --bin monitor
```

## 📊 Monitoring API Endpoints

- `GET /api/pipeline/runs`: List all runs.
- `POST /api/pipeline/runs/{id}/resume`: Resume a failed run.
- `POST /api/vector/search`: Preview RAG retrieval results.
- `GET /api/vector/stats`: Check MariaDB vs Qdrant sync status.

---
*Part of the Project-Mimir AI Ecosystem.*
