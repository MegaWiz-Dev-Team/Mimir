# Mimir Enterprise: v1.0.0 Deployment Guide

Welcome to the **Mimir RAG Platform (Agentic Update)**. This document contains the final instructions to start up the environment including the new features deployed in Sprints 33-35: Agentic (Swarm) RAG, Auto-Tuner, and Cross-Encoder Re-Ranking.

## Architecture Prerequisites
Ensure the network is healthy:
1. `Heimdall LLM Gateway` (Python/Ollama/vLLM)
2. `Mimir API Core` (Rust/Axum)
3. `Mimir UI Dashboard` (Next.js)
4. Database Ecosystem (`MariaDB` and `Qdrant` via Docker Compose)

## Launching Services

**1. Start the Database Layer (From Mimir Root)**
```bash
# Start background DBs including MariaDB (Port 3306) and Qdrant (Port 6333)
docker compose up -d
```

**2. Start Heimdall LLM Gateway (From `heimdall-rs`)**
```bash
# Run the Rust Gateway for LLM API load-balancing
cd heimdall-rs
cargo run --release
```

**3. Start Mimir API Core (From `ro-ai-bridge`)**
```bash
# Start the Backend containing the RAG logic and Agent Swarm endpoints
cd ro-ai-bridge
cargo run --release
```

**4. Start the Frontend UI (From `ro-ai-dashboard`)**
```bash
# Launch the React Web UI
cd ro-ai-dashboard
npm run dev
# OR for production build:
# npm run build && npm start
```

## Using the New Features
Once all services are online, navigate to `http://localhost:3000`:

- **Agentic RAG (Swarm):** Go to **RAG Playground**, change Search Mode to **Autonomous Agent (Swarm)**, and ask complex multi-step questions. The Agent will decide its tools (Vector, Tree, Graph) dynamically.
- **Cross-Encoder Re-Ranking:** In the same Playground, change the Re-ranking Strategy to **Cross-Encoder (Accurate/Slower) 🚀**. This gives the highest accuracy via pairwise ranking API hooks to Heimdall.
- **Auto-Tuner:** Go to the **Evaluation** tab. Select exactly 1 Run, and press the **🔥 Auto-Tune** button at the top to optimize system hyperparameters via Genetic / Optuna background jobs.

## Troubleshooting

- `Connection Refused` on API calls: Ensure `DATABASE_URL` is correct in `.env` and `Qdrant` is running on port 6333.
- `Flash-MoE` models not loading: Ensure Heimdall `OPENAI_API_BASE` points to the correct vLLM or Inference port as per Sprint 33.
- For DB inconsistencies, you can re-run backend migrations with `cargo sqlx migrate run` inside `ro-ai-bridge/mimir-core-ai`.
