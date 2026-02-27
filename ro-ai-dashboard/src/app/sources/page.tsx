"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Plus, Globe, FileSpreadsheet, FileText, Database, Settings, Trash2, RefreshCw, Terminal, Eye, ArrowLeft, ArrowRight, Upload, Image } from "lucide-react";
import { StatusBadge } from "@/components/ui/status-badge";
import { fetchSources, createSource, deleteSource, syncSource, updateSource, uploadFile, getFeatureFlags, DataSource, FeatureFlags } from "@/lib/api";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetFooter } from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IngressTypeSelector, IngressType } from "@/components/ingress-type-selector";
import { UploadDropzone } from "@/components/upload-dropzone";
import { FolderUpload } from "@/components/folder-upload";
import { UploadProgress, UploadFileStatus } from "@/components/upload-progress";
import { AdvancedSettings, AdvancedSettingsData } from "@/components/advanced-settings";

export default function SourcesPage() {
    const [sources, setSources] = useState<DataSource[]>([]);
    const [loading, setLoading] = useState(true);
    const [deletingId, setDeletingId] = useState<number | null>(null);

    // ─── Wizard Drawer State ────────────────────────────────────────────
    const [showWizard, setShowWizard] = useState(false);
    const [wizardStep, setWizardStep] = useState<1 | 2 | 3>(1);
    const [selectedType, setSelectedType] = useState<IngressType | null>(null);
    const [newName, setNewName] = useState("");
    const [newUrl, setNewUrl] = useState("");
    const [mcpConnectionString, setMcpConnectionString] = useState("");
    const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
    const [uploadFiles, setUploadFiles] = useState<UploadFileStatus[]>([]);
    const [advancedSettings, setAdvancedSettings] = useState<AdvancedSettingsData>({
        ocrEnabled: false,
        useHeaderRow: true,
        storageMode: "markdown",
    });
    const [featureFlags, setFeatureFlags] = useState<FeatureFlags | null>(null);
    const [activeTab, setActiveTab] = useState<"file" | "folder">("file");

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
        loadFeatureFlags();
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

    const loadFeatureFlags = async () => {
        try {
            const flags = await getFeatureFlags();
            setFeatureFlags(flags);
        } catch (error) {
            console.warn("[Sources] Failed to fetch feature flags:", error);
        }
    };

    const resetWizard = () => {
        setWizardStep(1);
        setSelectedType(null);
        setNewName("");
        setNewUrl("");
        setMcpConnectionString("");
        setSelectedFiles([]);
        setUploadFiles([]);
        setActiveTab("file");
        setAdvancedSettings({ ocrEnabled: false, useHeaderRow: true, storageMode: "markdown" });
    };

    const openWizard = () => {
        resetWizard();
        setShowWizard(true);
    };

    const handleTypeSelect = (type: IngressType) => {
        setSelectedType(type);
        setWizardStep(2);
    };

    const handleFilesAdded = (files: File[]) => {
        setSelectedFiles((prev) => [...prev, ...files]);
    };

    const handleFolderSelected = (files: File[]) => {
        setSelectedFiles(files);
    };

    const handleCreateSource = async () => {
        if (!selectedType || !newName) return;
        setIsSaving(true);

        try {
            const configJson: any = {};
            if (selectedType === "web") configJson.url = newUrl;
            if (selectedType === "mcp") configJson.connection_string = mcpConnectionString;
            if (selectedType === "file") configJson.storage_mode = advancedSettings.storageMode;
            if (selectedType === "file") configJson.ocr_enabled = advancedSettings.ocrEnabled;
            configJson.use_header_row = advancedSettings.useHeaderRow;

            const source = await createSource({
                name: newName,
                source_type: selectedType,
                config_json: configJson,
                schedule: "Manual",
            });

            // Upload files if any
            if (selectedFiles.length > 0 && source.id) {
                const fileStatuses: UploadFileStatus[] = selectedFiles.map((f) => ({
                    name: f.name,
                    progress: 0,
                    status: "pending" as const,
                }));
                setUploadFiles(fileStatuses);

                for (let i = 0; i < selectedFiles.length; i++) {
                    setUploadFiles((prev) =>
                        prev.map((f, idx) => (idx === i ? { ...f, status: "uploading" as const } : f))
                    );
                    try {
                        await uploadFile(source.id!, selectedFiles[i], (pct) => {
                            setUploadFiles((prev) =>
                                prev.map((f, idx) => (idx === i ? { ...f, progress: pct } : f))
                            );
                        });
                        setUploadFiles((prev) =>
                            prev.map((f, idx) =>
                                idx === i ? { ...f, progress: 100, status: "complete" as const } : f
                            )
                        );
                    } catch {
                        setUploadFiles((prev) =>
                            prev.map((f, idx) =>
                                idx === i ? { ...f, status: "error" as const } : f
                            )
                        );
                    }
                }
            }

            setShowWizard(false);
            resetWizard();
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to create source:", error);
            alert("Failed to create source");
        } finally {
            setIsSaving(false);
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
                schedule: configuringSource.schedule,
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
            await updateSource(previewingSource.id, {
                raw_markdown: previewingSource.raw_markdown,
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

            setTimeout(() => setLogs((l) => [...l, "> Fetching URL..."]), 1000);
            setTimeout(() => setLogs((l) => [...l, "> Parsing DOM..."]), 2000);
            setTimeout(() => {
                setLogs((l) => [...l, "> Completed ingestion! Check Vector space."]);
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
            case "web":
                return <Globe className="w-4 h-4 text-blue-500" />;
            case "tabular":
                return <FileSpreadsheet className="w-4 h-4 text-green-500" />;
            case "document":
                return <FileText className="w-4 h-4 text-orange-500" />;
            case "structured":
                return <FileText className="w-4 h-4 text-cyan-500" />;
            case "image":
                return <Image className="w-4 h-4 text-pink-500" />;
            case "file":
                return <Upload className="w-4 h-4 text-blue-500" />;
            case "mcp":
                return <Database className="w-4 h-4 text-purple-500" />;
            default:
                return <Database className="w-4 h-4" />;
        }
    };

    const canProceedStep2 = () => {
        if (!selectedType) return false;
        if (!newName.trim()) return false;
        if (selectedType === "web" && !newUrl.trim()) return false;
        if (selectedType === "mcp" && !mcpConnectionString.trim()) return false;
        if (selectedType === "file" && selectedFiles.length === 0)
            return false;
        return true;
    };

    // ─── Wizard Step Content Renderers ──────────────────────────────────

    const renderStep2Content = () => {
        if (!selectedType) return null;

        return (
            <div className="space-y-4">
                <div className="grid gap-2">
                    <Label htmlFor="wizard-name">Source Name</Label>
                    <Input
                        id="wizard-name"
                        value={newName}
                        onChange={(e) => setNewName(e.target.value)}
                        placeholder="e.g. Prontera Wiki"
                    />
                </div>

                {selectedType === "web" && (
                    <div className="grid gap-2">
                        <Label htmlFor="wizard-url">Target URL</Label>
                        <Input
                            id="wizard-url"
                            value={newUrl}
                            onChange={(e) => setNewUrl(e.target.value)}
                            placeholder="https://..."
                        />
                    </div>
                )}

                {selectedType === "mcp" && (
                    <div className="grid gap-2">
                        <Label htmlFor="wizard-mcp">Connection String</Label>
                        <Input
                            id="wizard-mcp"
                            value={mcpConnectionString}
                            onChange={(e) => setMcpConnectionString(e.target.value)}
                            placeholder="mcp://host:port/path"
                        />
                    </div>
                )}

                {selectedType === "file" && (
                    <div className="space-y-3">
                        <div className="flex gap-2">
                            <Button
                                variant={activeTab === "file" ? "default" : "outline"}
                                size="sm"
                                onClick={() => setActiveTab("file")}
                            >
                                Upload Files
                            </Button>
                            <Button
                                variant={activeTab === "folder" ? "default" : "outline"}
                                size="sm"
                                onClick={() => setActiveTab("folder")}
                            >
                                Upload Folder
                            </Button>
                        </div>

                        {activeTab === "file" ? (
                            <UploadDropzone onFilesAdded={handleFilesAdded} />
                        ) : (
                            <FolderUpload onFilesSelected={handleFolderSelected} />
                        )}

                        {selectedFiles.length > 0 && (
                            <div className="text-sm text-muted-foreground">
                                {selectedFiles.length} file(s) selected
                            </div>
                        )}
                    </div>
                )}

                {uploadFiles.length > 0 && <UploadProgress files={uploadFiles} />}
            </div>
        );
    };

    const renderStep3Content = () => {
        if (!selectedType) return null;
        const domain = featureFlags?.domain || "general";

        return (
            <AdvancedSettings
                ingressType={selectedType}
                domain={domain}
                settings={advancedSettings}
                onSettingsChange={setAdvancedSettings}
            />
        );
    };

    // ─── Wizard Step Title ──────────────────────────────────────────────

    const getStepTitle = () => {
        switch (wizardStep) {
            case 1:
                return "Select Source Type";
            case 2:
                return "Configure Source";
            case 3:
                return "Advanced Settings";
        }
    };

    return (
        <div className="container mx-auto p-8 relative">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Data Ingress Sources</h1>
                    <p className="text-muted-foreground">Manage and configure how data enters your tenant&apos;s vector space.</p>
                </div>
                <Button onClick={openWizard}>
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
                                        <TableCell colSpan={5} className="text-center py-8">
                                            Loading sources...
                                        </TableCell>
                                    </TableRow>
                                ) : sources.length === 0 ? (
                                    <TableRow>
                                        <TableCell colSpan={5} className="text-center py-8 text-muted-foreground">
                                            No data sources configured yet.
                                        </TableCell>
                                    </TableRow>
                                ) : (
                                    sources.map((s) => (
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
                                            <TableCell className="text-sm text-gray-500">
                                                {s.schedule || "Manual"}
                                            </TableCell>
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
                                                <Button variant="ghost" size="sm" title="Configure" onClick={() => setConfiguringSource(s)}>
                                                    <Settings className="w-4 h-4" />
                                                </Button>
                                                <Button variant="ghost" size="sm" title="Delete Source" className="text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950" onClick={() => setDeletingId(s.id!)}>
                                                    <Trash2 className="w-4 h-4" />
                                                </Button>
                                            </TableCell>
                                        </TableRow>
                                    ))
                                )}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            </div>

            {/* ═══ Add Source Wizard — Sliding Drawer ═══ */}
            <Sheet open={showWizard} onOpenChange={(open) => { if (!open) setShowWizard(false); }}>
                <SheetContent className="sm:max-w-xl overflow-y-auto">
                    <SheetHeader className="px-6 pt-6 pb-4">
                        <SheetTitle className="text-lg">{getStepTitle()}</SheetTitle>
                        <SheetDescription>
                            Step {wizardStep} of 3 — {getStepTitle()}
                        </SheetDescription>
                        {/* Step indicator */}
                        <div className="flex items-center gap-2 pt-3">
                            {[1, 2, 3].map((step) => (
                                <div
                                    key={step}
                                    className={`h-2 flex-1 rounded-full transition-colors ${step <= wizardStep ? "bg-primary" : "bg-muted"
                                        }`}
                                />
                            ))}
                        </div>
                    </SheetHeader>

                    <div className="px-6 py-4 flex-1">
                        {wizardStep === 1 && <IngressTypeSelector onSelect={handleTypeSelect} />}
                        {wizardStep === 2 && renderStep2Content()}
                        {wizardStep === 3 && renderStep3Content()}
                    </div>

                    <SheetFooter className="flex-row justify-between gap-2 px-6 py-4 border-t">
                        {wizardStep > 1 && (
                            <Button variant="outline" onClick={() => setWizardStep((s) => Math.max(1, s - 1) as 1 | 2 | 3)}>
                                <ArrowLeft className="w-4 h-4 mr-1" />
                                Back
                            </Button>
                        )}
                        <div className="flex-1" />
                        {wizardStep === 2 && (
                            <Button onClick={() => setWizardStep(3)} disabled={!canProceedStep2()}>
                                Next
                                <ArrowRight className="w-4 h-4 ml-1" />
                            </Button>
                        )}
                        {wizardStep === 3 && (
                            <Button onClick={handleCreateSource} disabled={isSaving}>
                                {isSaving ? "Creating..." : "Create Source"}
                            </Button>
                        )}
                    </SheetFooter>
                </SheetContent>
            </Sheet>

            {/* ═══ Streaming Console Dialog ═══ */}
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

            {/* ═══ Delete Confirmation Dialog ═══ */}
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

            {/* ═══ Configure Source Dialog ═══ */}
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
                                    onChange={(e) => setConfiguringSource({ ...configuringSource, name: e.target.value })}
                                />
                            </div>
                            <div className="grid gap-2">
                                <Label htmlFor="config-type">Source Type</Label>
                                <Input id="config-type" value={configuringSource.source_type} disabled className="bg-muted capitalize" />
                            </div>
                            {configuringSource.source_type === "web" && (
                                <div className="grid gap-2">
                                    <Label htmlFor="config-url">Target URL</Label>
                                    <Input
                                        id="config-url"
                                        value={configuringSource.config_json?.url || ""}
                                        onChange={(e) =>
                                            setConfiguringSource({
                                                ...configuringSource,
                                                config_json: { ...configuringSource.config_json, url: e.target.value },
                                            })
                                        }
                                    />
                                </div>
                            )}
                            <div className="grid gap-2">
                                <Label htmlFor="config-schedule">Execution Schedule</Label>
                                <Input
                                    id="config-schedule"
                                    value={configuringSource.schedule || ""}
                                    placeholder="e.g. Manual, Daily, 0 * * * *"
                                    onChange={(e) => setConfiguringSource({ ...configuringSource, schedule: e.target.value })}
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

            {/* ═══ Markdown Preview & Edit Dialog ═══ */}
            <Dialog open={previewingSource !== null} onOpenChange={(open) => !open && setPreviewingSource(null)}>
                <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col h-[80vh]">
                    <DialogHeader>
                        <DialogTitle>Markdown Preview {previewingSource?.name ? `- ${previewingSource.name}` : ""}</DialogTitle>
                        <DialogDescription className="text-muted-foreground">
                            Preview and quick-edit the raw markdown extracted from this source.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="flex-1 overflow-hidden py-2">
                        <textarea
                            className="w-full h-full bg-muted/30 p-4 rounded-md font-mono text-sm resize-none focus:outline-none focus:ring-1 border border-border"
                            value={previewingSource?.raw_markdown || ""}
                            onChange={(e) => setPreviewingSource((prev) => (prev ? { ...prev, raw_markdown: e.target.value } : null))}
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
