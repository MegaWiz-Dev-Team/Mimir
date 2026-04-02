"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Settings2, Save, Plus, Loader2, Download } from "lucide-react";
import { Dialog, DialogTrigger, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { LlmConfig, LlmSlot, fetchModels, modelsToProviders, LlmProvider } from "@/lib/api";
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

export function AIModelsTab({ isLoading, isSaving, config, setConfig, handleSaveAIModels }: SettingsTabProps) {
    const [providers, setProviders] = useState<LlmProvider[]>([]);
    const [isLoadingModels, setIsLoadingModels] = useState(true);

    // Dialog state
    const [isAddModalOpen, setIsAddModalOpen] = useState(false);
    const [addProvider, setAddProvider] = useState<string>("ollama");
    const [addModelId, setAddModelId] = useState<string>("");
    const [isPulling, setIsPulling] = useState(false);
    const [pullError, setPullError] = useState<string | null>(null);

    const handlePullModel = async () => {
        if (!addModelId.trim()) return;
        setIsPulling(true);
        setPullError(null);
        try {
            // Note: UI integrated, backend will intercept in subsequent task
            const res = await fetch("/api/v1/config/models/pull", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ provider: addProvider, model_id: addModelId })
            });
            if (!res.ok) throw new Error((await res.json()).error || "Failed to pull model");
            
            // Reload models
            const models = await fetchModels();
            setProviders(modelsToProviders(models));
            setIsAddModalOpen(false);
            setAddModelId("");
        } catch (err: any) {
            setPullError(err.message);
        } finally {
            setIsPulling(false);
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

    return (
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
                <Dialog open={isAddModalOpen} onOpenChange={setIsAddModalOpen}>
                    <DialogTrigger asChild>
                        <Button variant="outline" size="sm" className="gap-2 bg-primary/5 hover:bg-primary/10 border-primary/20 text-primary">
                            <Download className="w-4 h-4" /> Add Model
                        </Button>
                    </DialogTrigger>
                    <DialogContent className="sm:max-w-[425px]">
                        <DialogHeader>
                            <DialogTitle>Pull New Model</DialogTitle>
                            <DialogDescription>
                                Download models directly to the host machine via Ollama or Heimdall.
                            </DialogDescription>
                        </DialogHeader>
                        <div className="grid gap-4 py-4">
                            <div className="space-y-2">
                                <label className="text-sm font-medium">Provider Network</label>
                                <select 
                                    value={addProvider} 
                                    onChange={e => setAddProvider(e.target.value)}
                                    className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                                >
                                    <option value="ollama">Ollama</option>
                                    <option value="heimdall">Heimdall (MLX via Host)</option>
                                </select>
                            </div>
                            <div className="space-y-2">
                                <label className="text-sm font-medium">Model ID / HuggingFace Path</label>
                                <Input 
                                    placeholder={addProvider === 'ollama' ? "e.g., qwen2.5:1.5b" : "e.g., mlx-community/Qwen3.5-27B-4bit"}
                                    value={addModelId}
                                    onChange={e => setAddModelId(e.target.value)}
                                />
                                <p className="text-[10px] text-muted-foreground">
                                    {addProvider === 'heimdall' && 'Models will be written to External SSD cache via RustFS.'}
                                </p>
                            </div>
                            {pullError && (
                                <div className="text-xs text-red-500 bg-red-500/10 p-2 rounded-md">
                                    Error: {pullError}
                                </div>
                            )}
                        </div>
                        <DialogFooter>
                            <Button 
                                onClick={handlePullModel} 
                                disabled={!addModelId.trim() || isPulling}
                                className="w-full sm:w-auto"
                            >
                                {isPulling ? (
                                    <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Pulling...</>
                                ) : (
                                    <><Download className="w-4 h-4 mr-2" /> Pull & Register</>
                                )}
                            </Button>
                        </DialogFooter>
                    </DialogContent>
                </Dialog>
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
