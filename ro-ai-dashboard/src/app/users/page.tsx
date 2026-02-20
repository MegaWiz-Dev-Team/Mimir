"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Plus, UserX, Edit2, Key } from "lucide-react";
// Fake data for UI representation
const MOCK_USERS = [
    { id: "1", username: "admin", role: "admin", tenant: "default_tenant", lastActive: "2 hours ago" },
    { id: "2", username: "john_doe", role: "editor", tenant: "ragnarok_th", lastActive: "1 day ago" },
    { id: "3", username: "jane_smith", role: "viewer", tenant: "med_clinic_a", lastActive: "Just now" },
];

export default function UsersPage() {
    const [users, setUsers] = useState(MOCK_USERS);
    const [isAddMode, setIsAddMode] = useState(false);

    return (
        <div className="container mx-auto p-8">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">User Management</h1>
                    <p className="text-muted-foreground">Manage on-premise users, roles, and tenant assignments.</p>
                </div>
                <Button onClick={() => setIsAddMode(!isAddMode)}>
                    <Plus className="w-4 h-4 mr-2" />
                    Add User
                </Button>
            </div>

            {isAddMode && (
                <Card className="mb-8">
                    <CardHeader>
                        <CardTitle className="text-lg">Create New User</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <form className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 items-end" onSubmit={(e) => { e.preventDefault(); setIsAddMode(false); }}>
                            <div>
                                <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-zinc-300">Username</label>
                                <Input placeholder="Enter username" />
                            </div>
                            <div>
                                <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-zinc-300">Password</label>
                                <Input type="password" placeholder="Temporary password" />
                            </div>
                            <div className="grid gap-1.5">
                                <label className="text-sm font-medium text-gray-700 dark:text-zinc-300">Role</label>
                                <Select defaultValue="viewer">
                                    <SelectTrigger><SelectValue placeholder="Role" /></SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="admin">Admin</SelectItem>
                                        <SelectItem value="editor">Editor</SelectItem>
                                        <SelectItem value="viewer">Viewer</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <Button type="submit" className="w-full">Save User</Button>
                        </form>
                    </CardContent>
                </Card>
            )}

            <Card>
                <CardContent className="p-0">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Username</TableHead>
                                <TableHead>Tenant ID</TableHead>
                                <TableHead>Role</TableHead>
                                <TableHead>Last Active</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {users.map(user => (
                                <TableRow key={user.id}>
                                    <TableCell className="font-medium">{user.username}</TableCell>
                                    <TableCell><span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300">{user.tenant}</span></TableCell>
                                    <TableCell className="capitalize">{user.role}</TableCell>
                                    <TableCell className="text-sm text-gray-500">{user.lastActive}</TableCell>
                                    <TableCell className="text-right whitespace-nowrap">
                                        <Button variant="ghost" size="sm" title="Reset Password" onClick={() => alert("Simulating password reset overlay")}><Key className="w-4 h-4 text-emerald-600" /></Button>
                                        <Button variant="ghost" size="sm" title="Edit Role"><Edit2 className="w-4 h-4 text-blue-600" /></Button>
                                        <Button variant="ghost" size="sm" title="Deactivate"><UserX className="w-4 h-4 text-red-600" /></Button>
                                    </TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>
        </div>
    );
}
