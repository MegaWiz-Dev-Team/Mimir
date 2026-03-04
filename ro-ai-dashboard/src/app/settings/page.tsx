"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
    Building2, Save, Settings2, Bot, Workflow, Share2,
    Search, Shield, Lock, Users, Layers, Plus, Trash2, RefreshCw
} from "lucide-react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import Cookies from "js-cookie";

import { fetchTenants, updateTenant, fetchTenantConfig, updateTenantConfig, createTenant, deleteTenant, fetchUsers, createUser, deleteUser, updateUserRole, Tenant, TenantConfig, LlmConfig, LlmSlot, User, CreateTenantRequest } from "@/lib/api";

// ─── Tab Definitions ────────────────────────────────────────────────────────────

const TABS = [
    { id: "general", label: "General", icon: Building2 },
    { id: "ai-models", label: "AI Models", icon: Bot },
    { id: "pipeline", label: "Pipeline", icon: Workflow },
    { id: "knowledge-graph", label: "Knowledge Graph", icon: Share2 },
    { id: "search", label: "Search", icon: Search },
    { id: "security", label: "Security", icon: Shield },
    { id: "tenants", label: "Tenants", icon: Layers },
    { id: "users", label: "Users", icon: Users },
] as const;

type TabId = typeof TABS[number]["id"];

// ─── Main Component ─────────────────────────────────────────────────────────────

export default function SettingsPage() {
    const [activeTab, setActiveTab] = useState<TabId>("general");
    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [config, setConfig] = useState<TenantConfig | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [tenantName, setTenantName] = useState("");
    const [currentTenantId, setCurrentTenantId] = useState<string | null>(null);

    // Pipeline settings (local state until backend is wired)
    const [chunkStrategy, setChunkStrategy] = useState("auto");
    const [chunkSize, setChunkSize] = useState(512);
    const [chunkOverlap, setChunkOverlap] = useState(50);
    const [dedupThreshold, setDedupThreshold] = useState(0);

    // Search settings (local state)
    const [embeddingModel, setEmbeddingModel] = useState("nomic-embed-text");
    const [topK, setTopK] = useState(5);
    const [similarityThreshold, setSimilarityThreshold] = useState(0.7);
    const [searchMode, setSearchMode] = useState("hybrid");

    useEffect(() => {
        loadData();
    }, []);

    // Tenants & Users state
    const [allTenants, setAllTenants] = useState<Tenant[]>([]);
    const [allUsers, setAllUsers] = useState<User[]>([]);
    const [showCreateTenantDialog, setShowCreateTenantDialog] = useState(false);
    const [showCreateUserDialog, setShowCreateUserDialog] = useState(false);
    const [newTenant, setNewTenant] = useState({ name: "", admin_email: "", admin_password: "", is_dedicated_vector_db: false });
    const [newUser, setNewUser] = useState({ username: "", password: "", tenant_id: "", role: "viewer" });

    const loadData = async () => {
        setIsLoading(true);
        try {
            const tenantsData = await fetchTenants();
            setTenants(tenantsData);
            setAllTenants(tenantsData);

            if (tenantsData.length > 0) {
                // Use active tenant from cookie, fallback to first tenant
                const activeTenantId = Cookies.get("tenant_id") || tenantsData[0].id;
                const activeTenant = tenantsData.find(t => t.id === activeTenantId) || tenantsData[0];
                setCurrentTenantId(activeTenant.id);
                setTenantName(activeTenant.name);

                try {
                    const configData = await fetchTenantConfig(activeTenant.id);
                    setConfig(configData);
                    // Initialize search settings from config
                    if (configData.search_settings) {
                        setEmbeddingModel(configData.search_settings.embedding_model || "nomic-embed-text");
                        setTopK(configData.search_settings.top_k || 5);
                        setSimilarityThreshold(configData.search_settings.similarity_threshold || 0.7);
                        setSearchMode(configData.search_settings.search_mode || "hybrid");
                    }
                } catch (err) {
                    console.warn("[Settings] Failed to load tenant config:", err);
                }
            }

            // Load users
            try {
                const usersData = await fetchUsers();
                setAllUsers(usersData);
            } catch (err) {
                console.warn("[Settings] Failed to load users:", err);
            }
        } catch (error) {
            console.warn("[Settings] Failed to load tenants:", error);
        } finally {
            setIsLoading(false);
        }
    };

    const handleSave = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!currentTenantId) return;

        setIsSaving(true);
        try {
            await updateTenant(currentTenantId, tenantName);
            if (config) {
                await updateTenantConfig(currentTenantId, config);
            }
            alert("Settings updated successfully.");
            loadData();
        } catch (error) {
            console.warn("[Settings]", error);
            alert("Failed to update settings.");
        } finally {
            setIsSaving(false);
        }
    };

    // ─── Tab Content Renderers ──────────────────────────────────────────────────

    const renderGeneralTab = () => (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Building2 className="w-5 h-5 text-primary" />
                    Tenant Configuration
                </CardTitle>
                <CardDescription>
                    Update the name of your organization/tenant. This name is displayed across the platform.
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading settings...</div>
                ) : currentTenantId ? (
                    <form onSubmit={handleSave} className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Tenant Name</label>
                            <Input
                                required
                                placeholder="Enter tenant name"
                                value={tenantName}
                                onChange={e => setTenantName(e.target.value)}
                            />
                        </div>

                        <div className="space-y-2">
                            <label className="text-sm font-medium text-muted-foreground">Tenant ID</label>
                            <Input
                                disabled
                                value={currentTenantId}
                                className="bg-muted cursor-not-allowed font-mono text-xs"
                            />
                            <p className="text-xs text-muted-foreground">The internal ID cannot be modified.</p>
                        </div>

                        <div className="pt-4 flex justify-end">
                            <Button type="submit" disabled={isSaving || !tenantName.trim()}>
                                <Save className="w-4 h-4 mr-2" />
                                {isSaving ? "Saving..." : "Save Changes"}
                            </Button>
                        </div>
                    </form>
                ) : (
                    <div className="py-4 text-center text-muted-foreground">No tenant data found.</div>
                )}
            </CardContent>
        </Card>
    );

    const PROVIDER_OPTIONS = [
        { value: "ollama", label: "Ollama (Local)" },
        { value: "heimdall", label: "Heimdall (Self-Hosted)" },
        { value: "gemini", label: "Google Gemini" },
    ] as const;

    const MODEL_OPTIONS: Record<string, { value: string; label: string }[]> = {
        ollama: [
            { value: "llama3.2", label: "llama3.2" },
            { value: "llama3.1", label: "llama3.1" },
            { value: "qwen2.5", label: "qwen2.5" },
            { value: "qwen2.5:32b", label: "qwen2.5:32b" },
        ],
        heimdall: [
            { value: "mlx-community/Qwen3.5-35B-A3B-4bit", label: "Qwen 3.5 35B MoE" },
            { value: "mlx-community/Qwen3.5-27B-4bit", label: "Qwen 3.5 27B" },
            { value: "mlx-community/Qwen3.5-9B-MLX-4bit", label: "Qwen 3.5 9B" },
            { value: "mlx-community/Qwen3-0.6B-4bit", label: "Qwen 3 0.6B" },
            { value: "lmstudio-community/medgemma-4b-it-MLX-4bit", label: "MedGemma 4B" },
        ],
        gemini: [
            { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
            { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
            { value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash Lite" },
        ],
    };

    const EMBEDDING_MODEL_OPTIONS: Record<string, { value: string; label: string }[]> = {
        ollama: [
            { value: "nomic-embed-text", label: "nomic-embed-text" },
            { value: "bge-m3", label: "bge-m3" },
        ],
        openai: [
            { value: "text-embedding-3-small", label: "text-embedding-3-small" },
            { value: "text-embedding-3-large", label: "text-embedding-3-large" },
        ],
        google: [
            { value: "text-embedding-004", label: "text-embedding-004" },
        ],
    };

    const selectClass = "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

    const updateSlot = (slotName: keyof LlmConfig, field: "provider" | "model", value: string) => {
        if (!config) return;
        const current = config.llm_config || {};
        const currentSlot = (current[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
        const updatedSlot = { ...currentSlot, [field]: value };
        // Auto-select first model when provider changes
        if (field === "provider") {
            const isEmbedding = slotName === "embedding";
            const models = isEmbedding ? (EMBEDDING_MODEL_OPTIONS[value] || []) : (MODEL_OPTIONS[value] || []);
            updatedSlot.model = models[0]?.value || "";
        }
        setConfig({ ...config, llm_config: { ...current, [slotName]: updatedSlot } });
    };

    const SlotCard = ({ slotName, icon, title, desc }: { slotName: keyof LlmConfig; icon: string; title: string; desc: string }) => {
        const slot = (config?.llm_config?.[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
        const isEmbedding = slotName === "embedding";
        const providerModels = isEmbedding ? (EMBEDDING_MODEL_OPTIONS[slot.provider] || []) : (MODEL_OPTIONS[slot.provider] || []);
        return (
            <div className="rounded-lg border bg-card p-4 space-y-3">
                <div className="flex items-center gap-2">
                    <span className="text-lg">{icon}</span>
                    <div>
                        <h4 className="font-medium text-sm">{title}</h4>
                        <p className="text-xs text-muted-foreground">{desc}</p>
                    </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">Provider</label>
                        <select className={selectClass} value={slot.provider}
                            onChange={e => updateSlot(slotName, "provider", e.target.value)}>
                            <option value="">Select...</option>
                            {(isEmbedding ? [
                                { value: "ollama", label: "Ollama" },
                                { value: "openai", label: "OpenAI" },
                                { value: "google", label: "Google" },
                            ] : PROVIDER_OPTIONS).map(p => (
                                <option key={p.value} value={p.value}>{p.label}</option>
                            ))}
                        </select>
                    </div>
                    <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">Model</label>
                        <select className={selectClass} value={slot.model}
                            onChange={e => updateSlot(slotName, "model", e.target.value)}>
                            <option value="">Select...</option>
                            {providerModels.map(m => (
                                <option key={m.value} value={m.value}>{m.label}</option>
                            ))}
                        </select>
                    </div>
                </div>
            </div>
        );
    };

    const renderAIModelsTab = () => (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Settings2 className="w-5 h-5 text-primary" />
                    AI Model Configuration
                </CardTitle>
                <CardDescription>
                    Configure models for each purpose. Each slot can use a different provider and model.
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading...</div>
                ) : config ? (
                    <form onSubmit={handleSave} className="space-y-6">
                        {/* Per-Purpose Slot Cards */}
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <SlotCard slotName="chat" icon="💬" title="Chat & NPC" desc="Agent chat (Tier 1+2)" />
                            <SlotCard slotName="rag" icon="📚" title="RAG (Oracle Agent)" desc="Knowledge retrieval queries" />
                            <SlotCard slotName="pipeline_generator" icon="🔄" title="Pipeline Generator" desc="QA pair generation" />
                            <SlotCard slotName="judge" icon="⚖️" title="Evaluation Judge" desc="LLM-as-Judge scoring" />
                            <SlotCard slotName="embedding" icon="🧬" title="Embedding" desc="Vector embedding model" />
                        </div>

                        {/* Heimdall Gateway */}
                        <div className="rounded-lg border bg-card p-4 space-y-3">
                            <div className="flex items-center gap-2">
                                <span className="text-lg">🔗</span>
                                <div>
                                    <h4 className="font-medium text-sm">Heimdall Gateway</h4>
                                    <p className="text-xs text-muted-foreground">Self-hosted LLM gateway connection</p>
                                </div>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                <div className="space-y-1">
                                    <label className="text-xs text-muted-foreground">URL</label>
                                    <Input
                                        placeholder="https://...ngrok-free.dev/v1"
                                        value={config.llm_config?.heimdall_url || ""}
                                        onChange={e => setConfig({ ...config, llm_config: { ...config.llm_config, heimdall_url: e.target.value } })}
                                    />
                                </div>
                                <div className="space-y-1">
                                    <label className="text-xs text-muted-foreground flex items-center gap-1">
                                        <Lock className="w-3 h-3" /> API Key
                                    </label>
                                    <Input
                                        type="password"
                                        placeholder="••••••••"
                                        value={config.llm_config?.heimdall_api_key || ""}
                                        onChange={e => setConfig({ ...config, llm_config: { ...config.llm_config, heimdall_api_key: e.target.value } })}
                                    />
                                </div>
                            </div>
                        </div>

                        {/* System Prompt */}
                        <div className="space-y-2">
                            <label className="text-sm font-medium">System Prompt</label>
                            <textarea
                                className="flex min-h-[100px] w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                placeholder="You are an expert assistant..."
                                value={config.system_prompt || ""}
                                onChange={e => setConfig({ ...config, system_prompt: e.target.value })}
                            />
                        </div>

                        {/* Max Daily Tokens + Dedicated Vector DB */}
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <label className="text-sm font-medium">Max Daily Tokens</label>
                                <Input
                                    type="number"
                                    value={config.max_daily_tokens || 100000}
                                    onChange={e => setConfig({ ...config, max_daily_tokens: parseInt(e.target.value) || 0 })}
                                />
                            </div>
                            <div className="space-y-2 flex items-end gap-2 pb-1">
                                <input
                                    type="checkbox"
                                    id="vectorDb"
                                    checked={config.is_dedicated_vector_db}
                                    onChange={e => setConfig({ ...config, is_dedicated_vector_db: e.target.checked })}
                                    className="w-4 h-4 rounded border-gray-300 text-primary"
                                />
                                <label htmlFor="vectorDb" className="text-sm font-medium">Dedicated Vector DB</label>
                            </div>
                        </div>

                        <div className="pt-4 flex justify-end">
                            <Button type="submit" disabled={isSaving}>
                                <Save className="w-4 h-4 mr-2" />
                                {isSaving ? "Saving..." : "Save Changes"}
                            </Button>
                        </div>
                    </form>
                ) : (
                    <div className="py-4 text-center text-muted-foreground">No configuration loaded.</div>
                )}
            </CardContent>
        </Card>
    );

    const renderPipelineTab = () => (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Workflow className="w-5 h-5 text-primary" />
                    Pipeline Settings
                </CardTitle>
                <CardDescription>
                    Configure chunking strategy, extraction settings, crawl limits, and deduplication threshold.
                </CardDescription>
            </CardHeader>
            <CardContent>
                <div className="space-y-6">
                    {/* Max Crawl Pages — Issue #164 */}
                    <div className="space-y-2">
                        <label className="text-sm font-medium">Max Crawl Pages</label>
                        <Input
                            type="number"
                            value={config?.max_crawl_pages ?? 100}
                            onChange={e => {
                                if (config) setConfig({ ...config, max_crawl_pages: Math.max(10, Math.min(500, parseInt(e.target.value) || 100)) });
                            }}
                            min={10}
                            max={500}
                        />
                        <p className="text-xs text-muted-foreground">
                            จำนวนหน้าสูงสุดที่ Web Hierarchy Loader จะ crawl (10–500, default: 100)
                        </p>
                    </div>

                    <div className="space-y-2">
                        <label className="text-sm font-medium">Chunking Strategy</label>
                        <select
                            value={chunkStrategy}
                            onChange={e => setChunkStrategy(e.target.value)}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                        >
                            <option value="auto">Auto (recommended)</option>
                            <option value="fixed">Fixed Size</option>
                            <option value="recursive">Recursive (Markdown-aware)</option>
                        </select>
                        <p className="text-xs text-muted-foreground">Auto mode selects the best strategy based on content type.</p>
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Chunk Size (chars)</label>
                            <Input
                                type="number"
                                value={chunkSize}
                                onChange={e => setChunkSize(parseInt(e.target.value) || 512)}
                                min={100}
                                max={4000}
                            />
                        </div>
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Chunk Overlap (chars)</label>
                            <Input
                                type="number"
                                value={chunkOverlap}
                                onChange={e => setChunkOverlap(parseInt(e.target.value) || 0)}
                                min={0}
                                max={500}
                            />
                        </div>
                    </div>

                    <div className="space-y-2">
                        <label className="text-sm font-medium">Dedup Threshold</label>
                        <select
                            value={dedupThreshold}
                            onChange={e => setDedupThreshold(parseInt(e.target.value))}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                        >
                            <option value={0}>Exact Match Only (SHA-256)</option>
                            <option value={3}>High Similarity (SimHash ≤ 3 bits)</option>
                            <option value={5}>Moderate Similarity (SimHash ≤ 5 bits)</option>
                            <option value={10}>Loose Similarity (SimHash ≤ 10 bits)</option>
                        </select>
                        <p className="text-xs text-muted-foreground">
                            Controls how similar content must be to be considered a duplicate.
                        </p>
                    </div>

                    <div className="pt-4 flex justify-end">
                        <Button
                            disabled={isSaving || !currentTenantId}
                            onClick={async () => {
                                if (!currentTenantId || !config) return;
                                setIsSaving(true);
                                try {
                                    await updateTenantConfig(currentTenantId, {
                                        max_crawl_pages: config.max_crawl_pages,
                                    });
                                    alert("Pipeline settings saved successfully.");
                                } catch (error) {
                                    console.warn("[Settings] Failed to save pipeline settings:", error);
                                    alert("Failed to save pipeline settings.");
                                } finally {
                                    setIsSaving(false);
                                }
                            }}
                        >
                            <Save className="w-4 h-4 mr-2" />
                            {isSaving ? "Saving..." : "Save Pipeline Settings"}
                        </Button>
                    </div>
                    <p className="text-xs text-muted-foreground text-right">
                        Chunking and dedup settings persistence will be wired in a future sprint.
                    </p>
                </div>
            </CardContent>
        </Card>
    );

    const renderSearchTab = () => (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2"><Search className="w-5 h-5" /> Search & Retrieval Settings</CardTitle>
                <CardDescription>Configure embedding model, retrieval parameters, and search modes</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
                <div className="grid gap-2">
                    <label className="text-sm font-medium">Embedding Model</label>
                    <select
                        value={embeddingModel}
                        onChange={(e) => setEmbeddingModel(e.target.value)}
                        className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                    >
                        <option value="nomic-embed-text">nomic-embed-text (Ollama — local)</option>
                        <option value="text-embedding-3-small">text-embedding-3-small (OpenAI)</option>
                        <option value="text-embedding-3-large">text-embedding-3-large (OpenAI)</option>
                        <option value="text-embedding-004">text-embedding-004 (Google)</option>
                        <option value="bge-m3">bge-m3 (Ollama — multilingual)</option>
                    </select>
                    <p className="text-xs text-muted-foreground">Changing the model requires re-embedding all existing chunks</p>
                </div>
                <div className="grid grid-cols-2 gap-6">
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Top-K Results</label>
                        <Input
                            type="number"
                            min={1}
                            max={50}
                            value={topK}
                            onChange={(e) => setTopK(parseInt(e.target.value) || 5)}
                        />
                        <p className="text-xs text-muted-foreground">Number of similar chunks to retrieve (1-50)</p>
                    </div>
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Similarity Threshold</label>
                        <div className="flex items-center gap-3">
                            <input
                                type="range"
                                min={0}
                                max={100}
                                value={similarityThreshold * 100}
                                onChange={(e) => setSimilarityThreshold(parseInt(e.target.value) / 100)}
                                className="flex-1 h-2 bg-gray-200 dark:bg-zinc-700 rounded-lg appearance-none cursor-pointer"
                            />
                            <span className="text-sm font-mono w-12 text-right">{similarityThreshold.toFixed(2)}</span>
                        </div>
                        <p className="text-xs text-muted-foreground">Minimum similarity score for results (0.0-1.0)</p>
                    </div>
                </div>
                <div className="grid gap-2">
                    <label className="text-sm font-medium">Search Mode</label>
                    <div className="grid grid-cols-3 gap-3">
                        {["semantic", "hybrid", "keyword"].map((mode) => (
                            <button
                                key={mode}
                                onClick={() => setSearchMode(mode)}
                                className={`p-3 rounded-lg border text-sm font-medium capitalize transition-colors ${searchMode === mode
                                    ? "border-blue-500 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400"
                                    : "border-border hover:bg-muted"
                                    }`}
                            >
                                {mode === "semantic" && "🧠 "}
                                {mode === "hybrid" && "🔀 "}
                                {mode === "keyword" && "🔤 "}
                                {mode}
                            </button>
                        ))}
                    </div>
                    <p className="text-xs text-muted-foreground">
                        {searchMode === "semantic" && "Pure vector similarity search using embeddings"}
                        {searchMode === "hybrid" && "Combines vector search, graph search, and SQL — best coverage"}
                        {searchMode === "keyword" && "Full-text keyword matching — fastest but least flexible"}
                    </p>
                </div>
                <div className="pt-4 border-t">
                    <Button
                        onClick={async () => {
                            if (!currentTenantId) return;
                            setIsSaving(true);
                            try {
                                await updateTenantConfig(currentTenantId, {
                                    search_settings: {
                                        embedding_model: embeddingModel,
                                        top_k: topK,
                                        similarity_threshold: similarityThreshold,
                                        search_mode: searchMode,
                                    },
                                } as any);
                                alert("Search settings saved successfully.");
                            } catch (error) {
                                console.warn("[Settings] Failed to save search settings:", error);
                                alert("Failed to save search settings.");
                            } finally {
                                setIsSaving(false);
                            }
                        }}
                        disabled={isSaving || !currentTenantId}
                    >
                        <Save className="w-4 h-4 mr-2" />
                        {isSaving ? "Saving..." : "Save Settings"}
                    </Button>
                </div>
            </CardContent>
        </Card>
    );

    const renderComingSoonTab = (icon: React.ElementType, title: string, sprint: string) => {
        const Icon = icon;
        return (
            <Card>
                <CardContent className="py-16">
                    <div className="flex flex-col items-center justify-center text-center">
                        <div className="w-16 h-16 rounded-full bg-gray-50 dark:bg-zinc-800 flex items-center justify-center mb-4">
                            <Icon className="w-8 h-8 text-gray-400 dark:text-zinc-500" />
                        </div>
                        <h3 className="text-lg font-semibold mb-2">{title}</h3>
                        <div className="inline-flex items-center px-3 py-1.5 rounded-full bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400 text-sm font-medium">
                            🚧 Coming in {sprint}
                        </div>
                    </div>
                </CardContent>
            </Card>
        );
    };

    const renderTenantsTab = () => (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between">
                <div>
                    <CardTitle className="flex items-center gap-2">
                        <Layers className="w-5 h-5 text-primary" />
                        Tenant Management
                    </CardTitle>
                    <CardDescription>Create and manage organization tenants.</CardDescription>
                </div>
                <Button size="sm" onClick={() => setShowCreateTenantDialog(true)}>
                    <Plus className="w-4 h-4 mr-1" /> Create Tenant
                </Button>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading...</div>
                ) : (
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Name</TableHead>
                                <TableHead>ID</TableHead>
                                <TableHead>Created</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {allTenants.map((t) => (
                                <TableRow key={t.id}>
                                    <TableCell className="font-medium">{t.name}</TableCell>
                                    <TableCell className="font-mono text-xs text-muted-foreground">{t.id.substring(0, 12)}...</TableCell>
                                    <TableCell className="text-sm" suppressHydrationWarning>{t.created_at ? new Date(t.created_at).toLocaleDateString() : "—"}</TableCell>
                                    <TableCell className="text-right">
                                        <Button variant="ghost" size="sm" className="text-red-500 hover:text-red-700"
                                            onClick={async () => {
                                                if (!confirm(`Delete tenant "${t.name}"? This cannot be undone.`)) return;
                                                try {
                                                    await deleteTenant(t.id);
                                                    loadData();
                                                } catch (err) { alert("Failed to delete tenant"); }
                                            }}
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </Button>
                                    </TableCell>
                                </TableRow>
                            ))}
                            {allTenants.length === 0 && (
                                <TableRow><TableCell colSpan={4} className="text-center py-8 text-muted-foreground">No tenants found.</TableCell></TableRow>
                            )}
                        </TableBody>
                    </Table>
                )}
            </CardContent>
        </Card>
    );

    const renderUsersTab = () => (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between">
                <div>
                    <CardTitle className="flex items-center gap-2">
                        <Users className="w-5 h-5 text-primary" />
                        User Management
                    </CardTitle>
                    <CardDescription>Create and manage platform users.</CardDescription>
                </div>
                <Button size="sm" onClick={() => {
                    if (allTenants.length > 0) setNewUser(prev => ({ ...prev, tenant_id: allTenants[0].id }));
                    setShowCreateUserDialog(true);
                }}>
                    <Plus className="w-4 h-4 mr-1" /> Create User
                </Button>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading...</div>
                ) : (
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Username</TableHead>
                                <TableHead>ID</TableHead>
                                <TableHead>Tenant</TableHead>
                                <TableHead>Role</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {allUsers.map((u) => (
                                <TableRow key={u.id}>
                                    <TableCell className="font-medium">{u.username}</TableCell>
                                    <TableCell className="font-mono text-xs text-muted-foreground">{u.id.substring(0, 12)}...</TableCell>
                                    <TableCell className="text-sm">{allTenants.find(t => t.id === u.tenant_id)?.name || u.tenant_id?.substring(0, 8) || "—"}</TableCell>
                                    <TableCell>
                                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${u.role === "admin" ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400" : "bg-gray-100 text-gray-600 dark:bg-zinc-800 dark:text-zinc-400"
                                            }`}>{u.role || "viewer"}</span>
                                    </TableCell>
                                    <TableCell className="text-right">
                                        <Button variant="ghost" size="sm" className="text-red-500 hover:text-red-700"
                                            onClick={async () => {
                                                if (!confirm(`Delete user "${u.username}"?`)) return;
                                                try {
                                                    await deleteUser(u.id);
                                                    loadData();
                                                } catch (err) { alert("Failed to delete user"); }
                                            }}
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </Button>
                                    </TableCell>
                                </TableRow>
                            ))}
                            {allUsers.length === 0 && (
                                <TableRow><TableCell colSpan={5} className="text-center py-8 text-muted-foreground">No users found.</TableCell></TableRow>
                            )}
                        </TableBody>
                    </Table>
                )}
            </CardContent>
        </Card>
    );

    const renderTabContent = () => {
        switch (activeTab) {
            case "general":
                return renderGeneralTab();
            case "ai-models":
                return renderAIModelsTab();
            case "pipeline":
                return renderPipelineTab();
            case "knowledge-graph":
                return (
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Share2 className="w-5 h-5" />
                                Knowledge Graph Settings
                            </CardTitle>
                            <CardDescription>Configure entity extraction and graph visualization.</CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="p-4 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
                                <h4 className="font-medium text-green-800 dark:text-green-300">✓ Knowledge Graph Active</h4>
                                <p className="text-sm text-green-600 dark:text-green-400 mt-1">
                                    Sprint 17 — Entity extraction, graph visualization, and path finding are available.
                                </p>
                            </div>
                            <div className="grid grid-cols-2 gap-4">
                                <a href="/graph" className="p-4 rounded-lg border border-slate-200 dark:border-zinc-700 hover:border-purple-400 dark:hover:border-purple-500 transition-colors group">
                                    <h4 className="font-medium group-hover:text-purple-600 dark:group-hover:text-purple-400">Open Graph Explorer</h4>
                                    <p className="text-sm text-muted-foreground mt-1">Visualize entities and relationships</p>
                                </a>
                                <div className="p-4 rounded-lg border border-slate-200 dark:border-zinc-700">
                                    <h4 className="font-medium">Neo4j Connection</h4>
                                    <p className="text-sm text-muted-foreground mt-1">bolt://localhost:7687 (default)</p>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                );
            case "search":
                return renderSearchTab();
            case "security":
                return renderComingSoonTab(Shield, "Security & Access Settings", "Sprint 14");
            case "tenants":
                return renderTenantsTab();
            case "users":
                return renderUsersTab();
            default:
                return null;
        }
    };

    // ─── Render ─────────────────────────────────────────────────────────────────

    return (
        <div className="container mx-auto p-8">
            <div className="mb-8">
                <h1 className="text-3xl font-bold tracking-tight">Admin Settings</h1>
                <p className="text-muted-foreground">Manage your workspace, AI models, and pipeline configuration.</p>
            </div>

            <div className="flex gap-8">
                {/* Sidebar Tabs */}
                <nav className="w-56 shrink-0">
                    <div className="space-y-1">
                        {TABS.map((tab) => {
                            const Icon = tab.icon;
                            const isActive = activeTab === tab.id;
                            return (
                                <button
                                    key={tab.id}
                                    onClick={() => setActiveTab(tab.id)}
                                    className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all ${isActive
                                        ? "bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 shadow-sm"
                                        : "text-gray-600 hover:bg-gray-50 dark:text-zinc-400 dark:hover:bg-zinc-800/50"
                                        }`}
                                >
                                    <Icon className={`w-4 h-4 ${isActive ? "text-blue-600 dark:text-blue-400" : ""}`} />
                                    {tab.label}
                                </button>
                            );
                        })}
                    </div>
                </nav>

                {/* Tab Content */}
                <div className="flex-1 max-w-3xl">
                    {renderTabContent()}
                </div>
            </div>

            {/* ═══ Create Tenant Dialog ═══ */}
            <Dialog open={showCreateTenantDialog} onOpenChange={setShowCreateTenantDialog}>
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Create New Tenant</DialogTitle>
                        <DialogDescription>Create a new organization tenant with an admin user.</DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-2">
                        <div className="space-y-2">
                            <Label>Tenant Name</Label>
                            <Input placeholder="e.g. MegaCare" value={newTenant.name} onChange={e => setNewTenant({ ...newTenant, name: e.target.value })} />
                        </div>
                        <div className="space-y-2">
                            <Label>Admin Username</Label>
                            <Input placeholder="e.g. admin@megacare.com" value={newTenant.admin_email} onChange={e => setNewTenant({ ...newTenant, admin_email: e.target.value })} />
                        </div>
                        <div className="space-y-2">
                            <Label>Admin Password</Label>
                            <Input type="password" placeholder="Min 6 characters" value={newTenant.admin_password} onChange={e => setNewTenant({ ...newTenant, admin_password: e.target.value })} />
                        </div>
                        <div className="flex items-center gap-2">
                            <input type="checkbox" id="dedicatedVdb" checked={newTenant.is_dedicated_vector_db} onChange={e => setNewTenant({ ...newTenant, is_dedicated_vector_db: e.target.checked })} className="w-4 h-4" />
                            <Label htmlFor="dedicatedVdb" className="text-sm">Dedicated Vector DB</Label>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreateTenantDialog(false)}>Cancel</Button>
                        <Button disabled={!newTenant.name || !newTenant.admin_email} onClick={async () => {
                            try {
                                await createTenant(newTenant as CreateTenantRequest);
                                setShowCreateTenantDialog(false);
                                setNewTenant({ name: "", admin_email: "", admin_password: "", is_dedicated_vector_db: false });
                                loadData();
                            } catch (err) { alert("Failed to create tenant"); }
                        }}>Create</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* ═══ Create User Dialog ═══ */}
            <Dialog open={showCreateUserDialog} onOpenChange={setShowCreateUserDialog}>
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Create New User</DialogTitle>
                        <DialogDescription>Add a user to an existing tenant.</DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-2">
                        <div className="space-y-2">
                            <Label>Username</Label>
                            <Input placeholder="e.g. john_doe" value={newUser.username} onChange={e => setNewUser({ ...newUser, username: e.target.value })} />
                        </div>
                        <div className="space-y-2">
                            <Label>Password</Label>
                            <Input type="password" placeholder="Min 6 characters" value={newUser.password} onChange={e => setNewUser({ ...newUser, password: e.target.value })} />
                        </div>
                        <div className="space-y-2">
                            <Label>Tenant</Label>
                            <select
                                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                                value={newUser.tenant_id}
                                onChange={e => setNewUser({ ...newUser, tenant_id: e.target.value })}
                            >
                                {allTenants.map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
                            </select>
                        </div>
                        <div className="space-y-2">
                            <Label>Role</Label>
                            <select
                                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                                value={newUser.role}
                                onChange={e => setNewUser({ ...newUser, role: e.target.value })}
                            >
                                <option value="admin">Admin</option>
                                <option value="editor">Editor</option>
                                <option value="viewer">Viewer</option>
                            </select>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreateUserDialog(false)}>Cancel</Button>
                        <Button disabled={!newUser.username || !newUser.tenant_id} onClick={async () => {
                            try {
                                await createUser({ username: newUser.username, password: newUser.password || undefined, tenant_id: newUser.tenant_id, role: newUser.role });
                                setShowCreateUserDialog(false);
                                setNewUser({ username: "", password: "", tenant_id: allTenants[0]?.id || "", role: "viewer" });
                                loadData();
                            } catch (err) { alert("Failed to create user"); }
                        }}>Create</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
