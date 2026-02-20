"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Plus, Globe, FileSpreadsheet, FileText, Database, Settings, Trash2 } from "lucide-react";
import { StatusBadge } from "@/components/ui/status-badge";

const MOCK_SOURCES = [
    { id: 1, name: "Prontera Wiki (Web)", type: "web", schedule: "Daily at 00:00", status: "COMPLETED" },
    { id: 2, name: "Monster Drops (CSV)", type: "tabular", schedule: "Manual", status: "PENDING" },
    { id: 3, name: "Guild Chat Logs", type: "mcp", schedule: "Hourly", status: "FAILED" },
];

export default function SourcesPage() {
    const [sources, setSources] = useState(MOCK_SOURCES);
    const [showAdd, setShowAdd] = useState(false);

    const getTypeIcon = (type: string) => {
        switch (type) {
            case 'web': return <Globe className="w-4 h-4 text-blue-500" />;
            case 'tabular': return <FileSpreadsheet className="w-4 h-4 text-green-500" />;
            case 'document': return <FileText className="w-4 h-4 text-orange-500" />;
            case 'mcp': return <Database className="w-4 h-4 text-purple-500" />;
            default: return <Database className="w-4 h-4" />;
        }
    };

    return (
        <div className="container mx-auto p-8">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Data Ingress Sources</h1>
                    <p className="text-muted-foreground">Manage and configure how data enters your tenant's vector space.</p>
                </div>
                <Button onClick={() => alert("Open Add Source Dialog")}>
                    <Plus className="w-4 h-4 mr-2" />
                    Add Source
                </Button>
            </div>

            <div className="grid gap-6">
                <Card>
                    <CardContent className="p-0">
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Source Name</TableHead>
                                    <TableHead>Type</TableHead>
                                    <TableHead>Execution Schedule</TableHead>
                                    <TableHead>Last Sync Status</TableHead>
                                    <TableHead className="text-right">Actions</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {sources.map((s) => (
                                    <TableRow key={s.id}>
                                        <TableCell className="font-medium">{s.name}</TableCell>
                                        <TableCell>
                                            <div className="flex items-center gap-2">
                                                {getTypeIcon(s.type)}
                                                <span className="capitalize">{s.type}</span>
                                            </div>
                                        </TableCell>
                                        <TableCell className="text-sm text-gray-500">{s.schedule}</TableCell>
                                        <TableCell>
                                            <StatusBadge status={s.status} />
                                        </TableCell>
                                        <TableCell className="text-right">
                                            <Button variant="ghost" size="sm" title="Configure"><Settings className="w-4 h-4" /></Button>
                                            <Button variant="ghost" size="sm" title="Delete Source" className="text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950"><Trash2 className="w-4 h-4" /></Button>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}
