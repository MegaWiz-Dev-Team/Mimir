# Bifrost Project Specification (Updated: Rust Native Architecture)

> **MIGRATION STATUS (April 2026):**
> 🚨 **NOTICE:** The Bifrost project has undergone a complete architectural rewrite. The original Python/FastAPI/ADK (Agent Development Kit) ecosystem has been deprecated and replaced by a **Rust-native (Bifrost-RS)** architecture.

Bifrost is the specialized agent orchestration engine of the Mimir/Asgard ecosystem.

## 1. What is Bifrost-RS?

Bifrost-RS is a highly optimized, asynchronous microservice built with Rust and Axum. It is designed to host, orchestrate, and manage intelligent AI Agent workflows (referred to as a "Swarm"). Instead of hardcoding all reasoning inside the main Mimir backend, Bifrost acts as a specialized sidecar dedicated solely to complex Large Language Model inferences, planning, and memory coordination.

### Core Paradigms
- **The Overseer:** The central routing and planning agent. It interprets user intent and determines which agent modules (Tools) need to be invoked.
- **Skills (Tools):** Plugins that agents can use. These include RAG retrieval elements like the Vector Search, Graph Search, and Tree Search.
- **Souls (Memory):** Powered by `memvid-core`. Agents hold long-term state across sessions, stored as detached `.mv2` (Smart Frame) binary files.

## 2. Core Operational Flow (How it Works)

1. **Request Intake:** A user types a prompt into the `ro-ai-dashboard` (Agent Studio). The Mimir backend receives this and realizes the target is an advanced agent. It proxies the request to `http://localhost:8100/v1/agents/{agent_id}/run` (Bifrost).
2. **Context Assembly:** Bifrost receives the request. It fetches the Agent's identity and permissions. It initializes an instance of the Rig framework (`rig-core`).
3. **Memory Activation (The Soul):** The Agent spins up a `MemvidManager` corresponding to its `agent_id.mv2` file. It reads past contexts and brings them into its immediate context window if relevant.
4. **Tool Access Authorization:** Depending on tenant boundaries, the Agent is given instances of its allowed tools (e.g., `VectorSearchTool`, `GraphSearchTool`, `MemvidSearchTool`).
5. **The Reasoning Loop:** The agent interacts with the Swarm. It may choose to synthesize an answer immediately, or emit a sequence of function calls. If it calls a tool, the Swarm engine intercepts the response, embeds the RAG query directly against Qdrant/Neo4j, and feeds it back to the agent.
6. **Persistence:** Once a final response is generated, Bifrost logs the "Smart Frame" to its Memvid `.mv2` file for long-term survival, and proxies the final JSON response back to Mimir.

## 3. Technology Stack

- **Framework:** Rust + Axum + Tokio (Replacing FastAPI).
- **Agent Orchestrator:** `rig-core` by Playgrounds.
- **LLM Routing:** Mimir `LlmRouter` integration via `mimir-core-ai`.
- **Memory Engine:** `memvid-core` (Local `.mv2` flat-files instead of centralized MariaDB schemas for deep memory).
- **Dependencies:** `reqwest`, `serde`, `qdrant-client`, `neo4rs`.

## 4. Why Rust over Python?

The transition to Rust resolves critical operational bottlenecks observed in the Python RAG ADK:
1. **Concurrency:** Rig-core alongside Rust's async primitives enables handling thousands of concurrent agent loops without the Global Interpreter Lock (GIL) stalling API requests.
2. **Predictable Memory Limits:** RAG systems with massive context payloads often spiked Python garbage collection. Rust guarantees zero-cost abstractions and deterministic memory closure via the Drop trait.
3. **Unification:** Mimir, Heimdall, and now Bifrost share identically typed libraries via `mimir-core-ai`.

## 5. Deployment

Bifrost runs on **port 8100**. It requires persistent volume claims (PVC) in Kubernetes (or Docker Volumes) pointing to its `data/agents/` directory, as `memvid` relies heavily on file-level locking and I/O.
