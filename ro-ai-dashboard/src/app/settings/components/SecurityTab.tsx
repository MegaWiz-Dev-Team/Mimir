"use client";

import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Shield, Lock, Users, Plus, Trash2, RefreshCw, Server, Key, RotateCw, CheckCircle2, XCircle, Save } from "lucide-react";
import Cookies from "js-cookie";
import { SettingsTabProps } from "./types";

export function SecurityTab(props: SettingsTabProps) {
    const { config, vaultStatus, vaultSecrets, isVaultLoading, refreshVaultData,
        rotateDialog, setRotateDialog, rotateValue, setRotateValue, isRotating, rotatingKey, handleRotateSecret,
        roles, isRolesLoading, loadRoles, addRoleDialog, setAddRoleDialog, newRoleName, setNewRoleName,
        deleteRoleDialog, setDeleteRoleDialog, hasPendingChanges, isSavingRoles, handleSaveRoles,
        handleAddRole, handleDeleteRole, togglePermission, getEffectivePermission,
        PERMISSION_RESOURCES, PERMISSION_ICONS } = props;

    // Decode current JWT for session info
    const token = Cookies.get("access_token");
    let sessionInfo: Record<string, any> = {};
    if (token) {
        try {
            const parts = token.split(".");
            if (parts.length === 3) sessionInfo = JSON.parse(atob(parts[1]));
        } catch { /* ignore */ }
    }
    const expDate = sessionInfo.exp ? new Date(sessionInfo.exp * 1000) : null;
    const isExpired = expDate ? expDate < new Date() : true;

    const securityFeatures = [
        { label: "Password Hashing", value: "Argon2id", status: true, detail: "Industry-standard memory-hard hashing" },
        { label: "JWT Authentication", value: "HS256 / 24h expiry", status: true, detail: "Stateless token-based auth" },
        { label: "Role-Based Access Control", value: "3-tier RBAC", status: true, detail: "Admin / Editor / Viewer roles" },
        { label: "Tenant Isolation", value: config?.is_dedicated_vector_db ? "Dedicated" : "Shared", status: true, detail: "Data segregation per tenant" },
        { label: "API Authentication", value: "Bearer Token", status: true, detail: "All API routes require valid JWT" },
        { label: "CORS Protection", value: "Configured", status: true, detail: "Cross-origin request restriction" },
    ];

    const recommendations = [
        { text: "Rotate JWT secret periodically", done: false },
        { text: "Use strong passwords (8+ chars, mixed case, numbers)", done: true },
        { text: "Limit admin accounts to necessary personnel", done: true },
        { text: "Enable dedicated vector DB for sensitive tenants", done: config?.is_dedicated_vector_db || false },
        { text: "Review user access permissions regularly", done: false },
        { text: "Configure Heimdall API key in Vault (production)", done: !!config?.llm_config?.heimdall_api_key },
    ];

    const pendingPermChanges = props.hasPendingChanges; // used for display logic

    return (
        <>
            <div className="space-y-6">
                {/* Security Overview */}
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2"><Shield className="w-5 h-5 text-primary" /> Security Overview</CardTitle>
                        <CardDescription>Security features and authentication configuration for this tenant.</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                            {securityFeatures.map((f, i) => (
                                <div key={i} className="rounded-lg border bg-card p-4">
                                    <div className="flex items-center justify-between mb-1">
                                        <span className="text-sm font-medium">{f.label}</span>
                                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${f.status ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' : 'bg-red-100 text-red-800'}`}>
                                            {f.status ? '✓ Active' : '✗ Inactive'}
                                        </span>
                                    </div>
                                    <p className="text-sm font-mono text-primary">{f.value}</p>
                                    <p className="text-xs text-muted-foreground mt-1">{f.detail}</p>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>

                {/* Vault Status Dashboard */}
                <Card>
                    <CardHeader>
                        <div className="flex items-center justify-between">
                            <div>
                                <CardTitle className="flex items-center gap-2"><Server className="w-5 h-5 text-primary" /> Vault Status</CardTitle>
                                <CardDescription>HashiCorp Vault connectivity and secret management.</CardDescription>
                            </div>
                            <Button variant="outline" size="sm" disabled={isVaultLoading} onClick={refreshVaultData}>
                                <RotateCw className={`w-4 h-4 mr-1 ${isVaultLoading ? 'animate-spin' : ''}`} />
                                Health Check
                            </Button>
                        </div>
                    </CardHeader>
                    <CardContent>
                        {vaultStatus ? (
                            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                                <div className="space-y-1">
                                    <p className="text-xs text-muted-foreground">Status</p>
                                    <div className="flex items-center gap-2">
                                        <span className={`w-2.5 h-2.5 rounded-full ${vaultStatus.enabled && vaultStatus.connected ? 'bg-green-500' : vaultStatus.enabled ? 'bg-red-500' : 'bg-gray-400'}`} />
                                        <span className="text-sm font-medium">
                                            {vaultStatus.enabled && vaultStatus.connected ? 'Connected' : vaultStatus.enabled ? 'Disconnected' : 'Not Configured'}
                                        </span>
                                    </div>
                                </div>
                                <div className="space-y-1">
                                    <p className="text-xs text-muted-foreground">Address</p>
                                    <p className="text-sm font-mono truncate">{vaultStatus.addr || '—'}</p>
                                </div>
                                <div className="space-y-1">
                                    <p className="text-xs text-muted-foreground">Sealed</p>
                                    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${vaultStatus.sealed === false ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' :
                                        vaultStatus.sealed === true ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400' :
                                            'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
                                        }`}>
                                        {vaultStatus.sealed === false ? '🔓 Unsealed' : vaultStatus.sealed === true ? '🔒 Sealed' : '—'}
                                    </span>
                                </div>
                                <div className="space-y-1">
                                    <p className="text-xs text-muted-foreground">Version</p>
                                    <p className="text-sm font-mono">{vaultStatus.version || '—'}</p>
                                </div>
                            </div>
                        ) : (
                            <div className="text-center py-6 text-muted-foreground">
                                <Server className="w-8 h-8 mx-auto mb-2 opacity-40" />
                                <p className="text-sm">Click &quot;Health Check&quot; to check Vault connectivity.</p>
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* Managed Secrets Table */}
                {vaultSecrets && (
                    <Card>
                        <CardHeader>
                            <div className="flex items-center justify-between">
                                <div>
                                    <CardTitle className="flex items-center gap-2">
                                        <Key className="w-5 h-5 text-primary" /> Managed Secrets
                                        <span className="text-sm font-normal text-muted-foreground">({vaultSecrets.present_count}/{vaultSecrets.total} present)</span>
                                    </CardTitle>
                                    <CardDescription>Secrets managed by Vault or environment variables.</CardDescription>
                                </div>
                                {vaultSecrets.vault_enabled && (
                                    <Button variant="outline" size="sm" disabled={isVaultLoading} onClick={refreshVaultData}>
                                        <RefreshCw className={`w-4 h-4 mr-1 ${isVaultLoading ? 'animate-spin' : ''}`} /> Re-seed All
                                    </Button>
                                )}
                            </div>
                        </CardHeader>
                        <CardContent>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Secret</TableHead>
                                        <TableHead>Status</TableHead>
                                        <TableHead>Source</TableHead>
                                        <TableHead>Masked Value</TableHead>
                                        {vaultSecrets.vault_enabled && <TableHead className="text-right">Actions</TableHead>}
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {vaultSecrets.secrets.map((s) => (
                                        <TableRow key={s.key}>
                                            <TableCell className="font-mono text-sm">{s.key}</TableCell>
                                            <TableCell>
                                                <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${s.status === 'present'
                                                    ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
                                                    : 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'}`}>
                                                    {s.status === 'present' ? <CheckCircle2 className="w-3 h-3" /> : <XCircle className="w-3 h-3" />}
                                                    {s.status === 'present' ? 'Present' : 'Missing'}
                                                </span>
                                            </TableCell>
                                            <TableCell>
                                                <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${s.source === 'vault' ? 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400' :
                                                    s.source === 'env' ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400' :
                                                        'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'}`}>
                                                    {s.source === 'vault' ? '🔐 Vault' : s.source === 'env' ? '📁 Env' : '—'}
                                                </span>
                                            </TableCell>
                                            <TableCell className="font-mono text-xs text-muted-foreground">{s.masked_value || '—'}</TableCell>
                                            {vaultSecrets.vault_enabled && (
                                                <TableCell className="text-right">
                                                    <Button variant="ghost" size="sm" disabled={rotatingKey === s.key}
                                                        onClick={() => { setRotateDialog({ open: true, key: s.key }); setRotateValue(""); }}>
                                                        <RotateCw className={`w-3.5 h-3.5 mr-1 ${rotatingKey === s.key ? 'animate-spin' : ''}`} /> Rotate
                                                    </Button>
                                                </TableCell>
                                            )}
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                )}

                {/* Active Session */}
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2"><Lock className="w-5 h-5 text-primary" /> Current Session</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">User ID</p>
                                <p className="text-sm font-mono truncate">{sessionInfo.sub || "—"}</p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Tenant</p>
                                <p className="text-sm font-mono">{sessionInfo.tenant_id || "—"}</p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Role</p>
                                <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${sessionInfo.role === 'admin' ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400' :
                                    sessionInfo.role === 'editor' ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400' :
                                        'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-300'}`}>
                                    {(sessionInfo.role || "Unknown").toUpperCase()}
                                </span>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Token Expires</p>
                                <p className={`text-sm font-mono ${isExpired ? 'text-red-600' : 'text-green-600'}`}>
                                    {expDate ? expDate.toLocaleString() : "—"}
                                </p>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                {/* RBAC Roles — Dynamic ACL Matrix */}
                <Card>
                    <CardHeader>
                        <div className="flex items-center justify-between">
                            <div>
                                <CardTitle className="flex items-center gap-2"><Users className="w-5 h-5 text-primary" /> Role Permissions</CardTitle>
                                <CardDescription>Click permission cells on custom roles to toggle between Full / Read / None.</CardDescription>
                            </div>
                            <div className="flex gap-2">
                                <Button variant="outline" size="sm" onClick={loadRoles} disabled={isRolesLoading}>
                                    <RefreshCw className={`w-4 h-4 mr-1 ${isRolesLoading ? 'animate-spin' : ''}`} /> Refresh
                                </Button>
                                <Button variant="outline" size="sm" onClick={() => setAddRoleDialog(true)}>
                                    <Plus className="w-4 h-4 mr-1" /> Add Role
                                </Button>
                                {hasPendingChanges && (
                                    <Button size="sm" onClick={handleSaveRoles} disabled={isSavingRoles}>
                                        <Save className="w-4 h-4 mr-1" /> Save Changes
                                    </Button>
                                )}
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead className="w-[120px]">Role</TableHead>
                                    {PERMISSION_RESOURCES.map(r => (
                                        <TableHead key={r} className="text-center capitalize text-xs">{r}</TableHead>
                                    ))}
                                    <TableHead className="w-[60px]"></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {roles.length === 0 && !isRolesLoading ? (
                                    <TableRow>
                                        <TableCell colSpan={PERMISSION_RESOURCES.length + 2} className="text-center text-muted-foreground py-8">
                                            No roles found. Click &quot;Refresh&quot; to load roles.
                                        </TableCell>
                                    </TableRow>
                                ) : roles.map(role => (
                                    <TableRow key={role.id}>
                                        <TableCell>
                                            <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${role.name === 'admin' ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400' :
                                                role.name === 'editor' ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400' :
                                                    role.name === 'viewer' ? 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-300' :
                                                        'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400'}`}>
                                                {role.name.toUpperCase()}
                                                {role.is_builtin && <Lock className="w-3 h-3 ml-1 opacity-50" />}
                                            </span>
                                        </TableCell>
                                        {PERMISSION_RESOURCES.map(resource => {
                                            const level = getEffectivePermission(role, resource);
                                            return (
                                                <TableCell key={resource}
                                                    className={`text-center ${!role.is_builtin ? 'cursor-pointer hover:bg-muted/50 transition-colors' : ''}`}
                                                    onClick={() => !role.is_builtin && togglePermission(role.id, resource, level)}>
                                                    <span title={level}>{PERMISSION_ICONS[level] || '⛔'}</span>
                                                </TableCell>
                                            );
                                        })}
                                        <TableCell>
                                            {!role.is_builtin && (
                                                <Button variant="ghost" size="sm" className="h-6 w-6 p-0 text-destructive hover:text-destructive"
                                                    onClick={() => setDeleteRoleDialog({ open: true, role })}>
                                                    <Trash2 className="w-3 h-3" />
                                                </Button>
                                            )}
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                        <p className="text-xs text-muted-foreground mt-3">✅ Full access · 👁️ Read-only · ⛔ No access · <Lock className="w-3 h-3 inline" /> Built-in (immutable)</p>
                    </CardContent>
                </Card>

                {/* Security Checklist */}
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2"><Shield className="w-5 h-5 text-primary" /> Security Recommendations</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-3">
                            {recommendations.map((r, i) => (
                                <div key={i} className="flex items-center gap-3">
                                    <div className={`w-5 h-5 rounded-full flex items-center justify-center text-xs ${r.done ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' : 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'}`}>
                                        {r.done ? '✓' : '!'}
                                    </div>
                                    <span className={`text-sm ${r.done ? 'text-muted-foreground' : 'font-medium'}`}>{r.text}</span>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Rotate Secret Dialog */}
            <Dialog open={rotateDialog.open} onOpenChange={(open) => { if (!open) setRotateDialog({ open: false, key: "" }); }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2"><RotateCw className="w-5 h-5" /> Rotate Secret</DialogTitle>
                        <DialogDescription>Enter a new value for <code className="bg-muted px-1 py-0.5 rounded text-sm font-mono">{rotateDialog.key}</code></DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-2">
                        <div className="space-y-2">
                            <Label>New Secret Value</Label>
                            <Input type="password" placeholder="Enter new value..." value={rotateValue}
                                onChange={(e) => setRotateValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleRotateSecret(); }} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setRotateDialog({ open: false, key: "" })}>Cancel</Button>
                        <Button onClick={handleRotateSecret} disabled={isRotating || !rotateValue.trim()}>
                            {isRotating ? <><RotateCw className="w-4 h-4 mr-1 animate-spin" /> Rotating...</> : "Rotate Secret"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Add Role Dialog */}
            <Dialog open={addRoleDialog} onOpenChange={setAddRoleDialog}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Add Custom Role</DialogTitle>
                        <DialogDescription>Create a new role with default &quot;none&quot; permissions. You can edit permissions in the ACL matrix after creation.</DialogDescription>
                    </DialogHeader>
                    <div className="space-y-3 py-2">
                        <Label htmlFor="role-name">Role Name</Label>
                        <Input id="role-name" placeholder="e.g. operator, reviewer..." value={newRoleName}
                            onChange={e => setNewRoleName(e.target.value)} onKeyDown={e => e.key === "Enter" && handleAddRole()} />
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setAddRoleDialog(false)}>Cancel</Button>
                        <Button onClick={handleAddRole} disabled={!newRoleName.trim()}><Plus className="w-4 h-4 mr-1" /> Create Role</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Delete Role Dialog */}
            <Dialog open={deleteRoleDialog.open} onOpenChange={(open) => !open && setDeleteRoleDialog({ open: false, role: null })}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Delete Role</DialogTitle>
                        <DialogDescription>
                            Are you sure you want to delete the role <strong>&quot;{deleteRoleDialog.role?.name}&quot;</strong>? This action cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setDeleteRoleDialog({ open: false, role: null })}>Cancel</Button>
                        <Button variant="destructive" onClick={handleDeleteRole}><Trash2 className="w-4 h-4 mr-1" /> Delete Role</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
