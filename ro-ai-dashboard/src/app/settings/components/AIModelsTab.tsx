"use client";

import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Settings2, Save, Lock } from "lucide-react";
import { LlmConfig, LlmSlot } from "@/lib/api";
import { SettingsTabProps } from "./types";

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
    heimdall: [
        { value: "BAAI/bge-m3", label: "BGE-M3 (MLX)" },
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

function SlotCard({ slotName, icon, title, desc, config, setConfig }: {
    slotName: keyof LlmConfig; icon: string; title: string; desc: string;
    config: NonNullable<SettingsTabProps["config"]>; setConfig: SettingsTabProps["setConfig"];
}) {
    const slot = (config?.llm_config?.[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
    const isEmbedding = slotName === "embedding";
    const providerModels = isEmbedding ? (EMBEDDING_MODEL_OPTIONS[slot.provider] || []) : (MODEL_OPTIONS[slot.provider] || []);

    const updateSlot = (field: "provider" | "model", value: string) => {
        if (!config) return;
        const current = config.llm_config || {};
        const currentSlot = (current[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
        const updatedSlot = { ...currentSlot, [field]: value };
        if (field === "provider") {
            const models = isEmbedding ? (EMBEDDING_MODEL_OPTIONS[value] || []) : (MODEL_OPTIONS[value] || []);
            updatedSlot.model = models[0]?.value || "";
        }
        setConfig({ ...config, llm_config: { ...current, [slotName]: updatedSlot } });
    };

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
                        onChange={e => updateSlot("provider", e.target.value)}>
                        <option value="">Select...</option>
                        {(isEmbedding ? [
                            { value: "heimdall", label: "Heimdall (Self-Hosted)" },
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
                        onChange={e => updateSlot("model", e.target.value)}>
                        <option value="">Select...</option>
                        {providerModels.map(m => (
                            <option key={m.value} value={m.value}>{m.label}</option>
                        ))}
                    </select>
                </div>
            </div>
        </div>
    );
}

export function AIModelsTab({ isLoading, isSaving, config, setConfig, handleSave }: SettingsTabProps) {
    if (!config) return <div className="py-4 text-center text-muted-foreground">No configuration loaded.</div>;

    return (
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
                ) : (
                    <form onSubmit={handleSave} className="space-y-6">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <SlotCard slotName="chat" icon="💬" title="Chat & NPC" desc="Agent chat (Tier 1+2)" config={config} setConfig={setConfig} />
                            <SlotCard slotName="rag" icon="📚" title="RAG (Oracle Agent)" desc="Knowledge retrieval queries" config={config} setConfig={setConfig} />
                            <SlotCard slotName="pipeline_generator" icon="🔄" title="Pipeline Generator" desc="QA pair generation" config={config} setConfig={setConfig} />
                            <SlotCard slotName="judge" icon="⚖️" title="Evaluation Judge" desc="LLM-as-Judge scoring" config={config} setConfig={setConfig} />
                            <SlotCard slotName="embedding" icon="🧬" title="Embedding" desc="Vector embedding model" config={config} setConfig={setConfig} />
                        </div>

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
                                        placeholder="http://localhost:8080/v1"
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
                                        placeholder="sk-... (dev only, use Vault in production)"
                                        value={config.llm_config?.heimdall_api_key || ""}
                                        onChange={e => setConfig({ ...config, llm_config: { ...config.llm_config, heimdall_api_key: e.target.value } })}
                                    />
                                </div>
                                <p className="text-xs text-muted-foreground col-span-full mt-1">💡 For production, manage API keys via <strong>Security → Vault</strong> instead of storing here.</p>
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="text-sm font-medium">System Prompt</label>
                            <textarea
                                className="flex min-h-[100px] w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                placeholder="You are an expert assistant..."
                                value={config.system_prompt || ""}
                                onChange={e => setConfig({ ...config, system_prompt: e.target.value })}
                            />
                        </div>

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
                )}
            </CardContent>
        </Card>
    );
}
