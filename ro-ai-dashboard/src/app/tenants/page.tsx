"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Building2, Plus, Trash2, Shield } from "lucide-react";

import { fetchTenants, createTenant, deleteTenant, Tenant, CreateTenantRequest } from "@/lib/api";

export default function TenantsPage() {
    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [isCreating, setIsCreating] = useState(false);

    const [newTenantName, setNewTenantName] = useState("");
    const [newAdminEmail, setNewAdminEmail] = useState("");
    const [newAdminPassword, setNewAdminPassword] = useState("");
    const [isDedicatedDb, setIsDedicatedDb] = useState(false);

    useEffect(() => {
        loadTenants();
    }, []);

    const loadTenants = async () => {
        setIsLoading(true);
        try {
            const data = await fetchTenants();
            setTenants(data);
        } catch (error) {
            console.error(error);
            alert("Failed to load tenants. Ensure you are an Admin.");
        } finally {
            setIsLoading(false);
        }
    };

    const handleCreate = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!newTenantName || !newAdminEmail) return;

        setIsCreating(true);
        try {
            const req: CreateTenantRequest = {
                name: newTenantName,
                admin_email: newAdminEmail,
                admin_password: newAdminPassword || undefined,
                is_dedicated_vector_db: isDedicatedDb
            };
            await createTenant(req);
            alert("Tenant provisioned successfully!");

            setNewTenantName("");
            setNewAdminEmail("");
            setNewAdminPassword("");
            setIsDedicatedDb(false);

            loadTenants();
        } catch (error) {
            console.error(error);
            alert("Failed to create tenant");
        } finally {
            setIsCreating(false);
        }
    };

    const handleDelete = async (id: string, name: string) => {
        if (!confirm(`Are you sure you want to delete the tenant '${name}'? This action is irreversible and drops all isolated vectors and data!`)) return;

        try {
            await deleteTenant(id);
            alert("Tenant deleted");
            loadTenants();
        } catch (error) {
            console.error(error);
            alert("Failed to delete tenant");
        }
    };

    return (
        <div className="container mx-auto p-8">
            <div className="mb-8">
                <h1 className="text-3xl font-bold tracking-tight">Tenant Management</h1>
                <p className="text-muted-foreground">Superadmin dashboard to provision and manage customer workspaces.</p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Plus className="w-5 h-5 text-primary" />
                                Provision New Tenant
                            </CardTitle>
                            <CardDescription>
                                Create a new isolated workspace with a default admin user.
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <form onSubmit={handleCreate} className="space-y-4">
                                <div className="space-y-2">
                                    <label className="text-sm font-medium">Tenant/Company Name</label>
                                    <Input
                                        required
                                        placeholder="Acme Corp"
                                        value={newTenantName}
                                        onChange={e => setNewTenantName(e.target.value)}
                                    />
                                </div>

                                <div className="space-y-2">
                                    <label className="text-sm font-medium flex items-center gap-2">
                                        <Shield className="w-4 h-4 text-primary" />
                                        Default Admin Email Username
                                    </label>
                                    <Input
                                        required
                                        placeholder="admin@acme.com"
                                        value={newAdminEmail}
                                        onChange={e => setNewAdminEmail(e.target.value)}
                                    />
                                </div>

                                <div className="space-y-2">
                                    <label className="text-sm font-medium">Admin Password (Optional)</label>
                                    <Input
                                        type="password"
                                        placeholder="Leave blank for auto-generated 'admin123'"
                                        value={newAdminPassword}
                                        onChange={e => setNewAdminPassword(e.target.value)}
                                    />
                                </div>

                                <div className="space-y-2 flex items-center gap-2 pt-2">
                                    <input
                                        type="checkbox"
                                        id="provisionVector"
                                        checked={isDedicatedDb}
                                        onChange={e => setIsDedicatedDb(e.target.checked)}
                                        className="w-4 h-4 rounded border-gray-300 text-primary"
                                    />
                                    <label htmlFor="provisionVector" className="text-sm font-medium">Provision Dedicated Vector DB Collection</label>
                                </div>

                                <div className="pt-4">
                                    <Button type="submit" disabled={isCreating || !newTenantName || !newAdminEmail} className="w-full">
                                        {isCreating ? "Provisioning..." : "Provision Tenant"}
                                    </Button>
                                </div>
                            </form>
                        </CardContent>
                    </Card>
                </div>

                <div className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Building2 className="w-5 h-5 text-primary" />
                                Active Tenants
                            </CardTitle>
                            <CardDescription>
                                List of all registered tenants in the platform.
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            {isLoading ? (
                                <p className="text-center text-muted-foreground py-4">Loading tenants...</p>
                            ) : tenants.length === 0 ? (
                                <p className="text-center text-muted-foreground py-4">No tenants found.</p>
                            ) : (
                                <div className="space-y-4">
                                    {tenants.map(t => (
                                        <div key={t.id} className="flex flex-col gap-2 p-4 border rounded-lg bg-card text-card-foreground shadow-sm">
                                            <div className="flex justify-between items-start">
                                                <div>
                                                    <h3 className="font-semibold text-lg">{t.name}</h3>
                                                    <p className="text-xs text-muted-foreground font-mono">{t.id}</p>
                                                </div>
                                                <Button
                                                    variant="destructive"
                                                    size="icon"
                                                    onClick={() => handleDelete(t.id, t.name)}
                                                    title="Delete Tenant"
                                                >
                                                    <Trash2 className="w-4 h-4" />
                                                </Button>
                                            </div>
                                            <div className="text-sm text-muted-foreground">
                                                Created: {t.created_at ? new Date(t.created_at).toLocaleDateString() : 'N/A'}
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    );
}
