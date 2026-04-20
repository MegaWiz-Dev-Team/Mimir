"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Settings2, Save, RefreshCw, CheckCircle2, XCircle, Server } from "lucide-react";
import { LlmConfig, LlmSlot, fetchModels, modelsToProviders, LlmProvider, API_BASE_URL, authFetch } from "@/lib/api";
import { SettingsTabProps } from "./types";

const selectClass = "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

function SlotCard({ slotName, icon, title, desc, config, setConfig, providers }: {
    slotName: keyof LlmConfig; icon: string; title: string; desc: string;
    config: NonNullable<SettingsTabProps["config"]>; setConfig: SettingsTabProps["setConfig"];
    providers: LlmProvider[];
}) {
    const slot = (config?.llm_config?.[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
    
    // Find selected provider's models
    const selectedProvider = providers.find(p => p.id === slot.provider);
    const providerModels = selectedProvider?.models || [];

    const updateSlot = (field: "provider" | "model", value: string) => {
        if (!config) return;
        const current = config.llm_config || {};
        const currentSlot = (current[slotName] as LlmSlot | undefined) || { provider: "", model: "" };
        const updatedSlot = { ...currentSlot, [field]: value };
        
        if (field === "provider") {
            const p = providers.find(p => p.id === value);
            updatedSlot.model = p?.models[0]?.id || "";
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
                        {providers.map(p => (
                            <option key={p.id} value={p.id}>{p.display_name}</option>
                        ))}
                    </select>
                </div>
                <div className="space-y-1">
                    <label className="text-xs text-muted-foreground">Model</label>
                    <select className={selectClass} value={slot.model}
                        onChange={e => updateSlot("model", e.target.value)}>
                        <option value="">Select...</option>
                        {providerModels.map(m => (
                            <option key={m.id} value={m.id}>{m.display_name}</option>
                        ))}
                    </select>
                </div>
            </div>
        </div>
    );
}

interface SyncStatus {
    type: "success" | "error";
    message: string;
    synced?: number;
    deactivated?: number;
    timestamp?: Date;
}

export function AIModelsTab({ isLoading, isSaving, config, setConfig, handleSaveAIModels }: SettingsTabProps) {
    const [providers, setProviders] = useState<LlmProvider[]>([]);
    const [isLoadingModels, setIsLoadingModels] = useState(true);
    const [isSyncing, setIsSyncing] = useState(false);
    const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

    const handleSyncModels = async () => {
        setIsSyncing(true);
        setSyncStatus(null);
        try {
            const res = await authFetch(`${API_BASE_URL}/config/models/sync`, { method: "POST" });
            if (!res.ok) {
                const errBody = await res.json().catch(() => ({ error: "Unknown error" }));
                throw new Error(errBody.error || `HTTP ${res.status}`);
            }
            const data = await res.json();
            
            // Reload models after sync
            const models = await fetchModels();
            setProviders(modelsToProviders(models));

            setSyncStatus({
                type: "success",
                message: `Synced ${data.synced_models} model${data.synced_models !== 1 ? 's' : ''} from Heimdall & Ollama`,
                synced: data.synced_models,
                deactivated: data.deactivated_models,
                timestamp: new Date(),
            });
        } catch (err: any) {
            setSyncStatus({
                type: "error",
                message: err.message || "Failed to sync models",
                timestamp: new Date(),
            });
        } finally {
            setIsSyncing(false);
        }
    };

    useEffect(() => {
        fetchModels()
            .then(models => {
                const uiProviders = modelsToProviders(models);
                setProviders(uiProviders);
                setIsLoadingModels(false);
            })
            .catch(err => {
                console.error("Failed to load models:", err);
                setIsLoadingModels(false);
            });
    }, []);

    if (!config) return <div className="py-4 text-center text-muted-foreground">No configuration loaded.</div>;

    const defaultProviderData = providers.find(p => p.id === config.default_provider);
    const defaultProviderModels = defaultProviderData?.models || [];
    const totalModels = providers.reduce((sum, p) => sum + p.models.length, 0);

    return (
        <div className="space-y-6">
        {/* Sync Status Banner */}
        {syncStatus && (
            <div className={`rounded-lg border p-4 transition-all animate-in fade-in slide-in-from-top-2 duration-300 ${
                syncStatus.type === "success"
                    ? "border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-950/30"
                    : "border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-950/30"
            }`}>
                <div className="flex items-start gap-3">
                    {syncStatus.type === "success" ? (
                        <CheckCircle2 className="w-5 h-5 text-green-600 dark:text-green-400 mt-0.5 shrink-0" />
                    ) : (
                        <XCircle className="w-5 h-5 text-red-600 dark:text-red-400 mt-0.5 shrink-0" />
                    )}
                    <div className="flex-1">
                        <p className={`text-sm font-medium ${
                            syncStatus.type === "success" ? "text-green-800 dark:text-green-300" : "text-red-800 dark:text-red-300"
                        }`}>
                            {syncStatus.message}
                        </p>
                        {syncStatus.type === "success" && syncStatus.deactivated !== undefined && syncStatus.deactivated > 0 && (
                            <p className="text-xs text-green-600 dark:text-green-400 mt-1">
                                {syncStatus.deactivated} model{syncStatus.deactivated !== 1 ? 's' : ''} deactivated (no longer available on gateway)
                            </p>
                        )}
                        {syncStatus.timestamp && (
                            <p className="text-xs text-muted-foreground mt-1" suppressHydrationWarning>
                                {syncStatus.timestamp.toLocaleTimeString()}
                            </p>
                        )}
                    </div>
                    <button onClick={() => setSyncStatus(null)} className="text-muted-foreground hover:text-foreground text-sm">✕</button>
                </div>
            </div>
        )}

        <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
                <div>
                    <CardTitle className="flex items-center gap-2">
                        <Settings2 className="w-5 h-5 text-primary" />
                        AI Model Configuration
                    </CardTitle>
                    <CardDescription className="mt-1.5">
                        Configure models for each purpose. Each slot can use a different provider and model.
                    </CardDescription>
                </div>
                <div className="flex gap-2">
                    <Button 
                        variant="outline" 
                        size="sm" 
                        onClick={handleSyncModels} 
                        disabled={isSyncing}
                        className="gap-2"
                    >
                        <RefreshCw className={`w-4 h-4 ${isSyncing ? 'animate-spin' : ''}`} />
                        {isSyncing ? "Syncing..." : "Sync Models"}
                    </Button>
                </div>
            </CardHeader>
            <CardContent>
                {isLoading || isLoadingModels ? (
                    <div className="py-4 text-center text-muted-foreground">Loading...</div>
                ) : (
                    <form onSubmit={handleSaveAIModels} className="space-y-6">
                        <div className="rounded-lg border bg-slate-50 dark:bg-zinc-800/30 p-4 space-y-4">
                            <div className="flex items-center gap-2 mb-2">
                                <span className="text-xl">⭐</span>
                                <div>
                                    <h4 className="font-medium">Default Provider & Model</h4>
                                    <p className="text-xs text-muted-foreground">Fallback configuration when a specific task slot is not assigned.</p>
                                </div>
                            </div>
                            <div className="grid grid-cols-2 gap-4">
                                <div className="space-y-1">
                                    <label className="text-xs font-semibold text-muted-foreground">Provider</label>
                                    <select className={selectClass} value={config.default_provider || ""} aria-label="Default Provider"
                                        onChange={e => {
                                            const providerId = e.target.value;
                                            const pData = providers.find(p => p.id === providerId);
                                            setConfig({ ...config, default_provider: providerId, default_model: pData?.models[0]?.id || "" });
                                        }}>
                                        <option value="">Select...</option>
                                        {providers.map(p => (
                                            <option key={p.id} value={p.id}>{p.display_name}</option>
                                        ))}
                                    </select>
                                </div>
                                <div className="space-y-1">
                                    <label className="text-xs font-semibold text-muted-foreground">Model</label>
                                    <select className={selectClass} value={config.default_model || ""} aria-label="Default Model"
                                        onChange={e => setConfig({ ...config, default_model: e.target.value })}>
                                        <option value="">Select...</option>
                                        {defaultProviderModels.map(m => (
                                            <option key={m.id} value={m.id}>{m.display_name}</option>
                                        ))}
                                    </select>
                                </div>
                            </div>
                        </div>

                        <div className="pt-2">
                            <h3 className="text-sm font-semibold mb-3">Task Assignments</h3>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <SlotCard slotName="chat" icon="💬" title="Chat & NPC" desc="Agent chat (Tier 1+2)" config={config} setConfig={setConfig} providers={providers} />
                                <SlotCard slotName="rag" icon="📚" title="RAG (Oracle Agent)" desc="Knowledge retrieval queries" config={config} setConfig={setConfig} providers={providers} />
                                <SlotCard slotName="pipeline_extractor" icon="📄" title="OCR & Extractor" desc="Document & Image text extraction" config={config} setConfig={setConfig} providers={providers} />
                                <SlotCard slotName="pipeline_generator" icon="🔄" title="Pipeline Generator" desc="QA pair generation" config={config} setConfig={setConfig} providers={providers} />
                                <SlotCard slotName="pipeline_evaluator" icon="📊" title="Pipeline Evaluator" desc="Coverage & ACU extraction" config={config} setConfig={setConfig} providers={providers} />
                                <SlotCard slotName="judge" icon="⚖️" title="Evaluation Judge" desc="LLM-as-Judge scoring" config={config} setConfig={setConfig} providers={providers} />
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

                        <div className="flex justify-end pt-4 border-t mt-6">
                            <Button type="submit" disabled={isSaving} size="sm" className="gap-2">
                                {isSaving ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                                {isSaving ? "Saving..." : "Save Changes"}
                            </Button>
                        </div>
                    </form>
                )}
            </CardContent>
        </Card>

        {/* Model Registry Overview */}
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Server className="w-5 h-5 text-primary" />
                    Model Registry
                    <span className="text-sm font-normal text-muted-foreground">({totalModels} models)</span>
                </CardTitle>
                <CardDescription>
                    Models synced from Heimdall Gateway and Ollama. Click &quot;Sync Models&quot; above to refresh from connected providers.
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoadingModels ? (
                    <div className="py-4 text-center text-muted-foreground">Loading model registry...</div>
                ) : providers.length === 0 ? (
                    <div className="py-8 text-center">
                        <Server className="w-10 h-10 mx-auto mb-3 text-muted-foreground/40" />
                        <p className="text-sm text-muted-foreground">No models in registry.</p>
                        <p className="text-xs text-muted-foreground mt-1">Click &quot;Sync Models&quot; to pull from Heimdall and Ollama.</p>
                    </div>
                ) : (
                    <div className="space-y-4">
                        {providers.map(provider => (
                            <div key={provider.id} className="rounded-lg border bg-card">
                                <div className="flex items-center justify-between px-4 py-3 border-b bg-muted/30">
                                    <div className="flex items-center gap-2">
                                        <span className="text-sm font-medium">{provider.display_name}</span>
                                        <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400">
                                            {provider.models.length} model{provider.models.length !== 1 ? 's' : ''}
                                        </span>
                                    </div>
                                </div>
                                <div className="divide-y">
                                    {provider.models.map(model => (
                                        <div key={model.id} className="px-4 py-2.5 flex items-center justify-between text-sm">
                                            <div>
                                                <span className="font-medium">{model.display_name}</span>
                                                <span className="text-xs text-muted-foreground ml-2 font-mono">({model.id})</span>
                                            </div>
                                            <div className="flex items-center gap-2">
                                                {model.capabilities?.reasoning && (
                                                    <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">🧠 Reasoning</span>
                                                )}
                                                {model.capabilities?.tools && (
                                                    <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400">🔧 Tools</span>
                                                )}
                                                {model.capabilities?.vision && (
                                                    <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">👁 Vision</span>
                                                )}
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </CardContent>
        </Card>
        </div>
    );
}
