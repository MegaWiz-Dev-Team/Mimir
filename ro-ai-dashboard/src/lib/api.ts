import { PipelineRun, RunDetails, QAResult, EvaluationReport } from "@/types/pipeline";
import Cookies from "js-cookie";

export const API_BASE_URL = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:3000/api") + "/v1";

function getAuthHeaders(): HeadersInit {
    const token = Cookies.get("access_token");
    const tenantId = Cookies.get("tenant_id");

    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    if (tenantId) headers["X-Tenant-Id"] = tenantId;

    return headers;
}

// Custom fetch wrapper to auto-add auth headers + silent token refresh
let _isRefreshing = false; // Guard against concurrent refresh attempts

export async function authFetch(url: string, options: RequestInit = {}): Promise<Response> {
    const headers = {
        ...getAuthHeaders(),
        ...(options.headers || {})
    };

    const res = await fetch(url, { ...options, headers });

    // Only attempt refresh on 401 if we're in the browser and not already refreshing
    if (res.status === 401 && typeof window !== "undefined" && !_isRefreshing) {
        const refreshToken = Cookies.get("refresh_token");
        if (refreshToken) {
            _isRefreshing = true;
            try {
                const refreshRes = await fetch("/api/auth/refresh", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ refresh_token: refreshToken }),
                });

                if (refreshRes.ok) {
                    const { access_token, refresh_token: newRefresh, expires_in } = await refreshRes.json();

                    if (access_token) {
                        const days = expires_in ? expires_in / 86400 : 1;
                        Cookies.set("access_token", access_token, { expires: days });
                    }
                    if (newRefresh) {
                        Cookies.set("refresh_token", newRefresh, { expires: 30 });
                    }

                    // Retry original request with new token
                    const retryHeaders = {
                        ...getAuthHeaders(),
                        ...(options.headers || {}),
                    };
                    _isRefreshing = false;
                    return fetch(url, { ...options, headers: retryHeaders });
                }
            } catch (e) {
                console.warn("[authFetch] Token refresh failed:", e);
            } finally {
                _isRefreshing = false;
            }
        }

        // Refresh failed — DON'T auto-redirect to /login to avoid loop with SSO.
        // Just return the 401 response; UI components handle auth errors gracefully.
        console.warn("[authFetch] 401 received, token refresh unsuccessful.");
    }

    return res;
}

export async function fetchHealth(): Promise<{ status: string; version: string; service: string }> {
    const rawApiUrl = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:3000/api").replace("/api/v1", "").replace("/api", "");
    const res = await fetch(`${rawApiUrl}/health`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch health");
    return await res.json();
}

// ─── Pipeline API ───────────────────────────────────────────────────────────

export async function fetchRuns(): Promise<PipelineRun[]> {
    const res = await authFetch(`${API_BASE_URL}/pipeline/runs`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch runs");
    return res.json();
}

export async function fetchRunDetails(id: string): Promise<RunDetails> {
    const res = await authFetch(`${API_BASE_URL}/pipeline/runs/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch run details");
    return res.json();
}

export async function fetchStepQA(stepId: number): Promise<QAResult[]> {
    const res = await authFetch(`${API_BASE_URL}/pipeline/steps/${stepId}/qa`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch QA results");
    return res.json();
}

export async function fetchStepReport(stepId: number): Promise<EvaluationReport> {
    const res = await authFetch(`${API_BASE_URL}/pipeline/steps/${stepId}/report`, { cache: "no-store" });
    // Report might be 404 if not ready, handle gracefully in UI or here
    if (res.status === 404) return { id: 0, coverage_score: 0, reasoning: "Not available", atomic_facts: [], missing_facts: [] };
    if (!res.ok) throw new Error("Failed to fetch report");
    return res.json();
}

export async function triggerRun(provider: string = "ollama", model: string = "llama3.2", testRun: boolean = false) {
    const res = await authFetch(`${API_BASE_URL}/pipeline/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, model, test_run: testRun }),
    });
    if (!res.ok) throw new Error("Failed to trigger run");
    return res.json();
}

export async function retryStep(stepId: number) {
    const res = await authFetch(`${API_BASE_URL}/pipeline/steps/${stepId}/retry`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to retry step");
    return res;
}

export async function resumeRun(id: string) {
    const res = await authFetch(`${API_BASE_URL}/pipeline/runs/${id}/resume`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to resume run");
    return res;
}

export async function deleteModel(modelId: string) {
    try {
        const response = await authFetch(`${API_BASE_URL}/config/models/${modelId}`, {
            method: 'DELETE',
        });
        return await response.json();
    } catch (error) {
        console.warn("[API] Delete model error:", error);
        throw error;
    }
}

// ─── Data Quality Control ───────────────────────────────────────────────────

export async function fetchQcClusters(status?: string) {
    const query = status ? `?status=${status}` : '';
    const response = await authFetch(`${API_BASE_URL}/qc/clusters${query}`, { cache: "no-store" });
    if (!response.ok) throw new Error("Failed to fetch QC clusters");
    return await response.json();
}

export async function resolveQcCluster(clusterId: string, resolutionType: string, goldenAnswer?: string) {
    const response = await authFetch(`${API_BASE_URL}/qc/resolve/${clusterId}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ resolution_type: resolutionType, golden_answer: goldenAnswer })
    });
    if (!response.ok) throw new Error("Failed to resolve cluster");
    return true;
}

export async function triggerQcGeneration() {
    const response = await authFetch(`${API_BASE_URL}/qc/generate`, { method: "POST" });
    if (!response.ok) throw new Error("Failed to trigger QC generation");
    return await response.json();
}

export async function stopQcGeneration() {
    const response = await authFetch(`${API_BASE_URL}/qc/stop`, { method: "POST" });
    if (!response.ok) throw new Error("Failed to stop QC generation");
    return await response.json();
}

export async function fetchQcStatus() {
    const response = await authFetch(`${API_BASE_URL}/qc/status`, { cache: "no-store" });
    if (!response.ok) {
        throw new Error(`Failed to fetch QC status: ${response.statusText}`);
    }
    return await response.json();
}

export async function generateMissingQA(stepId: number) {
    const res = await authFetch(`${API_BASE_URL}/pipeline/steps/${stepId}/generate_missing`, {
        method: "POST",
    });
    if (!res.ok) {
        let msg = "Failed to generate missing Q/A";
        try {
            const errData = await res.json();
            if (errData.error) msg += `: ${errData.error}`;
        } catch (e) { }
        throw new Error(msg);
    }
    return res;
}

// ─── Vector API ──────────────────────────────────────────────────────────────

export async function fetchVectorStats() {
    const res = await authFetch(`${API_BASE_URL}/vector/stats`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch vector stats");
    return res.json();
}

export async function triggerIndexing() {
    const res = await authFetch(`${API_BASE_URL}/vector/index`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to trigger indexing");
    return res;
}

export async function searchVectors(query: string, limit: number = 5) {
    const res = await authFetch(`${API_BASE_URL}/vector/search`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query, limit }),
    });
    if (!res.ok) throw new Error("Failed to search vectors");
    return res.json();
}

// ─── Agent Chat API ──────────────────────────────────────────────────────────

export interface ChatRequest {
    tier: 1 | 2;
    message: string;
    persona: string;
    session_id?: string;
    provider?: string;  // Dynamic provider from database (ollama, google, etc.)
    model?: string;
}

export interface SourceCitation {
    source_type: string;
    source_id: string;
    relevance: number;
    snippet: string;
}

export interface ChatResponse {
    content: string;
    tier: number;
    persona: string;
    latency_ms: number;
    provider: string;
    model: string;
    confidence_score?: number;
    confidence_level?: string;
    sources?: SourceCitation[];
    tools_used?: string[];
    action?: any;
}

export interface StreamToken {
    token: string;
}

export interface StreamDone {
    latency_ms: number;
    confidence_score?: number;
    confidence_level?: string;
    sources?: SourceCitation[];
    action?: any;
}

export interface Persona {
    name: string;
    display_name: string;
    tier: number;
    description: string;
    greeting: string;
    avatar_url?: string;
    traits: string[];
}

/// Agent config from Agent Studio API (DB-backed)
export interface AgentConfigResponse {
    id: number;
    tenant_id: string;
    name: string;
    display_name?: string;
    description?: string;
    system_prompt: string;
    model_id: string;
    provider: string;
    temperature?: number;
    max_tokens?: number;
    top_k?: number;
    use_rag?: boolean;
    use_knowledge_graph?: boolean;
    tools?: string[];
    personality_traits?: string[];
    greeting?: string;
    avatar_url?: string;
    template_id?: string;
    is_published?: boolean;
    tier?: number;
    response_mode?: string;
    created_at?: string;
    updated_at?: string;
}

/// Fetch all agents from Agent Studio API
export async function fetchAgents(): Promise<AgentConfigResponse[]> {
    try {
        const res = await authFetch(`${API_BASE_URL}/agents`, { cache: "no-store" });
        if (!res.ok) return [];
        return await res.json();
    } catch {
        return [];
    }
}

/// Convert AgentConfigResponse to Persona format (for backwards compatibility)
export function agentToPersona(agent: AgentConfigResponse): Persona {
    return {
        name: agent.name,
        display_name: agent.display_name || agent.name,
        tier: agent.tier || 2,
        description: agent.description || "",
        greeting: agent.greeting || "",
        avatar_url: agent.avatar_url,
        traits: agent.personality_traits || [],
    };
}

export interface ModelConfig {
    model_id: string;
    provider: string;
    model_type: string;
    is_active: boolean;
    capabilities?: Record<string, boolean>;
    metadata?: Record<string, unknown>;
    created_at: string;
    updated_at: string;
}

export interface LlmProvider {
    id: string;
    display_name: string;
    description: string;
    models: LlmModel[];
    requires_api_key: boolean;
}

export interface LlmModel {
    id: string;
    display_name: string;
    description: string;
    capabilities?: Record<string, boolean>;
}

/// Fetch available models from the database
export async function fetchModels(): Promise<ModelConfig[]> {
    const res = await authFetch(`${API_BASE_URL}/models`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch models");
    return res.json();
}

/// Convert database models to provider format for UI
export function modelsToProviders(models: ModelConfig[]): LlmProvider[] {
    const providerMap = new Map<string, LlmProvider>();

    // Define provider metadata
    const providerMeta: Record<string, { display_name: string; description: string; requires_api_key: boolean }> = {
        heimdall: { display_name: "Heimdall (Self-Hosted)", description: "Self-hosted LLM gateway with multiple models", requires_api_key: true },
        flashmoe: { display_name: "Flash-MoE (Local SSD Streaming)", description: "Ultra-large MoE running from SSD", requires_api_key: false },
        ollama: { display_name: "Ollama (Local)", description: "Run models locally with Ollama", requires_api_key: false },
        google: { display_name: "Google Gemini (Cloud)", description: "Google's Gemini models via API", requires_api_key: true },
        openai: { display_name: "OpenAI (Cloud)", description: "OpenAI GPT models via API", requires_api_key: true },
        azure: { display_name: "Azure OpenAI", description: "Azure OpenAI models", requires_api_key: true },
    };

    for (const model of models) {
        const providerId = model.provider;

        if (!providerMap.has(providerId)) {
            const meta = providerMeta[providerId] || {
                display_name: providerId.charAt(0).toUpperCase() + providerId.slice(1),
                description: `${providerId} models`,
                requires_api_key: providerId !== "ollama"
            };
            providerMap.set(providerId, {
                id: providerId,
                display_name: meta.display_name,
                description: meta.description,
                requires_api_key: meta.requires_api_key,
                models: [],
            });
        }

        const provider = providerMap.get(providerId)!;
        provider.models.push({
            id: model.model_id,
            display_name: model.model_id,
            description: model.capabilities?.tools ? "Supports tools" : "Standard model",
            capabilities: model.capabilities,
        });
    }

    return Array.from(providerMap.values());
}

/// Fetch agents from Agent Studio and convert to Persona format for Playground
/// This replaces the old hardcoded PERSONAS array — agents in DB are now the single source of truth
export async function fetchPlaygroundAgents(): Promise<{ personas: Persona[]; agents: AgentConfigResponse[] }> {
    const agents = await fetchAgents();
    if (agents.length === 0) {
        return { personas: [], agents: [] };
    }
    // Sort by tier (Tier 1 first) then by name
    agents.sort((a, b) => (a.tier || 2) - (b.tier || 2) || a.name.localeCompare(b.name));
    const personas = agents.map(agentToPersona);
    return { personas, agents };
}

/// Fallback providers when database is not available
export const PROVIDERS: LlmProvider[] = [
    {
        id: "heimdall",
        display_name: "Heimdall (Self-Hosted)",
        description: "Self-hosted LLM gateway with multiple models",
        requires_api_key: true,
        models: [
            { id: "mlx-community/Qwen3.5-35B-A3B-4bit", display_name: "Qwen 3.5 35B MoE", description: "Primary — RAG, Chat, QA generation" },
            { id: "mlx-community/Qwen3.5-27B-4bit", display_name: "Qwen 3.5 27B", description: "Complex reasoning tasks" },
            { id: "mlx-community/Qwen3.5-9B-MLX-4bit", display_name: "Qwen 3.5 9B", description: "Fast / low latency" },
            { id: "mlx-community/Qwen3-0.6B-4bit", display_name: "Qwen 3 0.6B", description: "Smoke test, ultra-fast" },
            { id: "lmstudio-community/medgemma-4b-it-MLX-4bit", display_name: "MedGemma 4B", description: "Medical domain specialized" },
        ],
    },
    {
        id: "ollama",
        display_name: "Ollama (Local)",
        description: "Run models locally with Ollama",
        requires_api_key: false,
        models: [
            { id: "llama3.2", display_name: "Llama 3.2", description: "Fast and capable, recommended for most tasks" },
            { id: "llama3.1", display_name: "Llama 3.1", description: "Larger context window, good for complex RAG" },
            { id: "mistral", display_name: "Mistral", description: "Efficient and fast" },
            { id: "qwen2.5", display_name: "Qwen 2.5", description: "Strong multilingual support" },
        ],
    },
    {
        id: "google",
        display_name: "Google Gemini (Cloud)",
        description: "Google's Gemini models via API",
        requires_api_key: true,
        models: [
            { id: "gemini-2.0-flash", display_name: "Gemini 2.0 Flash", description: "Fast and efficient, recommended" },
            { id: "gemini-2.5-flash", display_name: "Gemini 2.5 Flash", description: "Latest flash model, best balance" },
            { id: "gemini-2.5-pro", display_name: "Gemini 2.5 Pro", description: "Most capable, large context" },
        ],
    },
];

/// Send a chat message and get a response
export async function sendChat(request: ChatRequest): Promise<ChatResponse> {
    const res = await fetch(`${API_BASE_URL}/agents/chat`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
    });
    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: "Unknown error" }));
        throw new Error(error.error || "Failed to send chat");
    }
    return res.json();
}

/// Update persona configuration (e.g., set default model)
export async function updatePersonaConfig(personaName: string, modelId: string): Promise<any> {
    const res = await authFetch(`${API_BASE_URL}/personas/${personaName}/config`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model_id: modelId }),
    });
    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: "Unknown error" }));
        throw new Error(error.error || "Failed to update persona config");
    }
    return res.json();
}

/// Stream a chat message using SSE
export function streamChat(
    request: ChatRequest,
    onToken: (token: string) => void,
    onDone: (metadata: StreamDone) => void,
    onError: (error: string) => void
): () => void {
    const controller = new AbortController();

    const token = Cookies.get("access_token") || "";
    const tenantId = Cookies.get("tenant_id") || "";
    const query = new URLSearchParams({ token, tenant_id: tenantId }).toString();

    authFetch(`${API_BASE_URL}/agents/chat/stream?${query}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
        signal: controller.signal,
    })
        .then(async (response) => {
            if (!response.ok) {
                const error = await response.json().catch(() => ({ error: "Unknown error" }));
                throw new Error(error.error || "Failed to stream chat");
            }

            const reader = response.body?.getReader();
            if (!reader) throw new Error("No response body");

            const decoder = new TextDecoder();
            let buffer = "";

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });

                // Parse SSE events
                let eventEndIndex = buffer.indexOf("\n\n");

                while (eventEndIndex !== -1) {
                    const eventString = buffer.slice(0, eventEndIndex);
                    buffer = buffer.slice(eventEndIndex + 2); // Consume the processed event and the \n\n

                    const eventLines = eventString.split('\n');
                    let eventData = "";

                    for (const line of eventLines) {
                        if (line.startsWith("data:")) {
                            eventData += line.slice(5).trim();
                        }
                    }

                    if (eventData) {
                        try {
                            const event = JSON.parse(eventData);

                            if (event.token) {
                                onToken(event.token);
                            } else if (event.latency_ms !== undefined) {
                                // Done event
                                onDone({
                                    latency_ms: event.latency_ms,
                                    confidence_score: event.confidence_score,
                                    confidence_level: event.confidence_level,
                                    sources: event.sources,
                                    action: event.action,
                                });
                            } else if (event.error) {
                                onError(event.error);
                            }
                        } catch (e) {
                            console.warn("[API] SSE parse error", e, "Data:", eventData);
                        }
                    }

                    eventEndIndex = buffer.indexOf("\n\n");
                }
            }
        })
        .catch((error) => {
            if (error.name !== "AbortError") {
                onError(error.message);
            }
        });

    // Return cleanup function
    return () => controller.abort();
}

// ─── IAM (User Management) API ─────────────────────────────────────────────

export interface User {
    id: string;
    username: string;
    tenant_id: string | null;
    role: string | null;
    created_at: string | null;
}

export interface Tenant {
    id: string;
    name: string;
    created_at: string | null;
    updated_at: string | null;
}

export interface LlmSlot {
    provider: string;
    model: string;
}

export interface LlmConfig {
    chat?: LlmSlot;
    rag?: LlmSlot;
    pipeline_generator?: LlmSlot;
    pipeline_evaluator?: LlmSlot;
    judge?: LlmSlot;
    embedding?: LlmSlot;
    heimdall_url?: string;
    heimdall_api_key?: string;
    // Cloud provider API keys (migrated from dropped provider_api_keys column)
    openai_api_key?: string;
    google_api_key?: string;
    azure_api_key?: string;
}

export interface TenantConfig {
    tenant_id: string;
    default_provider?: string;
    default_model?: string;
    /** @deprecated Column dropped — keys now stored in llm_config */
    provider_api_keys?: Record<string, any>;
    qa_rules?: Record<string, any>;
    system_prompt?: string;
    max_daily_tokens: number;
    is_dedicated_vector_db: boolean;
    max_crawl_pages?: number;
    search_settings?: {
        embedding_model?: string;
        top_k?: number;
        similarity_threshold?: number;
        search_mode?: string;
    };
    pipeline_settings?: {
        chunk_strategy?: string;
        chunk_size?: number;
        chunk_overlap?: number;
        dedup_threshold?: number;
    };
    llm_config?: LlmConfig;
}

export interface CreateTenantRequest {
    name: string;
    is_dedicated_vector_db: boolean;
    admin_email: string;
    admin_password?: string;
}

export async function fetchUsers(): Promise<User[]> {
    const res = await authFetch(`${API_BASE_URL}/iam/users`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch users");
    return res.json();
}

export async function fetchTenants(): Promise<Tenant[]> {
    const res = await authFetch(`${API_BASE_URL}/tenants`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch tenants");
    return res.json();
}

export async function fetchMyTenants(): Promise<Tenant[]> {
    const res = await authFetch(`${API_BASE_URL}/iam/my-tenants`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch my tenants");
    return res.json();
}

export async function createTenant(data: CreateTenantRequest): Promise<Tenant> {
    const res = await authFetch(`${API_BASE_URL}/iam/tenants`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to create tenant");
    return res.json();
}

export async function deleteTenant(id: string): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/tenants/${id}`, {
        method: "DELETE",
    });
    if (!res.ok) throw new Error("Failed to delete tenant");
}

export async function fetchTenantConfig(id: string): Promise<TenantConfig> {
    const res = await authFetch(`${API_BASE_URL}/iam/tenants/${id}/config`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch tenant config");
    return res.json();
}

export async function updateTenantConfig(id: string, data: Partial<TenantConfig>): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/tenants/${id}/config`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to update tenant config");
}

export async function createUser(data: any): Promise<User> {
    const res = await authFetch(`${API_BASE_URL}/iam/users`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to create user");
    return res.json();
}

export async function updateUserRole(id: string, data: { tenant_id: string; role: string }): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/users/${id}/role`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to update user role");
}

export async function updateUserPassword(id: string, password: string): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/users/${id}/password`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
    });
    if (!res.ok) throw new Error("Failed to update user password");
}

export async function deleteUser(id: string): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/users/${id}`, {
        method: "DELETE",
    });
    if (!res.ok) throw new Error("Failed to delete user");
}

export async function updateTenant(id: string, name: string): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/tenants/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
    });
    if (!res.ok) throw new Error("Failed to update tenant");
}

// ─── Dashboard Stats API ──────────────────────────────────────────────────

export interface SourceHealth {
    healthy: number;
    failed: number;
    pending: number;
    running: number;
}

export interface StatsResponse {
    total_sources: number;
    total_chunks: number;
    qa_pairs: number;
    vector_coverage: number;
    source_health: SourceHealth;
}

export async function fetchStats(): Promise<StatsResponse> {
    const res = await authFetch(`${API_BASE_URL}/stats`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch stats");
    return res.json();
}

export async function syncAllSources(): Promise<any> {
    const res = await authFetch(`${API_BASE_URL}/sources/sync-all`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to sync all sources");
    return res.json();
}

// ─── Chunks API ───────────────────────────────────────────────────────────

export interface ChunkItem {
    id: number;
    source_id: number;
    source_name: string;
    chunk_index: number;
    content: string;
    token_count: number | null;
    metadata_json: any;
    created_at: string | null;
}

export interface ChunkListResponse {
    chunks: ChunkItem[];
    total: number;
    total_tokens: number;
    page: number;
    per_page: number;
}

export async function fetchChunks(params?: {
    source_id?: number;
    search?: string;
    page?: number;
    per_page?: number;
}): Promise<ChunkListResponse> {
    const query = new URLSearchParams();
    if (params?.source_id) query.set("source_id", String(params.source_id));
    if (params?.search) query.set("search", params.search);
    if (params?.page) query.set("page", String(params.page));
    if (params?.per_page) query.set("per_page", String(params.per_page));
    const qs = query.toString();
    const res = await authFetch(`${API_BASE_URL}/chunks${qs ? `?${qs}` : ""}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch chunks");
    return res.json();
}

export async function fetchChunk(id: number): Promise<ChunkItem> {
    const res = await authFetch(`${API_BASE_URL}/chunks/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch chunk");
    return res.json();
}

export interface GenerateQaResponse {
    success: boolean;
    message: string;
    chunk_count: number;
}

export async function generateQaForChunks(chunkIds: number[]): Promise<GenerateQaResponse> {
    if (!chunkIds || chunkIds.length === 0) {
        throw new Error("No chunks selected");
    }
    const res = await authFetch(`${API_BASE_URL}/chunks/generate-qa`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ chunk_ids: chunkIds }),
    });
    if (!res.ok) throw new Error(`Failed to generate QA: ${res.statusText}`);
    return res.json();
}

// ─── Data Sources Ingress API ─────────────────────────────────────────────

export interface DataSource {
    id: number;
    tenant_id: string;
    name: string;
    source_type: 'web' | 'tabular' | 'document' | 'mcp' | 'file';
    config_json: any;
    schedule: string | null;
    last_sync_status: 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | null;
    raw_markdown?: string | null;
    mb_size?: number | null;
    total_chunks?: number | null;
    pageindex_tree?: any | null;
    last_sync_at: string | null;
    created_at: string;
    updated_at: string;
}

export async function fetchSources(): Promise<DataSource[]> {
    const res = await authFetch(`${API_BASE_URL}/sources`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch sources");
    return res.json();
}

export async function createSource(data: Partial<DataSource>): Promise<DataSource> {
    const res = await authFetch(`${API_BASE_URL}/sources`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to create source");
    return res.json();
}

export async function deleteSource(id: number): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/sources/${id}`, {
        method: "DELETE",
    });
    if (!res.ok) throw new Error("Failed to delete source");
}

export async function syncSource(id: number): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/sources/${id}/sync`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to trigger sequence sync");
}

export interface AiExtractResponse {
    content: string;
    tokens_used: number;
    model: string;
}

export async function extractWithAi(
    sourceId: number,
    model: string,
    outputFormat: string
): Promise<AiExtractResponse> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/extract-ai`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model, output_format: outputFormat }),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: "Unknown error" }));
        throw new Error(err.error || "AI extraction failed");
    }
    return res.json();
}

export interface PageIndexResponse {
    success: boolean;
    message: string;
    source_id: number;
}

export async function generatePageIndexTree(
    sourceId: number,
    provider: string = "gemini",
    model: string = "gemini-2.5-flash"
): Promise<PageIndexResponse> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/extract-pageindex`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, model }),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: "Unknown error" }));
        throw new Error(err.error || "PageIndex generation failed");
    }
    return res.json();
}

export async function fetchSource(id: number): Promise<DataSource | null> {
    const sources = await fetchSources();
    return sources.find(s => s.id === id) || null;
}

export async function updateSource(id: number, data: Partial<DataSource>): Promise<DataSource> {
    const res = await authFetch(`${API_BASE_URL}/sources/${id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to update source");
    return res.json();
}

export function uploadFile(
    sourceId: number | null,
    file: File,
    onProgress?: (percent: number) => void,
    metadata?: { name?: string; source_type?: string; folder_path?: string }
): Promise<any> {
    return new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        const formData = new FormData();
        formData.append("file", file);
        if (metadata?.name) formData.append("name", metadata.name);
        if (metadata?.source_type) formData.append("source_type", metadata.source_type);
        if (metadata?.folder_path) formData.append("folder_path", metadata.folder_path);

        xhr.upload.addEventListener("progress", (event) => {
            if (event.lengthComputable && onProgress) {
                onProgress(Math.round((event.loaded / event.total) * 100));
            }
        });

        xhr.addEventListener("load", () => {
            if (xhr.status >= 200 && xhr.status < 300) {
                try {
                    resolve(JSON.parse(xhr.responseText));
                } catch {
                    resolve({ ok: true });
                }
            } else {
                reject(new Error(`Upload failed with status ${xhr.status}`));
            }
        });

        xhr.addEventListener("error", () => reject(new Error("Upload failed")));

        const headers = getAuthHeaders() as Record<string, string>;
        // Use the correct /sources/upload endpoint (not per-source ID)
        xhr.open("POST", `${API_BASE_URL}/sources/upload`);
        Object.entries(headers).forEach(([key, value]) => xhr.setRequestHeader(key, value));
        xhr.send(formData);
    });
}

export interface FeatureFlags {
    ocr_enabled: boolean;
    dicom_enabled: boolean;
    domain: string;
}

export async function getFeatureFlags(domain?: string): Promise<FeatureFlags> {
    const query = domain ? `?domain=${encodeURIComponent(domain)}` : "";
    const res = await authFetch(`${API_BASE_URL}/feature-flags${query}`, { cache: "no-store" });
    if (!res.ok) {
        // Return sensible defaults if the endpoint is not yet available
        return { ocr_enabled: false, dicom_enabled: false, domain: domain || "general" };
    }
    return res.json();
}

// ─── Sprint 12: Web Hierarchy API ─────────────────────────────────────────────

export interface HierarchyNode {
    url: string;
    title: string | null;
    depth: number;
    status: "new" | "updated" | "unchanged" | "duplicate" | "error";
    children: HierarchyNode[];
}

export interface DiscoverHierarchyResponse {
    source_id: number;
    root_url: string;
    total_pages: number;
    pages: HierarchyNode[];
}

export async function discoverHierarchy(
    sourceId: number,
    options?: { max_depth?: number; max_pages?: number }
): Promise<DiscoverHierarchyResponse> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/discover-hierarchy`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(options || {}),
    });
    if (!res.ok) throw new Error("Failed to discover hierarchy");
    return res.json();
}

export async function importPages(
    sourceId: number,
    urls: { url: string; title?: string; depth?: number }[]
): Promise<{ source_id: number; imported: number; skipped: number; total_requested: number }> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/import-pages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ urls }),
    });
    if (!res.ok) throw new Error("Failed to import pages");
    return res.json();
}

// ─── Sprint 12: LLM Usage API ────────────────────────────────────────────────

export interface LlmUsageLog {
    id: number;
    tenant_id: number;
    model_id: string;
    provider: string;
    endpoint: string | null;
    caller: string | null;
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    latency_ms: number;
    status: "success" | "error" | "timeout";
    error_message: string | null;
    created_at: string;
}

export interface LlmUsageSummary {
    total_calls: number;
    total_input_tokens: number;
    total_output_tokens: number;
    total_tokens: number;
    avg_latency_ms: number;
    estimated_cost_usd: number;
    models: {
        model_id: string;
        provider: string;
        total_calls: number;
        total_tokens: number;
        avg_latency_ms: number;
        estimated_cost_usd: number;
    }[];
}

export interface PaginatedUsageLogs {
    logs: LlmUsageLog[];
    total: number;
    page: number;
    per_page: number;
}

export async function fetchLlmUsage(params?: {
    page?: number;
    per_page?: number;
    model_id?: string;
    provider?: string;
    status?: string;
    date_from?: string;
    date_to?: string;
}): Promise<PaginatedUsageLogs> {
    const query = params ? "?" + new URLSearchParams(
        Object.entries(params)
            .filter(([, v]) => v !== undefined)
            .map(([k, v]) => [k, String(v)])
    ).toString() : "";
    const res = await authFetch(`${API_BASE_URL}/llm-usage${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch LLM usage");
    return res.json();
}

export async function fetchLlmUsageSummary(params?: {
    date_from?: string;
    date_to?: string;
}): Promise<LlmUsageSummary> {
    const query = params ? "?" + new URLSearchParams(
        Object.entries(params)
            .filter(([, v]) => v !== undefined)
            .map(([k, v]) => [k, String(v)])
    ).toString() : "";
    const res = await authFetch(`${API_BASE_URL}/llm-usage/summary${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch LLM usage summary");
    return res.json();
}

// ─── Sprint 13: Agent Studio API ─────────────────────────────────────────────

export interface AgentConfig {
    id: number;
    tenant_id: string;
    name: string;
    display_name?: string;
    description?: string;
    system_prompt: string;
    model_id: string;
    provider: string;
    temperature?: number;
    max_tokens?: number;
    top_k?: number;
    use_rag?: boolean;
    use_knowledge_graph?: boolean;
    tools?: string[];
    personality_traits?: string[];
    greeting?: string;
    avatar_url?: string;
    template_id?: string;
    is_published?: boolean;
    api_key?: string;
    tier?: number;
    response_mode?: string;
    created_at?: string;
    updated_at?: string;
}

export interface CreateAgentRequest {
    name: string;
    display_name?: string;
    description?: string;
    system_prompt: string;
    model_id: string;
    provider?: string;
    temperature?: number;
    max_tokens?: number;
    top_k?: number;
    use_rag?: boolean;
    use_knowledge_graph?: boolean;
    tools?: string[];
    personality_traits?: string[];
    greeting?: string;
    avatar_url?: string;
    template_id?: string;
    tier?: number;
    response_mode?: string;
}

export interface AgentTemplate {
    id: string;
    name: string;
    display_name: string;
    description: string;
    system_prompt: string;
    model_id: string;
    provider: string;
    temperature: number;
    max_tokens: number;
    use_rag: boolean;
    use_knowledge_graph: boolean;
    tools: string[];
    personality_traits: string[];
    greeting: string;
    tier?: number;
    avatar_url?: string;
}

export interface AgentChatResponse {
    content: string;
    session_id: string;
    model_id: string;
    provider: string;
    latency_ms: number;
    input_tokens: number;
    output_tokens: number;
    confidence_score?: number;
}

export interface ConversationSession {
    session_id: string;
    agent_config_id?: number;
    agent_name?: string;
    message_count: number;
    first_message_at?: string;
    last_message_at?: string;
}

export interface ConversationMessage {
    id: number;
    session_id: string;
    role: string;
    content: string;
    model_id?: string;
    latency_ms?: number;
    input_tokens?: number;
    output_tokens?: number;
    feedback?: string;
    created_at?: string;
}

export interface UsageAlert {
    alert_type: string;
    model_id: string;
    message: string;
    severity: string;
    current_value: number;
    threshold: number;
}

export interface BudgetConfig {
    id: number;
    model_id: string;
    daily_token_limit: number;
    alert_threshold_pct: number;
}

export interface BenchmarkEntry {
    model_id: string;
    provider: string;
    total_calls: number;
    success_rate: number;
    avg_latency_ms: number;
    p50_latency_ms: number;
    p95_latency_ms: number;
    avg_tokens_per_call: number;
    total_tokens: number;
    estimated_cost: number;
}

// Agent CRUD (fetchAgents is defined above near AgentConfigResponse)


export async function createAgent(data: CreateAgentRequest): Promise<AgentConfig> {
    const res = await authFetch(`${API_BASE_URL}/agents`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || "Failed to create agent");
    }
    return res.json();
}

export async function getAgent(id: number): Promise<AgentConfig> {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch agent");
    return res.json();
}

export async function updateAgent(id: number, data: Partial<CreateAgentRequest>): Promise<AgentConfig> {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to update agent");
    return res.json();
}

export async function deleteAgent(id: number) {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}`, { method: "DELETE" });
    if (!res.ok) throw new Error("Failed to delete agent");
}

export async function publishAgent(id: number) {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}/publish`, { method: "POST" });
    if (!res.ok) throw new Error("Failed to publish agent");
    return res.json();
}

export async function agentChat(id: number, message: string, sessionId?: string): Promise<AgentChatResponse> {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}/chat`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message, session_id: sessionId }),
    });
    if (!res.ok) throw new Error("Failed to send message");
    return res.json();
}

export async function fetchAgentConversations(id: number, page = 1) {
    const res = await authFetch(`${API_BASE_URL}/agents/${id}/conversations?page=${page}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch conversations");
    return res.json();
}

export async function fetchTemplates(): Promise<AgentTemplate[]> {
    const res = await authFetch(`${API_BASE_URL}/agents/templates`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch templates");
    return res.json();
}

// Conversations

export async function fetchConversations(params?: {
    agent_config_id?: number;
    page?: number;
    per_page?: number;
}) {
    const query = params ? "?" + new URLSearchParams(
        Object.entries(params)
            .filter(([, v]) => v !== undefined)
            .map(([k, v]) => [k, String(v)])
    ).toString() : "";
    const res = await authFetch(`${API_BASE_URL}/conversations${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch conversations");
    return res.json();
}

export async function getConversation(sessionId: string) {
    const res = await authFetch(`${API_BASE_URL}/conversations/${sessionId}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch conversation");
    return res.json();
}

export async function submitFeedback(messageId: number, feedback: "thumbs_up" | "thumbs_down") {
    const res = await authFetch(`${API_BASE_URL}/conversations/${messageId}/feedback`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ feedback }),
    });
    if (!res.ok) throw new Error("Failed to submit feedback");
    return res.json();
}

export async function fetchConversationStats() {
    const res = await authFetch(`${API_BASE_URL}/conversations/stats`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch stats");
    return res.json();
}

// Evaluations

export async function runEvaluation(data: {
    models: string[];
    questions: { question: string; expected_answer?: string }[];
    agent_name?: string;
    agent_config_id?: number;
    judge_model?: string;
}) {
    const res = await authFetch(`${API_BASE_URL}/evaluations/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to run evaluation");
    return res.json();
}

export async function getEvalResults(params?: { batch_id?: string; model_id?: string; page?: number }) {
    const query = params ? "?" + new URLSearchParams(
        Object.entries(params)
            .filter(([, v]) => v !== undefined)
            .map(([k, v]) => [k, String(v)])
    ).toString() : "";
    const res = await authFetch(`${API_BASE_URL}/evaluations/results${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch results");
    return res.json();
}

export async function compareModels(modelA: string, modelB: string, batchId?: string) {
    const params = new URLSearchParams({ model_a: modelA, model_b: modelB });
    if (batchId) params.set("batch_id", batchId);
    const res = await authFetch(`${API_BASE_URL}/evaluations/compare?${params}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to compare models");
    return res.json();
}

export async function getFeedbackSummary() {
    const res = await authFetch(`${API_BASE_URL}/evaluations/feedback-summary`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch feedback summary");
    return res.json();
}

// Budget & Alerts

export async function getBudgetConfig(): Promise<BudgetConfig[]> {
    const res = await authFetch(`${API_BASE_URL}/settings/llm-budget`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch budget config");
    return res.json();
}

export async function saveBudgetConfig(budgets: { model_id: string; daily_token_limit: number; alert_threshold_pct?: number }[]) {
    const res = await authFetch(`${API_BASE_URL}/settings/llm-budget`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ budgets }),
    });
    if (!res.ok) throw new Error("Failed to save budget config");
    return res.json();
}

export async function getAlerts(): Promise<UsageAlert[]> {
    const res = await authFetch(`${API_BASE_URL}/llm-usage/alerts`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch alerts");
    return res.json();
}

export async function getBenchmark(): Promise<BenchmarkEntry[]> {
    const res = await authFetch(`${API_BASE_URL}/llm-usage/benchmark`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch benchmark");
    return res.json();
}

// ─── Cron Worker API (Sprint 14 — #150) ─────────────────────────────────────

export interface CronStatus {
    running: boolean;
    tick_count: number;
    sources_refreshed: number;
    last_tick_at: string | null;
}

export async function fetchCronStatus(): Promise<CronStatus> {
    const res = await authFetch(`${API_BASE_URL}/cron/status`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch cron status");
    return res.json();
}

// ─── DB Connector API (Sprint 14 — #152) ────────────────────────────────────

export interface DbConnectionConfig {
    name: string;
    db_type: "mysql" | "postgres" | "sqlite";
    connection_string: string;
}

export interface DbTestResult {
    success: boolean;
    version?: string;
    error?: string;
}

export interface DbTableSchema {
    table_name: string;
    columns: { name: string; data_type: string }[];
}

export async function testDbConnection(config: DbConnectionConfig): Promise<DbTestResult> {
    const res = await authFetch(`${API_BASE_URL}/db-connector/test-connection`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
    });
    return res.json();
}

export async function discoverDbSchema(config: DbConnectionConfig): Promise<{ tables: DbTableSchema[] }> {
    const res = await authFetch(`${API_BASE_URL}/db-connector/discover-schema`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
    });
    if (!res.ok) throw new Error("Failed to discover schema");
    return res.json();
}

export async function importDbData(config: DbConnectionConfig & { query: string }): Promise<{ markdown: string; row_count: number }> {
    const res = await authFetch(`${API_BASE_URL}/db-connector/import`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
    });
    if (!res.ok) throw new Error("Failed to import data");
    return res.json();
}

// ─── Feedback API (Sprint 14 — #153) ────────────────────────────────────────

export interface FeedbackRequest {
    report_type: "bug" | "feedback" | "feature";
    title: string;
    description?: string;
    priority?: "critical" | "high" | "medium" | "low";
    page_url?: string;
    browser_info?: Record<string, unknown>;
    client_logs?: unknown;
}

export interface FeedbackReport {
    id: number;
    report_type: string;
    title: string;
    description?: string;
    status: string;
    priority?: string;
    github_issue_url?: string;
    created_at: string;
}

export async function submitFeedbackReport(data: FeedbackRequest): Promise<{ feedback_id: number; github_issue_url?: string }> {
    const res = await authFetch(`${API_BASE_URL}/feedback`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to submit feedback");
    return res.json();
}

export async function fetchFeedbackList(params?: { status?: string; page?: number }): Promise<FeedbackReport[]> {
    const query = new URLSearchParams();
    if (params?.status) query.set("status", params.status);
    if (params?.page) query.set("page", String(params.page));
    const qs = query.toString();
    const res = await authFetch(`${API_BASE_URL}/feedback${qs ? `?${qs}` : ""}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch feedback");
    return res.json();
}

// ─── Vault API (Sprint 14 — #157, Sprint 20 — #190) ─────────────────────────

export interface VaultStatus {
    enabled: boolean;
    addr?: string;
    connected: boolean;
    version?: string;
    sealed?: boolean;
}

export async function fetchVaultStatus(): Promise<VaultStatus> {
    const res = await authFetch(`${API_BASE_URL}/vault/status`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch vault status");
    return res.json();
}

export interface VaultSecretInfo {
    key: string;
    status: "present" | "missing";
    source: "vault" | "env" | "none";
    masked_value: string | null;
}

export interface VaultSecretsResponse {
    secrets: VaultSecretInfo[];
    total: number;
    present_count: number;
    vault_enabled: boolean;
}

export async function fetchVaultSecrets(): Promise<VaultSecretsResponse> {
    const res = await authFetch(`${API_BASE_URL}/vault/secrets`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch vault secrets");
    return res.json();
}

export async function rotateVaultSecret(key: string, newValue: string): Promise<any> {
    const res = await authFetch(`${API_BASE_URL}/vault/rotate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key, new_value: newValue }),
    });
    if (!res.ok) throw new Error("Failed to rotate secret");
    return res.json();
}

// ─── Custom Roles API (Issue #191) ──────────────────────────────────────────

export interface Role {
    id: string;
    tenant_id: string;
    name: string;
    is_builtin: boolean;
    permissions: Record<string, string>;
    created_at?: string;
    updated_at?: string;
}

export interface CreateRoleRequest {
    name: string;
    permissions: Record<string, string>;
}

export interface UpdateRoleRequest {
    permissions: Record<string, string>;
}

export async function fetchRoles(): Promise<Role[]> {
    const res = await authFetch(`${API_BASE_URL}/iam/roles`);
    if (!res.ok) throw new Error("Failed to fetch roles");
    return res.json();
}

export async function createRole(req: CreateRoleRequest): Promise<Role> {
    const res = await authFetch(`${API_BASE_URL}/iam/roles`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(req),
    });
    if (!res.ok) {
        if (res.status === 409) throw new Error("Role name already exists");
        throw new Error("Failed to create role");
    }
    return res.json();
}

export async function updateRole(roleId: string, req: UpdateRoleRequest): Promise<Role> {
    const res = await authFetch(`${API_BASE_URL}/iam/roles/${roleId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(req),
    });
    if (!res.ok) {
        if (res.status === 403) throw new Error("Cannot modify built-in role");
        throw new Error("Failed to update role");
    }
    return res.json();
}

export async function deleteRole(roleId: string): Promise<void> {
    const res = await authFetch(`${API_BASE_URL}/iam/roles/${roleId}`, {
        method: "DELETE",
    });
    if (!res.ok) {
        if (res.status === 403) throw new Error("Cannot delete built-in role");
        if (res.status === 400) throw new Error("Role is in use");
        throw new Error("Failed to delete role");
    }
}

// ─── Knowledge Graph API (Sprint 17) ────────────────────────────────────────

export interface GraphEntity {
    id: number;
    name: string;
    entity_type: string;
    properties?: Record<string, unknown>;
    source_id?: number;
    chunk_id?: number;
    neo4j_node_id?: string;
    color?: string;
}

export interface GraphRelation {
    id: number;
    from_entity: string;
    to_entity: string;
    relation_type: string;
    properties?: Record<string, unknown>;
}

export interface GraphStats {
    total_entities: number;
    total_relations: number;
    entities_by_type: { type: string; count: number }[];
    relations_by_type: { type: string; count: number }[];
}

export interface GraphVisualizationData {
    nodes: VisualizationNode[];
    edges: VisualizationEdge[];
    total_nodes: number;
    total_edges: number;
}

export interface VisualizationNode {
    id: string;
    label: string;
    entity_type: string;
    color: string;
    size: number;
}

export interface VisualizationEdge {
    id: string;
    source: string;
    target: string;
    label: string;
}

export interface PathResult {
    found: boolean;
    paths: { steps: { from: string; to: string; relation_type: string }[]; length: number }[];
    message?: string;
}

export interface ExtractionRun {
    id: number;
    source_id: number;
    status: string;
    entities_found: number;
    relations_found: number;
    chunks_processed: number;
    started_at: string;
    completed_at?: string;
    error_message?: string;
}

export async function fetchGraphStats(): Promise<GraphStats> {
    const res = await authFetch(`${API_BASE_URL}/graph/stats`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch graph stats");
    return res.json();
}

export async function searchGraphEntities(params?: {
    q?: string;
    type?: string;
    limit?: number;
    page?: number;
}): Promise<{ entities: GraphEntity[]; total: number; page: number; limit: number }> {
    const query = new URLSearchParams();
    if (params?.q) query.set("q", params.q);
    if (params?.type) query.set("type", params.type);
    if (params?.limit) query.set("limit", String(params.limit));
    if (params?.page) query.set("page", String(params.page));
    const qs = query.toString();
    const res = await authFetch(`${API_BASE_URL}/graph/entities${qs ? `?${qs}` : ""}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to search graph entities");
    return res.json();
}

export async function fetchEntityNeighbors(entityId: number, depth?: number): Promise<{
    center: { name: string; entity_type: string };
    nodes: VisualizationNode[];
    edges: VisualizationEdge[];
}> {
    const query = depth ? `?depth=${depth}` : "";
    const res = await authFetch(`${API_BASE_URL}/graph/entity/${entityId}/neighbors${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch entity neighbors");
    return res.json();
}

export async function fetchGraphVisualization(params?: {
    limit?: number;
    type?: string;
}): Promise<GraphVisualizationData> {
    const query = new URLSearchParams();
    if (params?.limit) query.set("limit", String(params.limit));
    if (params?.type) query.set("type", params.type);
    const qs = query.toString();
    const res = await authFetch(`${API_BASE_URL}/graph/visualization${qs ? `?${qs}` : ""}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch graph visualization data");
    return res.json();
}

export async function findGraphPaths(from: string, to: string, depth?: number): Promise<PathResult> {
    const query = new URLSearchParams({ from, to });
    if (depth) query.set("depth", String(depth));
    const res = await authFetch(`${API_BASE_URL}/graph/paths?${query}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to find graph paths");
    return res.json();
}

export async function triggerKgExtraction(data: {
    source_id?: number;
    text?: string;
    max_entities?: number;
}): Promise<any> {
    const res = await authFetch(`${API_BASE_URL}/graph/extract`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to trigger KG extraction");
    return res.json();
}

export async function deleteGraphSource(sourceId: number): Promise<{
    deleted_entities: number;
    deleted_relations: number;
}> {
    const res = await authFetch(`${API_BASE_URL}/graph/source/${sourceId}`, {
        method: "DELETE",
    });
    if (!res.ok) throw new Error("Failed to delete graph source entities");
    return res.json();
}

export async function fetchExtractionRuns(): Promise<{ runs: ExtractionRun[] }> {
    const res = await authFetch(`${API_BASE_URL}/graph/extraction-runs`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch extraction runs");
    return res.json();
}

/** Convenience wrapper: trigger KG extraction for a specific source */
export async function triggerGraphExtraction(sourceId: number): Promise<{ status: string; run_id: number; source_id: number; message: string }> {
    return triggerKgExtraction({ source_id: sourceId });
}

/** POST /api/v1/sources/{id}/auto-pipeline — Run full 5-step pipeline */
export async function runAutoPipeline(sourceId: number, options?: {
    provider?: string;
    model?: string;
    enablePageIndex?: boolean;
    skipKg?: boolean;
}): Promise<{ pipeline_run_id: string; source_id: number; status: string; message: string }> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/auto-pipeline`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            provider: options?.provider || "gemini",
            model: options?.model || "gemini-2.5-flash",
            enable_pageindex: options?.enablePageIndex,
            skip_kg: options?.skipKg,
        }),
    });
    if (!res.ok) throw new Error("Failed to start auto-pipeline");
    return res.json();
}

// ─── Sprint 18: Coverage Analytics API ──────────────────────────────────────

export interface PipelineStages {
    ingested: number;
    chunked: number;
    qa_generated: number;
    vectorized: number;
    kg_extracted: number;
}

export interface CoverageOverview {
    total_sources: number;
    sources_with_chunks: number;
    sources_with_qa: number;
    sources_with_vectors: number;
    sources_with_kg: number;
    overall_score: number;
    pipeline_stages: PipelineStages;
}

export interface SourceCoverage {
    source_id: number;
    name: string;
    source_type: string;
    status: string;
    chunk_count: number;
    qa_count: number;
    vector_coverage_pct: number;
    kg_entity_count: number;
    dedup_ratio: number;
    blindspots: string[];
    coverage_score: number;
    last_sync_at: string | null;
}

export interface GapSource {
    source_id: number;
    name: string;
}

export interface CoverageGaps {
    sources_missing_chunks: GapSource[];
    sources_missing_qa: GapSource[];
    sources_missing_vectors: GapSource[];
    sources_missing_kg: GapSource[];
    stale_sources: GapSource[];
    high_dedup_sources: GapSource[];
}

export async function fetchCoverageOverview(): Promise<CoverageOverview> {
    const res = await authFetch(`${API_BASE_URL}/coverage/overview`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch coverage overview");
    return res.json();
}

export async function fetchCoverageSources(): Promise<SourceCoverage[]> {
    const res = await authFetch(`${API_BASE_URL}/coverage/sources`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch coverage sources");
    return res.json();
}

export async function fetchCoverageGaps(): Promise<CoverageGaps> {
    const res = await authFetch(`${API_BASE_URL}/coverage/gaps`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch coverage gaps");
    return res.json();
}


export async function fetchPipelineStatus(sourceId: number): Promise<any> {
    const res = await authFetch(`${API_BASE_URL}/sources/${sourceId}/pipeline-status`);
    if (!res.ok) throw new Error("Failed to fetch pipeline status");
    return res.json();
}
