"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, DropdownMenuSeparator, DropdownMenuLabel } from "@/components/ui/dropdown-menu";
import { Plus, UserX, Edit2, Key, Search, MoreHorizontal } from "lucide-react";

import { fetchUsers, fetchTenants, createUser, updateUserRole, updateUserPassword, deleteUser, User, Tenant } from "@/lib/api";

export default function UsersPage() {
    const [users, setUsers] = useState<User[]>([]);
    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [searchQuery, setSearchQuery] = useState("");
    const [tenantFilter, setTenantFilter] = useState("all");
    const [isLoading, setIsLoading] = useState(true);

    // Dialog States
    const [isAddMode, setIsAddMode] = useState(false);
    const [isEditRoleMode, setIsEditRoleMode] = useState(false);
    const [isResetPasswordMode, setIsResetPasswordMode] = useState(false);
    const [selectedUser, setSelectedUser] = useState<User | null>(null);

    // Form States
    const [formUsername, setFormUsername] = useState("");
    const [formPassword, setFormPassword] = useState("");
    const [formTenantId, setFormTenantId] = useState("");
    const [formRole, setFormRole] = useState("VIEWER");

    useEffect(() => {
        loadData();
    }, []);

    const loadData = async () => {
        setIsLoading(true);
        try {
            const [usersData, tenantsData] = await Promise.all([
                fetchUsers(),
                fetchTenants()
            ]);
            setUsers(usersData);
            setTenants(tenantsData);
        } catch (error) {
            console.warn("[Users] Failed to load users:", error);
            alert("Failed to load users. Are you logged in as Admin?");
        } finally {
            setIsLoading(false);
        }
    };

    const handleCreateUser = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            await createUser({
                username: formUsername,
                password: formPassword || undefined,
                tenant_id: formTenantId,
                role: formRole
            });
            setIsAddMode(false);
            setFormUsername("");
            setFormPassword("");
            loadData();
        } catch (error) {
            console.warn("[Users]", error);
            alert("Failed to create user.");
        }
    };

    const handleEditRole = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedUser) return;
        try {
            await updateUserRole(selectedUser.id, {
                tenant_id: formTenantId,
                role: formRole
            });
            setIsEditRoleMode(false);
            loadData();
        } catch (error) {
            console.warn("[Users]", error);
            alert("Failed to update user role.");
        }
    };

    const handleResetPassword = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedUser) return;
        try {
            await updateUserPassword(selectedUser.id, formPassword);
            setIsResetPasswordMode(false);
            setFormPassword("");
            alert("Password updated successfully.");
        } catch (error) {
            console.warn("[Users]", error);
            alert("Failed to reset password.");
        }
    };

    const handleDeleteUser = async (user: User) => {
        if (!confirm(`Are you sure you want to deactivate ${user.username}? This cannot be undone.`)) return;
        try {
            await deleteUser(user.id);
            loadData();
        } catch (error) {
            console.warn("[Users]", error);
            alert("Failed to delete user.");
        }
    };

    const openEditRole = (user: User) => {
        setSelectedUser(user);
        setFormTenantId(user.tenant_id || "");
        setFormRole(user.role || "VIEWER");
        setIsEditRoleMode(true);
    };

    const openResetPassword = (user: User) => {
        setSelectedUser(user);
        setFormPassword("");
        setIsResetPasswordMode(true);
    };

    const filteredUsers = users.filter(user => {
        const matchesSearch = user.username.toLowerCase().includes(searchQuery.toLowerCase());
        const matchesTenant = tenantFilter === "all" || user.tenant_id === tenantFilter;
        return matchesSearch && matchesTenant;
    });

    const getRoleBadgeClass = (role: string | null) => {
        switch (role?.toLowerCase()) {
            case "admin": return "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300";
            case "editor": return "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300";
            default: return "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-300";
        }
    };

    return (
        <div className="container mx-auto p-8">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">User Management</h1>
                    <p className="text-muted-foreground">Manage on-premise users, roles, and tenant assignments.</p>
                </div>
                <Button onClick={() => {
                    setFormUsername("");
                    setFormPassword("");
                    setFormRole("VIEWER");
                    setFormTenantId(tenants[0]?.id || "");
                    setIsAddMode(true);
                }}>
                    <Plus className="w-4 h-4 mr-2" />
                    Add User
                </Button>
            </div>

            <div className="flex flex-col sm:flex-row gap-4 mb-6">
                <div className="relative flex-1">
                    <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-gray-500" />
                    <Input
                        placeholder="Search by username..."
                        className="pl-9"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>
                <Select value={tenantFilter} onValueChange={setTenantFilter}>
                    <SelectTrigger className="w-full sm:w-[200px]">
                        <SelectValue placeholder="Filter by Tenant" />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="all">All Tenants</SelectItem>
                        {tenants.map(t => (
                            <SelectItem key={t.id} value={t.id}>{t.name}</SelectItem>
                        ))}
                    </SelectContent>
                </Select>
            </div>

            <Card>
                <CardContent className="p-0">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Username</TableHead>
                                <TableHead>Tenant ID</TableHead>
                                <TableHead>Role</TableHead>
                                <TableHead>Created At</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {isLoading ? (
                                <TableRow>
                                    <TableCell colSpan={5} className="text-center py-8 text-muted-foreground">Loading users...</TableCell>
                                </TableRow>
                            ) : filteredUsers.length === 0 ? (
                                <TableRow>
                                    <TableCell colSpan={5} className="text-center py-8 text-muted-foreground">No users found.</TableCell>
                                </TableRow>
                            ) : (
                                filteredUsers.map(user => (
                                    <TableRow key={user.id}>
                                        <TableCell className="font-medium">{user.username}</TableCell>
                                        <TableCell>
                                            <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-800 dark:bg-emerald-900 dark:text-emerald-300">
                                                {user.tenant_id || "Unassigned"}
                                            </span>
                                        </TableCell>
                                        <TableCell>
                                            <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${getRoleBadgeClass(user.role)}`}>
                                                {(user.role || "Unknown").toUpperCase()}
                                            </span>
                                        </TableCell>
                                        <TableCell className="text-sm text-gray-500">
                                            {user.created_at ? new Date(user.created_at).toLocaleDateString() : "-"}
                                        </TableCell>
                                        <TableCell className="text-right">
                                            <DropdownMenu>
                                                <DropdownMenuTrigger asChild>
                                                    <Button variant="ghost" className="h-8 w-8 p-0">
                                                        <span className="sr-only">Open menu</span>
                                                        <MoreHorizontal className="h-4 w-4" />
                                                    </Button>
                                                </DropdownMenuTrigger>
                                                <DropdownMenuContent align="end">
                                                    <DropdownMenuLabel>Actions</DropdownMenuLabel>
                                                    <DropdownMenuItem onClick={() => openEditRole(user)}>
                                                        <Edit2 className="w-4 h-4 mr-2" /> Change Role / Tenant
                                                    </DropdownMenuItem>
                                                    <DropdownMenuItem onClick={() => openResetPassword(user)}>
                                                        <Key className="w-4 h-4 mr-2" /> Reset Password
                                                    </DropdownMenuItem>
                                                    <DropdownMenuSeparator />
                                                    <DropdownMenuItem onClick={() => handleDeleteUser(user)} className="text-red-600 focus:text-red-600 focus:bg-red-50 dark:focus:bg-red-950">
                                                        <UserX className="w-4 h-4 mr-2" /> Deactivate User
                                                    </DropdownMenuItem>
                                                </DropdownMenuContent>
                                            </DropdownMenu>
                                        </TableCell>
                                    </TableRow>
                                ))
                            )}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>

            {/* Add User Dialog */}
            <Dialog open={isAddMode} onOpenChange={setIsAddMode}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Create New User</DialogTitle>
                        <DialogDescription>
                            Create a new on-premise user and assign them to a tenant.
                        </DialogDescription>
                    </DialogHeader>
                    <form onSubmit={handleCreateUser}>
                        <div className="grid gap-4 py-4">
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Username</label>
                                <Input required placeholder="Enter username" value={formUsername} onChange={e => setFormUsername(e.target.value)} autoComplete="new-username" />
                            </div>
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Temporary Password</label>
                                <Input placeholder="Leave blank to auto-generate" type="password" value={formPassword} onChange={e => setFormPassword(e.target.value)} autoComplete="new-password" />
                            </div>
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Tenant</label>
                                <Select value={formTenantId} onValueChange={setFormTenantId} required>
                                    <SelectTrigger><SelectValue placeholder="Select Tenant" /></SelectTrigger>
                                    <SelectContent>
                                        {tenants.map(t => <SelectItem key={t.id} value={t.id}>{t.name}</SelectItem>)}
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Role</label>
                                <Select value={formRole} onValueChange={setFormRole} required>
                                    <SelectTrigger><SelectValue placeholder="Select Role" /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="ADMIN">Admin</SelectItem>
                                        <SelectItem value="EDITOR">Editor</SelectItem>
                                        <SelectItem value="VIEWER">Viewer</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                        </div>
                        <DialogFooter>
                            <Button type="button" variant="outline" onClick={() => setIsAddMode(false)}>Cancel</Button>
                            <Button type="submit">Save User</Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>

            {/* Edit Role/Tenant Dialog */}
            <Dialog open={isEditRoleMode} onOpenChange={setIsEditRoleMode}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Change Role & Tenant</DialogTitle>
                        <DialogDescription>
                            Update assignment for {selectedUser?.username}.
                        </DialogDescription>
                    </DialogHeader>
                    <form onSubmit={handleEditRole}>
                        <div className="grid gap-4 py-4">
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Tenant</label>
                                <Select value={formTenantId} onValueChange={setFormTenantId} required>
                                    <SelectTrigger><SelectValue placeholder="Select Tenant" /></SelectTrigger>
                                    <SelectContent>
                                        {tenants.map(t => <SelectItem key={t.id} value={t.id}>{t.name}</SelectItem>)}
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">Role</label>
                                <Select value={formRole} onValueChange={setFormRole} required>
                                    <SelectTrigger><SelectValue placeholder="Select Role" /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="admin">Admin</SelectItem>
                                        <SelectItem value="editor">Editor</SelectItem>
                                        <SelectItem value="viewer">Viewer</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                        </div>
                        <DialogFooter>
                            <Button type="button" variant="outline" onClick={() => setIsEditRoleMode(false)}>Cancel</Button>
                            <Button type="submit">Update</Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>

            {/* Reset Password Dialog */}
            <Dialog open={isResetPasswordMode} onOpenChange={setIsResetPasswordMode}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Reset Password</DialogTitle>
                        <DialogDescription>
                            Set a new temporary password for {selectedUser?.username}.
                        </DialogDescription>
                    </DialogHeader>
                    <form onSubmit={handleResetPassword}>
                        <div className="grid gap-4 py-4">
                            <div className="grid gap-2">
                                <label className="text-sm font-medium">New Password</label>
                                <Input required placeholder="Enter new password" type="password" value={formPassword} onChange={e => setFormPassword(e.target.value)} />
                            </div>
                        </div>
                        <DialogFooter>
                            <Button type="button" variant="outline" onClick={() => setIsResetPasswordMode(false)}>Cancel</Button>
                            <Button type="submit">Reset Password</Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>

        </div>
    );
}
