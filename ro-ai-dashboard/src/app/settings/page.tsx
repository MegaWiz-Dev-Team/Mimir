"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
    Building2, Save, Settings2, Bot, Workflow, Share2,
    Search, Shield, Lock
} from "lucide-react";

import { fetchTenants, updateTenant, fetchTenantConfig, updateTenantConfig, Tenant, TenantConfig } from "@/lib/api";

// ─── Tab Definitions ────────────────────────────────────────────────────────────

const TABS = [
    { id: "general", label: "General", icon: Building2 },
    { id: "ai-models", label: "AI Models", icon: Bot },
    { id: "pipeline", label: "Pipeline", icon: Workflow },
    { id: "knowledge-graph", label: "Knowledge Graph", icon: Share2 },
    { id: "search", label: "Search", icon: Search },
    { id: "security", label: "Security", icon: Shield },
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

    useEffect(() => {
        loadData();
    }, []);

    const loadData = async () => {
        setIsLoading(true);
        try {
            const tenantsData = await fetchTenants();
            setTenants(tenantsData);

            if (tenantsData.length > 0) {
                const firstTenant = tenantsData[0];
                setCurrentTenantId(firstTenant.id);
                setTenantName(firstTenant.name);

                try {
                    const configData = await fetchTenantConfig(firstTenant.id);
                    setConfig(configData);
                } catch (err) {
                    console.warn("[Settings] Failed to load tenant config:", err);
                }
            }
        } catch (error) {
            console.warn("[Settings] Failed to load tenants:", error);
            alert("Failed to load tenant data. Are you logged in as Admin?");
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

    const renderAIModelsTab = () => (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Settings2 className="w-5 h-5 text-primary" />
                    AI Model Configuration
                </CardTitle>
                <CardDescription>
                    Configure default models, limits, and system behavior.
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading...</div>
                ) : config ? (
                    <form onSubmit={handleSave} className="space-y-6">
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <label className="text-sm font-medium">Default Provider</label>
                                <Input
                                    placeholder="ollama, gemini"
                                    value={config.default_provider || ""}
                                    onChange={e => setConfig({ ...config, default_provider: e.target.value })}
                                />
                            </div>
                            <div className="space-y-2">
                                <label className="text-sm font-medium">Default Model</label>
                                <Input
                                    placeholder="llama3.2, gemini-2.5-flash"
                                    value={config.default_model || ""}
                                    onChange={e => setConfig({ ...config, default_model: e.target.value })}
                                />
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

                        <div className="space-y-2">
                            <label className="text-sm font-medium">Max Daily Tokens</label>
                            <Input
                                type="number"
                                value={config.max_daily_tokens || 100000}
                                onChange={e => setConfig({ ...config, max_daily_tokens: parseInt(e.target.value) || 0 })}
                            />
                        </div>

                        <div className="space-y-2 flex items-center gap-2 pt-2">
                            <input
                                type="checkbox"
                                id="vectorDb"
                                checked={config.is_dedicated_vector_db}
                                onChange={e => setConfig({ ...config, is_dedicated_vector_db: e.target.checked })}
                                className="w-4 h-4 rounded border-gray-300 text-primary"
                            />
                            <label htmlFor="vectorDb" className="text-sm font-medium">Use Dedicated Vector DB Collection</label>
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
                    Configure chunking strategy, extraction settings, and deduplication threshold.
                </CardDescription>
            </CardHeader>
            <CardContent>
                <div className="space-y-6">
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
                        <Button disabled>
                            <Save className="w-4 h-4 mr-2" />
                            Save Pipeline Settings
                        </Button>
                    </div>
                    <p className="text-xs text-muted-foreground text-right">
                        Pipeline settings persistence will be wired in a future sprint.
                    </p>
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

    const renderTabContent = () => {
        switch (activeTab) {
            case "general":
                return renderGeneralTab();
            case "ai-models":
                return renderAIModelsTab();
            case "pipeline":
                return renderPipelineTab();
            case "knowledge-graph":
                return renderComingSoonTab(Share2, "Knowledge Graph Settings", "Sprint 11");
            case "search":
                return renderComingSoonTab(Search, "Search & Vector DB Settings", "Sprint 10");
            case "security":
                return renderComingSoonTab(Shield, "Security & Access Settings", "Sprint 14");
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
        </div>
    );
}
