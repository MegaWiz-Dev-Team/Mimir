"use client";

import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Building2, Bot, Workflow, Share2, Search, Shield, Users, Layers } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import Cookies from "js-cookie";

import {
    fetchTenants, updateTenant, fetchTenantConfig, updateTenantConfig, createTenant, deleteTenant,
    fetchUsers, createUser, deleteUser, updateUserRole, fetchVaultStatus, fetchVaultSecrets,
    rotateVaultSecret, fetchRoles, createRole, updateRole, deleteRole,
    Tenant, TenantConfig, LlmConfig, LlmSlot, User, CreateTenantRequest, VaultStatus, VaultSecretsResponse, Role
} from "@/lib/api";

// ─── Tab Components ─────────────────────────────────────────────────────────────

import { SettingsTabProps } from "./components/types";
import { GeneralTab } from "./components/GeneralTab";
import { AIModelsTab } from "./components/AIModelsTab";
import { PipelineTab } from "./components/PipelineTab";
import { KnowledgeGraphTab } from "./components/KnowledgeGraphTab";
import { SearchTab } from "./components/SearchTab";
import { SecurityTab } from "./components/SecurityTab";
import { TenantsTab, UsersTab } from "./components/AdminTabs";

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
    const [topK, setTopK] = useState(5);
    const [similarityThreshold, setSimilarityThreshold] = useState(0.7);
    const [searchMode, setSearchMode] = useState("hybrid");

    // Vault state
    const [vaultStatus, setVaultStatus] = useState<VaultStatus | null>(null);
    const [vaultSecrets, setVaultSecrets] = useState<VaultSecretsResponse | null>(null);
    const [isVaultLoading, setIsVaultLoading] = useState(false);
    const [rotateDialog, setRotateDialog] = useState<{ open: boolean; key: string }>({ open: false, key: "" });
    const [rotateValue, setRotateValue] = useState("");
    const [isRotating, setIsRotating] = useState(false);
    const [rotatingKey, setRotatingKey] = useState<string | null>(null);

    // Custom Roles state
    const [roles, setRoles] = useState<Role[]>([]);
    const [isRolesLoading, setIsRolesLoading] = useState(false);
    const [addRoleDialog, setAddRoleDialog] = useState(false);
    const [newRoleName, setNewRoleName] = useState("");
    const [deleteRoleDialog, setDeleteRoleDialog] = useState<{ open: boolean; role: Role | null }>({ open: false, role: null });
    const [pendingPermChanges, setPendingPermChanges] = useState<Record<string, Record<string, string>>>({});
    const [isSavingRoles, setIsSavingRoles] = useState(false);

    const PERMISSION_RESOURCES = ["dashboard", "sources", "knowledge", "pipeline", "chat", "qc", "analytics", "settings", "users", "tenants"];
    const PERMISSION_LEVELS = ["full", "read", "none"];
    const PERMISSION_ICONS: Record<string, string> = { full: "\u2705", read: "\ud83d\udc41\ufe0f", none: "\u26d4" };

    // Tenants & Users state
    const [allTenants, setAllTenants] = useState<Tenant[]>([]);
    const [allUsers, setAllUsers] = useState<User[]>([]);
    const [showCreateTenantDialog, setShowCreateTenantDialog] = useState(false);
    const [showCreateUserDialog, setShowCreateUserDialog] = useState(false);
    const [newTenant, setNewTenant] = useState({ name: "", admin_email: "", admin_password: "", is_dedicated_vector_db: false });
    const [newUser, setNewUser] = useState({ username: "", password: "", tenant_id: "", role: "viewer" });

    // ─── Handlers ───────────────────────────────────────────────────────────────

    const loadRoles = async () => {
        setIsRolesLoading(true);
        try { const data = await fetchRoles(); setRoles(data); setPendingPermChanges({}); }
        catch (e) { console.error("Failed to load roles", e); }
        setIsRolesLoading(false);
    };

    const togglePermission = (roleId: string, resource: string, currentLevel: string) => {
        const idx = PERMISSION_LEVELS.indexOf(currentLevel);
        const next = PERMISSION_LEVELS[(idx + 1) % PERMISSION_LEVELS.length];
        setPendingPermChanges(prev => ({ ...prev, [roleId]: { ...(prev[roleId] || {}), [resource]: next } }));
    };

    const getEffectivePermission = (role: Role, resource: string): string => {
        return pendingPermChanges[role.id]?.[resource] ?? role.permissions[resource] ?? "none";
    };

    const hasPendingChanges = Object.keys(pendingPermChanges).length > 0;

    const handleSaveRoles = async () => {
        setIsSavingRoles(true);
        try {
            for (const [roleId, changes] of Object.entries(pendingPermChanges)) {
                const role = roles.find(r => r.id === roleId);
                if (!role || role.is_builtin) continue;
                await updateRole(roleId, { permissions: { ...role.permissions, ...changes } });
            }
            await loadRoles();
        } catch (e) { console.error("Failed to save roles", e); }
        setIsSavingRoles(false);
    };

    const handleAddRole = async () => {
        if (!newRoleName.trim()) return;
        try {
            const defaultPerms: Record<string, string> = {};
            PERMISSION_RESOURCES.forEach(r => defaultPerms[r] = "none");
            await createRole({ name: newRoleName.trim().toLowerCase(), permissions: defaultPerms });
            setNewRoleName(""); setAddRoleDialog(false); await loadRoles();
        } catch (e) { console.error("Failed to add role", e); }
    };

    const handleDeleteRole = async () => {
        if (!deleteRoleDialog.role) return;
        try { await deleteRole(deleteRoleDialog.role.id); setDeleteRoleDialog({ open: false, role: null }); await loadRoles(); }
        catch (e) { console.error("Failed to delete role", e); }
    };

    const refreshVaultData = async () => {
        setIsVaultLoading(true);
        try { const [status, secrets] = await Promise.all([fetchVaultStatus(), fetchVaultSecrets()]); setVaultStatus(status); setVaultSecrets(secrets); }
        catch (err) { console.warn("[Vault] Refresh failed:", err); }
        finally { setIsVaultLoading(false); }
    };

    const handleRotateSecret = async () => {
        if (!rotateValue.trim()) return;
        setIsRotating(true);
        try { await rotateVaultSecret(rotateDialog.key, rotateValue); setRotateDialog({ open: false, key: "" }); setRotateValue(""); await refreshVaultData(); }
        catch (err) { console.error("[Vault] Rotate failed:", err); }
        finally { setIsRotating(false); }
    };

    const loadData = async () => {
        setIsLoading(true);
        try {
            const tenantsData = await fetchTenants();
            setTenants(tenantsData); setAllTenants(tenantsData);
            if (tenantsData.length > 0) {
                const activeTenantId = Cookies.get("tenant_id") || tenantsData[0].id;
                const activeTenant = tenantsData.find(t => t.id === activeTenantId) || tenantsData[0];
                setCurrentTenantId(activeTenant.id); setTenantName(activeTenant.name);
                try {
                    const configData = await fetchTenantConfig(activeTenant.id);
                    setConfig(configData);
                    if (configData.search_settings) {
                        setTopK(configData.search_settings.top_k || 5);
                        setSimilarityThreshold(configData.search_settings.similarity_threshold || 0.7);
                        setSearchMode(configData.search_settings.search_mode || "hybrid");
                        // Fallback: older records might still have chunk settings here before migration
                        if (!configData.pipeline_settings?.chunk_strategy && (configData.search_settings as any).chunk_strategy) {
                            setChunkStrategy((configData.search_settings as any).chunk_strategy);
                            setChunkSize((configData.search_settings as any).chunk_size || 512);
                            setChunkOverlap((configData.search_settings as any).chunk_overlap || 50);
                            setDedupThreshold((configData.search_settings as any).dedup_threshold || 0);
                        }
                    }
                    if (configData.pipeline_settings) {
                        if (configData.pipeline_settings.chunk_strategy) setChunkStrategy(configData.pipeline_settings.chunk_strategy);
                        if (configData.pipeline_settings.chunk_size) setChunkSize(configData.pipeline_settings.chunk_size);
                        if (configData.pipeline_settings.chunk_overlap !== undefined) setChunkOverlap(configData.pipeline_settings.chunk_overlap);
                        if (configData.pipeline_settings.dedup_threshold !== undefined) setDedupThreshold(configData.pipeline_settings.dedup_threshold);
                    }
                } catch (err) { console.warn("[Settings] Failed to load tenant config:", err); }
            }
            try { const usersData = await fetchUsers(); setAllUsers(usersData); }
            catch (err) { console.warn("[Settings] Failed to load users:", err); }
        } catch (error) { console.warn("[Settings] Failed to load tenants:", error); }
        finally { setIsLoading(false); }
    };

    // B1: Tab-specific save functions to prevent cross-tab overwrites
    const handleSave = async (e: React.FormEvent) => {
        // General tab: save only tenant name
        e.preventDefault();
        if (!currentTenantId) return;
        setIsSaving(true);
        try { await updateTenant(currentTenantId, tenantName); alert("Tenant name updated."); }
        catch { alert("Failed to update tenant name."); }
        finally { setIsSaving(false); }
    };

    const handleSaveAIModels = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!currentTenantId || !config) return;
        setIsSaving(true);
        try {
            await updateTenantConfig(currentTenantId, {
                default_provider: config.default_provider,
                default_model: config.default_model,
                llm_config: config.llm_config,
                system_prompt: config.system_prompt,
                max_daily_tokens: config.max_daily_tokens,
                is_dedicated_vector_db: config.is_dedicated_vector_db,
            });
            alert("AI model settings saved."); loadData();
        }
        catch { alert("Failed to save AI model settings."); }
        finally { setIsSaving(false); }
    };

    const handleSaveCredentials = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!currentTenantId || !config) return;
        setIsSaving(true);
        try {
            await updateTenantConfig(currentTenantId, {
                llm_config: config.llm_config,
            });
            alert("Credentials saved."); loadData();
        }
        catch { alert("Failed to save credentials."); }
        finally { setIsSaving(false); }
    };

    useEffect(() => { loadData(); }, []);

    // ─── Shared Props ───────────────────────────────────────────────────────────

    const tabProps: SettingsTabProps = {
        isLoading, isSaving, config, setConfig, currentTenantId, tenantName, setTenantName, handleSave,
        handleSaveAIModels, handleSaveCredentials,
        chunkStrategy, setChunkStrategy, chunkSize, setChunkSize, chunkOverlap, setChunkOverlap, dedupThreshold, setDedupThreshold,
        updateTenantConfigFn: updateTenantConfig,
        topK, setTopK, similarityThreshold, setSimilarityThreshold, searchMode, setSearchMode,
        vaultStatus, vaultSecrets, isVaultLoading, refreshVaultData, rotateDialog, setRotateDialog, rotateValue, setRotateValue, isRotating, rotatingKey, handleRotateSecret,
        roles, isRolesLoading, loadRoles, addRoleDialog, setAddRoleDialog, newRoleName, setNewRoleName,
        deleteRoleDialog, setDeleteRoleDialog, hasPendingChanges, isSavingRoles, handleSaveRoles, handleAddRole, handleDeleteRole,
        togglePermission, getEffectivePermission, PERMISSION_RESOURCES, PERMISSION_LEVELS, PERMISSION_ICONS,
        allTenants, allUsers, showCreateTenantDialog, setShowCreateTenantDialog, showCreateUserDialog, setShowCreateUserDialog, loadData,
    };

    // ─── Tab Content Router ─────────────────────────────────────────────────────

    const renderTabContent = () => {
        switch (activeTab) {
            case "general": return <GeneralTab {...tabProps} />;
            case "ai-models": return <AIModelsTab {...tabProps} />;
            case "pipeline": return <PipelineTab {...tabProps} />;
            case "knowledge-graph": return <KnowledgeGraphTab {...tabProps} />;
            case "search": return <SearchTab {...tabProps} />;
            case "security": return <SecurityTab {...tabProps} />;
            case "tenants": return <TenantsTab {...tabProps} />;
            case "users": return <UsersTab {...tabProps} />;
            default: return null;
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
                <nav className="w-56 shrink-0">
                    <div className="space-y-1">
                        {TABS.map((tab) => {
                            const Icon = tab.icon;
                            const isActive = activeTab === tab.id;
                            return (
                                <button key={tab.id} onClick={() => setActiveTab(tab.id)}
                                    className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all ${isActive
                                        ? "bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 shadow-sm"
                                        : "text-gray-600 hover:bg-gray-50 dark:text-zinc-400 dark:hover:bg-zinc-800/50"}`}>
                                    <Icon className={`w-4 h-4 ${isActive ? "text-blue-600 dark:text-blue-400" : ""}`} />
                                    {tab.label}
                                </button>
                            );
                        })}
                    </div>
                </nav>

                <div className="flex-1 max-w-3xl">{renderTabContent()}</div>
            </div>

            {/* ═══ Create Tenant Dialog ═══ */}
            <Dialog open={showCreateTenantDialog} onOpenChange={setShowCreateTenantDialog}>
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Create New Tenant</DialogTitle>
                        <DialogDescription>Create a new organization tenant with an admin user.</DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-2">
                        <div className="space-y-2"><Label>Tenant Name</Label><Input placeholder="e.g. MegaCare" value={newTenant.name} onChange={e => setNewTenant({ ...newTenant, name: e.target.value })} /></div>
                        <div className="space-y-2"><Label>Admin Username</Label><Input placeholder="e.g. admin@megacare.com" value={newTenant.admin_email} onChange={e => setNewTenant({ ...newTenant, admin_email: e.target.value })} /></div>
                        <div className="space-y-2"><Label>Admin Password</Label><Input type="password" placeholder="Min 6 characters" value={newTenant.admin_password} onChange={e => setNewTenant({ ...newTenant, admin_password: e.target.value })} /></div>
                        <div className="flex items-center gap-2">
                            <input type="checkbox" id="dedicatedVdb" checked={newTenant.is_dedicated_vector_db} onChange={e => setNewTenant({ ...newTenant, is_dedicated_vector_db: e.target.checked })} className="w-4 h-4" />
                            <Label htmlFor="dedicatedVdb" className="text-sm">Dedicated Vector DB</Label>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreateTenantDialog(false)}>Cancel</Button>
                        <Button disabled={!newTenant.name || !newTenant.admin_email} onClick={async () => {
                            try { await createTenant(newTenant as CreateTenantRequest); setShowCreateTenantDialog(false); setNewTenant({ name: "", admin_email: "", admin_password: "", is_dedicated_vector_db: false }); loadData(); }
                            catch { alert("Failed to create tenant"); }
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
                        <div className="space-y-2"><Label>Username</Label><Input placeholder="e.g. john_doe" value={newUser.username} onChange={e => setNewUser({ ...newUser, username: e.target.value })} /></div>
                        <div className="space-y-2"><Label>Password</Label><Input type="password" placeholder="Min 6 characters" value={newUser.password} onChange={e => setNewUser({ ...newUser, password: e.target.value })} /></div>
                        <div className="space-y-2">
                            <Label>Tenant</Label>
                            <select className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" value={newUser.tenant_id} onChange={e => setNewUser({ ...newUser, tenant_id: e.target.value })}>
                                {allTenants.map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
                            </select>
                        </div>
                        <div className="space-y-2">
                            <Label>Role</Label>
                            <select className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" value={newUser.role} onChange={e => setNewUser({ ...newUser, role: e.target.value })}>
                                <option value="admin">Admin</option><option value="editor">Editor</option><option value="viewer">Viewer</option>
                            </select>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreateUserDialog(false)}>Cancel</Button>
                        <Button disabled={!newUser.username || !newUser.tenant_id} onClick={async () => {
                            try { await createUser({ username: newUser.username, password: newUser.password || undefined, tenant_id: newUser.tenant_id, role: newUser.role }); setShowCreateUserDialog(false); setNewUser({ username: "", password: "", tenant_id: allTenants[0]?.id || "", role: "viewer" }); loadData(); }
                            catch { alert("Failed to create user"); }
                        }}>Create</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
