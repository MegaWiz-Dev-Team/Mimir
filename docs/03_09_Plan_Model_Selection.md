# Plan: Model Selection for Agent Playground

## Overview
Add the ability to select AI models (both local Ollama and cloud Gemini) in the Agent Playground, similar to the model selection in the main dashboard.

## Architecture

### Current State
- Backend uses hardcoded Ollama client via `rig::providers::ollama`
- Frontend has no model selection in playground
- ChatRequest only has: `tier`, `message`, `persona`, `session_id`

### Target State
- Backend supports multiple providers (Ollama, Gemini)
- Frontend has Provider + Model selectors
- ChatRequest includes: `provider`, `model`

---

## Implementation Plan

### Phase 1: Backend Changes

#### 1.1 Update ChatRequest Struct
**File:** `ro-ai-bridge/src/bin/monitor.rs`

```rust
#[derive(Deserialize)]
struct ChatRequest {
    tier: i8,
    message: String,
    persona: String,
    session_id: Option<String>,
    provider: Option<String>,  // NEW: "ollama" | "gemini"
    model: Option<String>,      // NEW: model name
}
```

#### 1.2 Update ChatResponse Struct
```rust
#[derive(Serialize)]
struct ChatResponse {
    content: String,
    tier: i8,
    persona: String,
    latency_ms: u64,
    provider: String,           // NEW
    model: String,              // NEW
    confidence_score: Option<f32>,
    confidence_level: Option<String>,
    sources: Option<Vec<serde_json::Value>>,
    tools_used: Option<Vec<String>>,
}
```

#### 1.3 Create Provider-Agnostic Agent Factory
**File:** `ro-ai-bridge/src/agents/oracle_rag.rs`

```rust
pub enum LlmProvider {
    Ollama,
    Gemini,
}

impl OracleRagAgent {
    pub fn with_provider(
        persona: Persona,
        qdrant: QdrantService,
        db_pool: Option<DbPool>,
        provider: LlmProvider,
        model: &str,
    ) -> Self {
        match provider {
            LlmProvider::Ollama => {
                // Current implementation
            }
            LlmProvider::Gemini => {
                // Use rig::providers::gemini
            }
        }
    }
}
```

#### 1.4 Update Chat Handler
**File:** `ro-ai-bridge/src/bin/monitor.rs`

```rust
async fn chat_handler(...) -> impl IntoResponse {
    let provider = payload.provider.as_deref().unwrap_or("ollama");
    let model = payload.model.as_deref().unwrap_or("llama3.2");
    
    match payload.tier {
        1 => {
            let agent = SimpleNpcAgent::with_model_and_provider(
                persona, 
                model, 
                provider
            );
            // ...
        }
        2 => {
            let agent = OracleRagAgent::with_provider(
                persona,
                state.qdrant.clone(),
                Some(state.db.clone()),
                provider,
                model,
            );
            // ...
        }
    }
}
```

### Phase 2: Frontend Changes

#### 2.1 Update API Types
**File:** `ro-ai-dashboard/src/lib/api.ts`

```typescript
export interface ChatRequest {
    tier: 1 | 2;
    message: string;
    persona: string;
    session_id?: string;
    provider?: "ollama" | "gemini";  // NEW
    model?: string;                   // NEW
}

export interface ChatResponse {
    content: string;
    tier: number;
    persona: string;
    latency_ms: number;
    provider: string;           // NEW
    model: string;              // NEW
    confidence_score?: number;
    confidence_level?: string;
    sources?: SourceCitation[];
    tools_used?: string[];
}

// Model configurations
export const PROVIDERS = {
    ollama: {
        name: "Ollama (Local)",
        models: [
            { id: "llama3.2", name: "Llama 3.2", recommended: true },
            { id: "llama3.2:1b", name: "Llama 3.2 1B (Fast)" },
            { id: "mistral", name: "Mistral" },
            { id: "phi3:3.8b", name: "Phi 3 Mini" },
        ],
    },
    gemini: {
        name: "Gemini (Cloud)",
        models: [
            { id: "gemini-2.5-flash", name: "Gemini 2.5 Flash", recommended: true },
            { id: "gemini-2.5-pro", name: "Gemini 2.5 Pro" },
            { id: "gemini-2.0-flash", name: "Gemini 2.0 Flash" },
        ],
    },
};
```

#### 2.2 Update Playground UI
**File:** `ro-ai-dashboard/src/app/playground/page.tsx`

Add to state:
```typescript
const [provider, setProvider] = useState<"ollama" | "gemini">("ollama");
const [model, setModel] = useState("llama3.2");
```

Add to Settings Panel:
```tsx
{/* Provider Selector */}
<div className="space-y-2">
    <Label>Provider</Label>
    <Select value={provider} onValueChange={(v) => {
        setProvider(v as "ollama" | "gemini");
        // Set default model for provider
        setModel(v === "ollama" ? "llama3.2" : "gemini-2.5-flash");
    }}>
        <SelectTrigger>
            <SelectValue />
        </SelectTrigger>
        <SelectContent>
            <SelectItem value="ollama">
                <div className="flex items-center gap-2">
                    <Zap className="h-4 w-4 text-yellow-500" />
                    Ollama (Local)
                </div>
            </SelectItem>
            <SelectItem value="gemini">
                <div className="flex items-center gap-2">
                    <Cloud className="h-4 w-4 text-blue-500" />
                    Gemini (Cloud)
                </div>
            </SelectItem>
        </SelectContent>
    </Select>
</div>

{/* Model Selector */}
<div className="space-y-2">
    <Label>Model</Label>
    <Select value={model} onValueChange={setModel}>
        <SelectTrigger>
            <SelectValue />
        </SelectTrigger>
        <SelectContent>
            {PROVIDERS[provider].models.map((m) => (
                <SelectItem key={m.id} value={m.id}>
                    {m.name}
                    {m.recommended && <Badge className="ml-2">Recommended</Badge>}
                </SelectItem>
            ))}
        </SelectContent>
    </Select>
</div>
```

Update sendChat call:
```typescript
const response = await sendChat({
    tier,
    message: userMessage,
    persona,
    provider,
    model,
});
```

---

## Dependencies

### Backend
- `rig-core` already supports Gemini via `rig::providers::gemini`
- Need to add Gemini API key to environment: `GEMINI_API_KEY`

### Frontend
- No new dependencies needed
- Uses existing shadcn/ui components

---

## Environment Variables

```bash
# .env
OLLAMA_BASE_URL=http://localhost:11434
GEMINI_API_KEY=your-gemini-api-key
```

---

## Testing Checklist

- [ ] Tier 1 with Ollama Llama 3.2
- [ ] Tier 1 with Gemini Flash
- [ ] Tier 2 with Ollama (RAG)
- [ ] Tier 2 with Gemini (RAG)
- [ ] Model switching preserves conversation
- [ ] Error handling for missing API keys
- [ ] Streaming works with both providers

---

## Estimated Effort

| Task                         | Time     |
| ---------------------------- | -------- |
| Backend provider abstraction | 2h       |
| Backend chat handler update  | 1h       |
| Frontend UI components       | 1h       |
| Frontend API integration     | 30m      |
| Testing                      | 1h       |
| **Total**                    | **5.5h** |

---

## Notes

1. **Gemini Streaming**: rig-core supports Gemini streaming, but SSE format may differ. Need to verify.

2. **Rate Limits**: Gemini has API rate limits. Consider adding rate limit handling.

3. **Cost Tracking**: Could add token usage tracking for Gemini in the future.

4. **Fallback**: If Gemini fails, could auto-fallback to Ollama.
