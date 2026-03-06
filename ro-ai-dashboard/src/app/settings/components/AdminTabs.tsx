"use client";

import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Layers, Plus, Trash2, Users } from "lucide-react";
import { deleteTenant, deleteUser } from "@/lib/api";
import { SettingsTabProps } from "./types";

export function TenantsTab({ isLoading, allTenants, setShowCreateTenantDialog, loadData }: SettingsTabProps) {
    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between">
                <div>
                    <CardTitle className="flex items-center gap-2"><Layers className="w-5 h-5 text-primary" /> Tenant Management</CardTitle>
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
                                                try { await deleteTenant(t.id); loadData(); } catch { alert("Failed to delete tenant"); }
                                            }}>
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
}

export function UsersTab({ isLoading, allUsers, allTenants, setShowCreateUserDialog, loadData }: SettingsTabProps) {
    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between">
                <div>
                    <CardTitle className="flex items-center gap-2"><Users className="w-5 h-5 text-primary" /> User Management</CardTitle>
                    <CardDescription>Create and manage platform users.</CardDescription>
                </div>
                <Button size="sm" onClick={() => setShowCreateUserDialog(true)}>
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
                                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${u.role === "admin" ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400" : "bg-gray-100 text-gray-600 dark:bg-zinc-800 dark:text-zinc-400"}`}>
                                            {u.role || "viewer"}
                                        </span>
                                    </TableCell>
                                    <TableCell className="text-right">
                                        <Button variant="ghost" size="sm" className="text-red-500 hover:text-red-700"
                                            onClick={async () => {
                                                if (!confirm(`Delete user "${u.username}"?`)) return;
                                                try { await deleteUser(u.id); loadData(); } catch { alert("Failed to delete user"); }
                                            }}>
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
}
