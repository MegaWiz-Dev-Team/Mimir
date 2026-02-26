"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Plus, Globe, FileSpreadsheet, FileText, Database, Settings, Trash2, RefreshCw, Terminal, Eye } from "lucide-react";
import { StatusBadge } from "@/components/ui/status-badge";
import { fetchSources, createSource, deleteSource, syncSource, updateSource, DataSource } from "@/lib/api";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export default function SourcesPage() {
    const [sources, setSources] = useState<DataSource[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAdd, setShowAdd] = useState(false);
    const [deletingId, setDeletingId] = useState<number | null>(null);

    // Add Source Form State
    const [newName, setNewName] = useState("");
    const [newType, setNewType] = useState<'web' | 'tabular' | 'document' | 'mcp'>("web");
    const [newUrl, setNewUrl] = useState("");

    // Streaming Logs Console State
    const [showConsole, setShowConsole] = useState(false);
    const [logs, setLogs] = useState<string[]>([]);
    const [syncingSourceId, setSyncingSourceId] = useState<number | null>(null);

    // Configure Source State
    const [configuringSource, setConfiguringSource] = useState<DataSource | null>(null);
    const [isSaving, setIsSaving] = useState(false);

    // Markdown Preview State
    const [previewingSource, setPreviewingSource] = useState<DataSource | null>(null);

    useEffect(() => {
        loadSources();
    }, []);

    const loadSources = async () => {
        try {
            setLoading(true);
            const data = await fetchSources();
            setSources(data);
        } catch (error) {
            console.warn("[Sources] Failed to fetch sources:", error);
        } finally {
            setLoading(false);
        }
    };

    const handleAddSource = async () => {
        try {
            await createSource({
                name: newName,
                source_type: newType,
                config_json: { url: newUrl },
                schedule: "Manual"
            });
            setShowAdd(false);
            setNewName("");
            setNewUrl("");
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to create source:", error);
            alert("Failed to create source");
        }
    };

    const handleSaveConfig = async () => {
        if (!configuringSource) return;
        setIsSaving(true);
        try {
            await updateSource(configuringSource.id, {
                name: configuringSource.name,
                source_type: configuringSource.source_type,
                config_json: configuringSource.config_json,
                schedule: configuringSource.schedule
            });
            setConfiguringSource(null);
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to update source:", error);
            alert("Failed to update source");
        } finally {
            setIsSaving(false);
        }
    };

    const handleSaveMarkdown = async () => {
        if (!previewingSource) return;
        setIsSaving(true);
        try {
            // Re-use updateSource to save markdown if your API supports updating it
            await updateSource(previewingSource.id, {
                raw_markdown: previewingSource.raw_markdown
            });
            setPreviewingSource(null);
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to save markdown:", error);
            alert("Failed to save markdown");
        } finally {
            setIsSaving(false);
        }
    };

    const handleDelete = async (id: number) => {
        try {
            await deleteSource(id);
            setDeletingId(null);
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to delete source:", error);
            alert("Failed to delete source");
        }
    };

    const handleSync = async (id: number) => {
        try {
            await syncSource(id);
            setSyncingSourceId(id);
            setShowConsole(true);
            setLogs([`> Starting sync job for source #${id}...`, "> Initializing background worker..."]);

            // In a real implementation this would connect to the WebSockets/SSE stream.
            // Simulating stream logs for the UI implementation as per TRD:
            setTimeout(() => setLogs(l => [...l, "> Fetching URL..."]), 1000);
            setTimeout(() => setLogs(l => [...l, "> Parsing DOM..."]), 2000);
            setTimeout(() => {
                setLogs(l => [...l, "> Completed ingestion! Check Vector space."]);
                loadSources();
                setTimeout(() => setShowConsole(false), 2000);
            }, 4000);

        } catch (error) {
            console.warn("[Sources] Failed to sync source:", error);
            alert("Failed to sync source");
        }
    };

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
        <div className="container mx-auto p-8 relative">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Data Ingress Sources</h1>
                    <p className="text-muted-foreground">Manage and configure how data enters your tenant's vector space.</p>
                </div>
                <Button onClick={() => setShowAdd(true)}>
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
                                {loading ? (
                                    <TableRow>
                                        <TableCell colSpan={5} className="text-center py-8">Loading sources...</TableCell>
                                    </TableRow>
                                ) : sources.length === 0 ? (
                                    <TableRow>
                                        <TableCell colSpan={5} className="text-center py-8 text-muted-foreground">No data sources configured yet.</TableCell>
                                    </TableRow>
                                ) : sources.map((s) => (
                                    <TableRow key={s.id}>
                                        <TableCell className="font-medium">
                                            {s.name}
                                            {(s.mb_size != null || s.total_chunks != null) && (
                                                <div className="text-xs text-muted-foreground mt-1 font-normal flex items-center gap-2">
                                                    <span>{s.mb_size?.toFixed(2) || "0.00"} MB</span>
                                                    <span>•</span>
                                                    <span>{s.total_chunks || 0} chunks</span>
                                                </div>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <div className="flex items-center gap-2">
                                                {getTypeIcon(s.source_type)}
                                                <span className="capitalize">{s.source_type}</span>
                                            </div>
                                        </TableCell>
                                        <TableCell className="text-sm text-gray-500">{s.schedule || "Manual"}</TableCell>
                                        <TableCell>
                                            <StatusBadge status={s.last_sync_status || "PENDING"} />
                                        </TableCell>
                                        <TableCell className="text-right">
                                            <Button variant="ghost" size="sm" title="Sync Source" onClick={() => handleSync(s.id!)}>
                                                <RefreshCw className="w-4 h-4" />
                                            </Button>
                                            <Button variant="ghost" size="sm" title="Preview Markdown" onClick={() => setPreviewingSource(s)}>
                                                <Eye className="w-4 h-4" />
                                            </Button>
                                            <Button variant="ghost" size="sm" title="Configure" onClick={() => setConfiguringSource(s)}><Settings className="w-4 h-4" /></Button>
                                            <Button variant="ghost" size="sm" title="Delete Source" className="text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950" onClick={() => setDeletingId(s.id!)}>
                                                <Trash2 className="w-4 h-4" />
                                            </Button>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            </div>

            <Dialog open={showAdd} onOpenChange={setShowAdd}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Add New Data Source</DialogTitle>
                        <DialogDescription className="sr-only">Form to add a new data source</DialogDescription>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid gap-2">
                            <Label htmlFor="name">Source Name</Label>
                            <Input id="name" value={newName} onChange={e => setNewName(e.target.value)} placeholder="e.g. Prontera Wiki" />
                        </div>
                        <div className="grid gap-2">
                            <Label htmlFor="type">Source Type</Label>
                            <select
                                id="type"
                                className="flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                value={newType}
                                onChange={e => setNewType(e.target.value as any)}
                            >
                                <option value="web">Web Scraper</option>
                                <option value="tabular">Tabular (CSV/XLSX)</option>
                                <option value="document">Document (PDF/Image)</option>
                                <option value="mcp">MCP Connection</option>
                            </select>
                        </div>
                        {newType === 'web' && (
                            <div className="grid gap-2">
                                <Label htmlFor="url">Target URL</Label>
                                <Input id="url" value={newUrl} onChange={e => setNewUrl(e.target.value)} placeholder="https://..." />
                            </div>
                        )}
                        {/* Additional fields for other types would go here */}
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowAdd(false)}>Cancel</Button>
                        <Button onClick={handleAddSource} disabled={!newName || (newType === 'web' && !newUrl)}>Create Source</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={showConsole} onOpenChange={setShowConsole}>
                <DialogContent className="max-w-2xl bg-black border-zinc-800 text-green-400 font-mono">
                    <DialogHeader>
                        <DialogTitle className="text-white flex items-center gap-2">
                            <Terminal className="w-4 h-4" /> Ingress Live Console
                        </DialogTitle>
                        <DialogDescription className="text-zinc-400">
                            Streaming logs from background worker for source #{syncingSourceId}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="min-h-[300px] max-h-[500px] overflow-y-auto p-4 rounded bg-zinc-950 border border-zinc-900 shadow-inner">
                        {logs.map((log, i) => (
                            <div key={i} className="mb-1">{log}</div>
                        ))}
                        <div className="animate-pulse">_</div>
                    </div>
                </DialogContent>
            </Dialog>
            <Dialog open={deletingId !== null} onOpenChange={(open) => !open && setDeletingId(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Confirm Deletion</DialogTitle>
                        <DialogDescription>
                            Are you sure you want to delete this data source? This action cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setDeletingId(null)}>Cancel</Button>
                        <Button variant="destructive" className="bg-red-600 text-white hover:bg-red-700" onClick={() => deletingId && handleDelete(deletingId)}>Delete</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={configuringSource !== null} onOpenChange={(open) => !open && setConfiguringSource(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Configure Data Source</DialogTitle>
                        <DialogDescription className="sr-only">Update settings for this data source</DialogDescription>
                    </DialogHeader>
                    {configuringSource && (
                        <div className="grid gap-4 py-4">
                            <div className="grid gap-2">
                                <Label htmlFor="config-name">Source Name</Label>
                                <Input
                                    id="config-name"
                                    value={configuringSource.name}
                                    onChange={e => setConfiguringSource({ ...configuringSource, name: e.target.value })}
                                />
                            </div>
                            <div className="grid gap-2">
                                <Label htmlFor="config-type">Source Type</Label>
                                <Input
                                    id="config-type"
                                    value={configuringSource.source_type}
                                    disabled
                                    className="bg-muted capitalize"
                                />
                            </div>
                            {configuringSource.source_type === 'web' && (
                                <div className="grid gap-2">
                                    <Label htmlFor="config-url">Target URL</Label>
                                    <Input
                                        id="config-url"
                                        value={configuringSource.config_json?.url || ""}
                                        onChange={e => setConfiguringSource({
                                            ...configuringSource,
                                            config_json: { ...configuringSource.config_json, url: e.target.value }
                                        })}
                                    />
                                </div>
                            )}
                            <div className="grid gap-2">
                                <Label htmlFor="config-schedule">Execution Schedule</Label>
                                <Input
                                    id="config-schedule"
                                    value={configuringSource.schedule || ""}
                                    placeholder="e.g. Manual, Daily, 0 * * * *"
                                    onChange={e => setConfiguringSource({ ...configuringSource, schedule: e.target.value })}
                                />
                            </div>
                        </div>
                    )}
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setConfiguringSource(null)}>Cancel</Button>
                        <Button onClick={handleSaveConfig} disabled={isSaving}>
                            {isSaving ? "Saving..." : "Save Changes"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Markdown Preview & Edit Dialog */}
            <Dialog open={previewingSource !== null} onOpenChange={(open) => !open && setPreviewingSource(null)}>
                <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col h-[80vh]">
                    <DialogHeader>
                        <DialogTitle>Markdown Preview {previewingSource?.name ? `- ${previewingSource.name}` : ''}</DialogTitle>
                        <DialogDescription className="text-muted-foreground">
                            Preview and quick-edit the raw markdown extracted from this source.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="flex-1 overflow-hidden py-2">
                        <textarea
                            className="w-full h-full bg-muted/30 p-4 rounded-md font-mono text-sm resize-none focus:outline-none focus:ring-1 border border-border"
                            value={previewingSource?.raw_markdown || ""}
                            onChange={(e) => setPreviewingSource(prev => prev ? { ...prev, raw_markdown: e.target.value } : null)}
                            placeholder="Data is empty or has not been synced yet."
                        />
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setPreviewingSource(null)}>Cancel</Button>
                        <Button onClick={handleSaveMarkdown} disabled={isSaving}>
                            {isSaving ? "Saving..." : "Save Changes"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
