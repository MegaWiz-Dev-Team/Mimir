"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Building2, Save, Settings2 } from "lucide-react";

import { fetchTenants, updateTenant, fetchTenantConfig, updateTenantConfig, Tenant, TenantConfig } from "@/lib/api";

export default function SettingsPage() {
    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [config, setConfig] = useState<TenantConfig | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [tenantName, setTenantName] = useState("");
    const [currentTenantId, setCurrentTenantId] = useState<string | null>(null);

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
                    console.error("Failed to load tenant config", err);
                }
            }
        } catch (error) {
            console.error("Failed to load tenants:", error);
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
            alert("Tenant settings updated successfully.");
            loadData();
        } catch (error) {
            console.error(error);
            alert("Failed to update tenant.");
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <div className="container mx-auto p-8">
            <div className="mb-8">
                <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
                <p className="text-muted-foreground">Manage your workspace and tenant preferences.</p>
            </div>

            <div className="grid gap-6 max-w-3xl">
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

                                {config && (
                                    <>
                                        <hr className="my-6 border-muted" />
                                        <div className="space-y-2 mb-4">
                                            <h3 className="text-lg font-medium flex items-center gap-2">
                                                <Settings2 className="w-5 h-5 text-primary" />
                                                AI Configurations
                                            </h3>
                                            <p className="text-sm text-muted-foreground">
                                                Configure default models, limits, and system behavior.
                                            </p>
                                        </div>

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
                                    </>
                                )}

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
            </div>
        </div>
    );
}
