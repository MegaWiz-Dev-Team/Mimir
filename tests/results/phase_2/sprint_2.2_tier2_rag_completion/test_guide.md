# Testing Guide for Agent Playground

## Prerequisites

1. **Ollama** running with a model (e.g., `llama3.2`)
2. **Qdrant** running (for Tier 2 RAG)
3. **MySQL** database with rAthena data

## Step 1: Start the Backend

```bash
# Terminal 1: Start the Rust backend
cd ro-ai-bridge
cargo run --bin monitor
```

The API will be available at `http://localhost:8080`

## Step 2: Start the Frontend

```bash
# Terminal 2: Start the Next.js frontend
cd ro-ai-dashboard
npm run dev
```

The dashboard will be available at `http://localhost:3000`

## Step 3: Access the Playground

1. Open `http://localhost:3000` in your browser
2. Click **"Agent Playground"** in the top navigation
3. Or go directly to `http://localhost:3000/playground`

## Step 4: Test the Chat

### Tier 1 Test (Simple NPC - No RAG)
1. Select **Tier 1 - Simple NPC**
2. Select any persona (e.g., `sage_ariel`)
3. Type a message like: "Hello, who are you?"
4. Click Send

### Tier 2 Test (RAG Agent)
1. Select **Tier 2 - RAG Agent**
2. Select a persona:
   - **Sage Ariel** - Scholarly, detailed explanations
   - **Fortune Teller Maya** - Mysterious, cryptic responses
   - **Blacksmith Grumm** - Direct, practical responses
3. Ask game-related questions:
   - "Tell me about Poring monster"
   - "What drops from Orc Hero?"
   - "What is the best weapon for a Knight?"
4. Observe:
   - Streaming response (tokens appear one by one)
   - Confidence score badge
   - Source citations panel

## API Testing (Alternative)

### Non-streaming chat:
```bash
curl -X POST http://localhost:8080/api/agents/chat \
  -H "Content-Type: application/json" \
  -d '{"tier": 2, "message": "Tell me about Poring", "persona": "sage_ariel"}'
```

### Streaming chat (SSE):
```bash
curl -X POST http://localhost:8080/api/agents/chat/stream \
  -H "Content-Type: application/json" \
  -d '{"tier": 2, "message": "What drops from Orc Hero?", "persona": "fortune_teller"}'
```

## Expected Results

### Tier 1 Response:
```json
{
  "content": "Welcome, adventurer...",
  "tier": 1,
  "persona": "sage_ariel",
  "latency_ms": 1234
}
```

### Tier 2 Response:
```json
{
  "content": "The Poring is a jelly-like creature...",
  "tier": 2,
  "persona": "sage_ariel",
  "latency_ms": 2345,
  "confidence_score": 0.85,
  "confidence_level": "High",
  "sources": [
    {
      "source_type": "mob_db",
      "source_id": "Poring",
      "relevance": 0.95,
      "snippet": "Level 1 monster..."
    }
  ],
  "tools_used": ["QueryMobDbTool"]
}
```

## Troubleshooting

### "Failed to fetch" error
- Ensure backend is running on port 8080
- Check CORS settings in monitor.rs

### "Persona not found" error
- Check that persona YAML files exist in `ro-ai-bridge/config/personas/`

### Empty responses
- Ensure Ollama is running with the model: `ollama run llama3.2`
- Check OLLAMA_BASE_URL environment variable

### No sources shown (Tier 2)
- Ensure Qdrant is running and collections exist
- Run `cargo run --bin ingest_gamedata` to populate game data
