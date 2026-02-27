import { PipelineRun, RunDetails, QAResult, EvaluationReport } from "@/types/pipeline";
import Cookies from "js-cookie";

const API_BASE_URL = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api") + "/v1";

function getAuthHeaders(): HeadersInit {
    const token = Cookies.get("access_token");
    const tenantId = Cookies.get("tenant_id");

    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    if (tenantId) headers["X-Tenant-Id"] = tenantId;

    return headers;
}

// Custom fetch wrapper to auto-add auth headers
async function authFetch(url: string, options: RequestInit = {}): Promise<Response> {
    const headers = {
        ...getAuthHeaders(),
        ...(options.headers || {})
    };

    const res = await fetch(url, { ...options, headers });

    if (res.status === 401) {
        // Handle unauthorized (clear cookies, maybe redirect to login via window.location if browser)
        if (typeof window !== "undefined") {
            Cookies.remove("access_token");
            window.location.href = "/login";
        }
    }

    return res;
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

/// Available personas (static list matching backend configs)
export const PERSONAS: Persona[] = [
    {
        name: "Mimir",
        display_name: "Mimir The Guide",
        tier: 1,
        description: "All-knowing guide capable of actions",
        greeting: "สวัสดีนักผจญภัย ข้าคือ Mimir ผู้รอบรู้แห่ง Yggdrasil ข้าสามารถช่วยตอบคำถามพื้นฐาน และช่วยเหลือท่านด้วยคำสั่งต่างๆ (Action) ได้\n\n**ตัวอย่างคำถามที่ท่านสามารถทดสอบได้:**\n- `ช่วย Heal ฉันหน่อย`\n- `ขอรับบัพ Agi หน่อย`\n- `พาฉันกลับเมือง Prontera ที`",
        avatar_url: "/avatars/mimir.png",
        traits: ["helpful", "wise", "concise"],
    },
    {
        name: "sage_ariel",
        display_name: "Sage Ariel",
        tier: 2,
        description: "Scholar who explains in detail",
        greeting: "ยินดีต้อนรับสู่หอสมุดแห่ง Prontera ข้าคือ Sage Ariel ผู้รวมรวบความรู้แห่ง Midgard ข้าสามารถค้นหาข้อมูลจากเอกสารวิกิ (RAG) มาตอบท่านได้อย่างละเอียด\n\n**ลองสอบถามข้าดูสิ:**\n- `มอนสเตอร์ Baphomet อาศัยอยู่ที่ไหน?`\n- `ดาบ Excalibur ดรอปจากตัวอะไร?`\n- `เล่าประวัติศาสตร์ของเมือง Glast Heim ให้ฟังหน่อย`",
        avatar_url: "/avatars/sage_ariel.png",
        traits: ["wise", "calm", "helpful", "scholarly", "thorough"],
    },
    {
        name: "fortune_teller",
        display_name: "Fortune Teller Maya",
        tier: 2,
        description: "Mysterious seer, speaks in riddles",
        greeting: "ดวงดาวได้ทำนายการมาเยือนของท่าน... ข้าคือ Maya ผู้มองเห็นอนาคตผ่านหน้าไพ่ทาโรต์\n\n**ลองให้ข้าทำนายดูสิ:**\n- `ขอทราบนิสัยและจุดอ่อนของบอส Dark Lord`\n- `มีแผนที่ไหนดรอปการ์ดดีๆ บ้าง?`",
        avatar_url: "/avatars/fortune_teller.png",
        traits: ["mysterious", "cryptic", "enigmatic", "prophetic"],
    },
    {
        name: "blacksmith",
        display_name: "Blacksmith Grumm",
        tier: 2,
        description: "Gruff dwarf, speaks plainly",
        greeting: "หืม? มีธุระอะไรก็ว่ามา ข้าคือ Grumm ช่างตีเหล็กมือหนึ่ง ถนัดเรื่องอาวุธชุดเกราะ\n\n**อยากรู้เรื่องการคราฟหรืออุปกรณ์หรอ? ถามมาสิ:**\n- `ดาบธาตุไฟ คราฟยังไงใช้อะไรบ้าง?`\n- `เกราะแบบไหนป้องกันเวทย์ได้ดีที่สุด?`",
        avatar_url: "/avatars/blacksmith.png",
        traits: ["gruff", "straightforward", "practical", "knowledgeable"],
    },
];

/// Fallback providers when database is not available
export const PROVIDERS: LlmProvider[] = [
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

export interface TenantConfig {
    tenant_id: string;
    default_provider?: string;
    default_model?: string;
    provider_api_keys?: Record<string, any>;
    qa_rules?: Record<string, any>;
    system_prompt?: string;
    max_daily_tokens: number;
    is_dedicated_vector_db: boolean;
    search_settings?: {
        embedding_model?: string;
        top_k?: number;
        similarity_threshold?: number;
        search_mode?: string;
    };
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
    const res = await authFetch(`${API_BASE_URL}/iam/tenants`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch tenants");
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
    sourceId: number,
    file: File,
    onProgress?: (percent: number) => void
): Promise<any> {
    return new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        const formData = new FormData();
        formData.append("file", file);

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
        xhr.open("POST", `${API_BASE_URL}/sources/${sourceId}/upload`);
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
