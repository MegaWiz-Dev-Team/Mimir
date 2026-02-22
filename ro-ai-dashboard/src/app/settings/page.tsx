"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Building2, Save } from "lucide-react";

import { fetchTenants, updateTenant, Tenant } from "@/lib/api";

export default function SettingsPage() {
    const [tenants, setTenants] = useState<Tenant[]>([]);
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

            // For MVP, assume admin manages the first tenant (or their own assigned tenant).
            // In a robust implementation, we might get the user's specific `tenant_id` from their JWT or a `/me` endpoint.
            if (tenantsData.length > 0) {
                const firstTenant = tenantsData[0];
                setCurrentTenantId(firstTenant.id);
                setTenantName(firstTenant.name);
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

            <div className="grid gap-6 max-w-2xl">
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
                            <form onSubmit={handleSave} className="space-y-4">
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
            </div>
        </div>
    );
}
