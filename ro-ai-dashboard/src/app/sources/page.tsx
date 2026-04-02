"use client";

import React, { useState, useEffect, useMemo } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Plus, Globe, FileSpreadsheet, FileText, Database, Settings, Trash2, RefreshCw, Terminal, Eye, ArrowLeft, ArrowRight, Upload, Image, X, Sparkles, Loader2, Search, CheckSquare, Square, ChevronRight, ChevronDown } from "lucide-react";
import { StatusBadge } from "@/components/ui/status-badge";
import { fetchSources, fetchSource, createSource, deleteSource, syncSource, updateSource, uploadFile, getFeatureFlags, fetchModels, extractWithAi, discoverHierarchy, importPages, runAutoPipeline, generatePageIndexTree, triggerGraphExtraction, fetchPipelineStatus, DataSource, FeatureFlags, ModelConfig, HierarchyNode } from "@/lib/api";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetFooter } from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IngressTypeSelector, IngressType } from "@/components/ingress-type-selector";
import { UploadDropzone } from "@/components/upload-dropzone";
import { FolderUpload } from "@/components/folder-upload";
import { UploadProgress, UploadFileStatus } from "@/components/upload-progress";
import { AdvancedSettings, AdvancedSettingsData } from "@/components/advanced-settings";
import { DbConnectorWizard } from "@/components/db-connector-wizard";
import { CronScheduleSelector, ScheduleOption } from "@/components/cron-schedule-selector";

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
    
    // Auto-Pipeline States
    const [pipelineConfigSource, setPipelineConfigSource] = useState<DataSource | null>(null);
    const [pipelineSelectedProvider, setPipelineSelectedProvider] = useState("google");
    const [pipelineSelectedModel, setPipelineSelectedModel] = useState("");
    const [pipelineEnablePageIndex, setPipelineEnablePageIndex] = useState(false);
    const [pipelineSkipKg, setPipelineSkipKg] = useState(false);
    const [pipelineStarting, setPipelineStarting] = useState(false);
    const [pipelineRunStatus, setPipelineRunStatus] = useState<any>(null);

    // Markdown Preview State
    const [previewingSource, setPreviewingSource] = useState<DataSource | null>(null);

    // AI Extraction State
    const [aiModels, setAiModels] = useState<ModelConfig[]>([]);
    const [aiSelectedModel, setAiSelectedModel] = useState("");
    const [aiOutputFormat, setAiOutputFormat] = useState<"markdown" | "table">("markdown");
    const [aiExtracting, setAiExtracting] = useState(false);
    const [aiTokensUsed, setAiTokensUsed] = useState<number | null>(null);
    const [aiError, setAiError] = useState<string | null>(null);

    // Hierarchy Discovery State
    const [hierarchyPages, setHierarchyPages] = useState<HierarchyNode[]>([]);
    const [hierarchyLoading, setHierarchyLoading] = useState(false);
    const [selectedPageUrls, setSelectedPageUrls] = useState<Set<string>>(new Set());
    const [hierarchyDiscovered, setHierarchyDiscovered] = useState(false);
    const [importingPages, setImportingPages] = useState(false);

    // DB Connector Wizard State
    const [showDbWizard, setShowDbWizard] = useState(false);

    // B2: Source grouping — collapse state from localStorage
    const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(() => {
        if (typeof window !== "undefined") {
            try {
                const stored = localStorage.getItem("mimir_source_groups_collapsed");
                return stored ? new Set(JSON.parse(stored)) : new Set();
            } catch { return new Set(); }
        }
        return new Set();
    });

    useEffect(() => {
        loadSources();
        loadFeatureFlags();
        fetchModels()
            .then(models => {
                setAiModels(models);
                if (models.length > 0) {
                    setPipelineSelectedModel(models[0].model_id);
                }
            })
            .catch(console.error);
    }, []);

    // B2: Group sources by type
    const TYPE_LABELS: Record<string, string> = { file: "📁 File", web: "🌐 Web", mcp: "🔌 MCP", db: "🗃️ Database", folder: "📂 Folder" };
    const groupedSources = useMemo(() => {
        const groups: Record<string, typeof sources> = {};
        for (const s of sources) {
            const t = s.source_type || "file";
            if (!groups[t]) groups[t] = [];
            groups[t].push(s);
        }
        return Object.entries(groups).sort(([a], [b]) => a.localeCompare(b));
    }, [sources]);

    const toggleGroup = (type: string) => {
        setCollapsedGroups((prev) => {
            const next = new Set(prev);
            if (next.has(type)) next.delete(type); else next.add(type);
            localStorage.setItem("mimir_source_groups_collapsed", JSON.stringify([...next]));
            return next;
        });
    };;

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
        setHierarchyPages([]);
        setSelectedPageUrls(new Set());
        setHierarchyDiscovered(false);
        setImportingPages(false);
    };

    const openWizard = () => {
        resetWizard();
        setShowWizard(true);
    };

    const handleTypeSelect = (type: IngressType) => {
        setSelectedType(type);
        setWizardStep(2);
    };

    // A3: Filter hidden/system files (.DS_Store, __MACOSX, Thumbs.db, dotfiles)
    const HIDDEN_PATTERNS = ['.DS_Store', '.gitkeep', 'Thumbs.db', 'desktop.ini'];
    const filterHiddenFiles = (files: File[]) =>
        files.filter(f =>
            !f.name.startsWith('.') &&
            !HIDDEN_PATTERNS.includes(f.name) &&
            !f.webkitRelativePath?.includes('__MACOSX')
        );

    const handleFilesAdded = (files: File[]) => {
        setSelectedFiles((prev) => [...prev, ...filterHiddenFiles(files)]);
    };

    const handleFolderSelected = (files: File[]) => {
        setSelectedFiles(filterHiddenFiles(files));
    };

    const handleRemoveFile = (index: number) => {
        setSelectedFiles((prev) => prev.filter((_, i) => i !== index));
    };

    const handleClearFiles = () => {
        setSelectedFiles([]);
    };

    const handleCreateSource = async () => {
        if (!selectedType || !newName) return;
        setIsSaving(true);

        try {
            // A2 Fix: For file sources, use /sources/upload directly
            // (creates source record + uploads to S3 + triggers extraction)
            if (selectedType === "file" && selectedFiles.length > 0) {
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
                        await uploadFile(null, selectedFiles[i], (pct) => {
                            setUploadFiles((prev) =>
                                prev.map((f, idx) => (idx === i ? { ...f, progress: pct } : f))
                            );
                        }, {
                            name: selectedFiles.length === 1 ? newName : `${newName} — ${selectedFiles[i].name}`,
                            source_type: "file",
                            folder_path: (selectedFiles[i] as any).webkitRelativePath || "",
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
            } else {
                // Non-file sources: create via JSON API
                const configJson: any = {};
                if (selectedType === "web") configJson.url = newUrl;
                if (selectedType === "mcp") {
                    configJson.mcp_url = mcpConnectionString;
                    configJson.transport_type = "sse";
                }
                configJson.use_header_row = advancedSettings.useHeaderRow;

                await createSource({
                    name: newName,
                    source_type: selectedType,
                    config_json: configJson,
                    schedule: "Manual",
                });
            }

            setShowWizard(false);
            resetWizard();
            loadSources();
        } catch (error) {
            console.warn("[Sources] Failed to create source:", error);
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
            // Find the source to get its type for contextual messages
            const source = sources.find(s => s.id === id);
            const sourceType = source?.source_type || "unknown";

            await syncSource(id);
            setSyncingSourceId(id);
            setShowConsole(true);
            setLogs([`> Starting sync job for source #${id} (${sourceType})...`]);

            // Source-type-aware log messages
            const typeMessages: Record<string, string[]> = {
                web: ["> Fetching HTML from URL...", "> Parsing web content...", "> Extracting to Markdown..."],
                file: ["> Downloading file from storage...", "> Extracting content...", "> Converting to Markdown..."],
                document: ["> Downloading document from storage...", "> Extracting text content...", "> Converting to Markdown..."],
                tabular: ["> Downloading file from storage...", "> Parsing tabular data...", "> Building Markdown table..."],
                mcp: ["> Connecting to MCP server...", "> Fetching resources...", "> Converting to Markdown..."],
            };
            const msgs = typeMessages[sourceType] || ["> Processing source..."];

            // Show type-specific messages with delay
            msgs.forEach((msg, i) => {
                setTimeout(() => setLogs(l => [...l, msg]), (i + 1) * 1000);
            });

            // Poll for completion every 2 seconds
            const pollInterval = setInterval(async () => {
                try {
                    const updated = await fetchSource(id);
                    if (!updated) return;

                    if (updated.last_sync_status === "COMPLETED") {
                        clearInterval(pollInterval);
                        const chunks = updated.total_chunks ?? 0;
                        const sizeMb = updated.mb_size?.toFixed(2) ?? "0.00";
                        setLogs(l => [
                            ...l,
                            "> ─── Sync Complete ───",
                            `> ✓ Extracted ${sizeMb} MB of content`,
                            `> ✓ ${chunks} chunk(s) stored`,
                            "> ✓ Deduplication applied",
                            "> Completed!",
                        ]);
                        loadSources();
                        setTimeout(() => setShowConsole(false), 3000);
                    } else if (updated.last_sync_status === "FAILED") {
                        clearInterval(pollInterval);
                        const errorMsg = updated.raw_markdown || "Unknown error";
                        setLogs(l => [
                            ...l,
                            "> ─── Sync Failed ───",
                            `> ✗ Error: ${errorMsg}`,
                        ]);
                        loadSources();
                    }
                } catch {
                    // Polling error — ignore and retry on next tick
                }
            }, 2000);

            // Safety timeout: stop polling after 2 minutes
            setTimeout(() => {
                clearInterval(pollInterval);
            }, 120000);
        } catch (error) {
            console.warn("[Sources] Failed to sync source:", error);
            setShowConsole(true);
            setLogs(l => [...l, `> ✗ Failed to trigger sync: ${error}`]);
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

                {selectedType === "web" && newUrl.trim() && (
                    <div className="space-y-3">
                        <div className="flex items-center gap-2">
                            <Button
                                id="discover-pages-btn"
                                variant="outline"
                                size="sm"
                                disabled={hierarchyLoading}
                                onClick={async () => {
                                    setHierarchyLoading(true);
                                    setHierarchyDiscovered(false);
                                    try {
                                        // We need a source ID — create temp source first or use existing
                                        // For now, create the source first, then discover
                                        let sourceId: number;
                                        const existingSource = sources.find(s => {
                                            const url = s.config_json?.url;
                                            return url === newUrl;
                                        });
                                        if (existingSource) {
                                            sourceId = existingSource.id;
                                        } else {
                                            const created = await createSource({
                                                name: newName || "Hierarchy Discovery",
                                                source_type: "web",
                                                config_json: { url: newUrl },
                                            });
                                            sourceId = created.id;
                                            await loadSources();
                                        }
                                        const result = await discoverHierarchy(sourceId, { max_depth: 3, max_pages: 100 });
                                        setHierarchyPages(result.pages);
                                        const allUrls = new Set(result.pages.filter(p => p.status !== "error").map(p => p.url));
                                        setSelectedPageUrls(allUrls);
                                        setHierarchyDiscovered(true);
                                    } catch (error) {
                                        console.warn("[Hierarchy] Discovery failed:", error);
                                    } finally {
                                        setHierarchyLoading(false);
                                    }
                                }}
                            >
                                {hierarchyLoading ? (
                                    <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Discovering...</>
                                ) : (
                                    <><Search className="w-4 h-4 mr-2" /> Discover Pages</>
                                )}
                            </Button>
                            {hierarchyDiscovered && (
                                <span className="text-xs text-muted-foreground">
                                    {selectedPageUrls.size} of {hierarchyPages.length} pages selected
                                </span>
                            )}
                        </div>

                        {hierarchyDiscovered && hierarchyPages.length > 0 && (
                            <div className="border rounded-md p-3 space-y-2 max-h-64 overflow-y-auto bg-muted/30">
                                <div className="flex items-center justify-between mb-2">
                                    <span className="text-sm font-medium">Discovered Pages</span>
                                    <div className="flex gap-2">
                                        <button
                                            type="button"
                                            className="text-xs text-blue-500 hover:underline"
                                            onClick={() => setSelectedPageUrls(new Set(hierarchyPages.filter(p => p.status !== "error").map(p => p.url)))}
                                        >
                                            Select All
                                        </button>
                                        <button
                                            type="button"
                                            className="text-xs text-muted-foreground hover:underline"
                                            onClick={() => setSelectedPageUrls(new Set())}
                                        >
                                            Deselect All
                                        </button>
                                    </div>
                                </div>
                                {hierarchyPages.map((page, idx) => {
                                    const isSelected = selectedPageUrls.has(page.url);
                                    const statusEmoji = page.status === "new" ? "🆕" : page.status === "updated" ? "🔄" : page.status === "unchanged" ? "✅" : page.status === "duplicate" ? "🔁" : "❌";
                                    return (
                                        <div
                                            key={page.url}
                                            className={`flex items-center gap-2 py-1 px-2 rounded cursor-pointer hover:bg-muted/50 transition-colors ${isSelected ? "bg-blue-50 dark:bg-blue-950/30" : ""
                                                }`}
                                            style={{ paddingLeft: `${page.depth * 20 + 8}px` }}
                                            onClick={() => {
                                                const next = new Set(selectedPageUrls);
                                                if (isSelected) next.delete(page.url); else next.add(page.url);
                                                setSelectedPageUrls(next);
                                            }}
                                        >
                                            {isSelected ? (
                                                <CheckSquare className="w-4 h-4 text-blue-500 shrink-0" />
                                            ) : (
                                                <Square className="w-4 h-4 text-muted-foreground shrink-0" />
                                            )}
                                            <span className="text-sm">{statusEmoji}</span>
                                            <div className="min-w-0 flex-1">
                                                <div className="text-sm truncate">
                                                    {page.title || page.url}
                                                </div>
                                                {page.title && (
                                                    <div className="text-xs text-muted-foreground truncate">
                                                        {page.url}
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        )}

                        {hierarchyDiscovered && selectedPageUrls.size > 0 && (
                            <Button
                                id="import-selected-btn"
                                size="sm"
                                disabled={importingPages}
                                onClick={async () => {
                                    setImportingPages(true);
                                    try {
                                        const sourceMatch = sources.find(s => s.config_json?.url === newUrl);
                                        if (sourceMatch) {
                                            const urlEntries = Array.from(selectedPageUrls).map(url => {
                                                const page = hierarchyPages.find(p => p.url === url);
                                                return { url, title: page?.title || undefined, depth: page?.depth };
                                            });
                                            await importPages(sourceMatch.id, urlEntries);
                                            await loadSources();
                                        }
                                    } catch (error) {
                                        console.warn("[Hierarchy] Import failed:", error);
                                    } finally {
                                        setImportingPages(false);
                                    }
                                }}
                            >
                                {importingPages ? (
                                    <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Importing...</>
                                ) : (
                                    <>Import {selectedPageUrls.size} Selected Pages</>
                                )}
                            </Button>
                        )}
                    </div>
                )}

                {selectedType === "mcp" && (
                    <div className="grid gap-3">
                        <div className="grid gap-2">
                            <Label htmlFor="wizard-mcp-url">MCP Server URL</Label>
                            <Input
                                id="wizard-mcp-url"
                                value={mcpConnectionString}
                                onChange={(e) => setMcpConnectionString(e.target.value)}
                                placeholder="http://localhost:3000/sse"
                            />
                        </div>
                        <div className="grid gap-2">
                            <Label htmlFor="wizard-mcp-transport">Transport Type</Label>
                            <select
                                id="wizard-mcp-transport"
                                className="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm"
                                defaultValue="sse"
                            >
                                <option value="sse">SSE (Server-Sent Events)</option>
                                <option value="stdio">Stdio</option>
                            </select>
                        </div>
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
                            <div className="space-y-2">
                                <div className="flex items-center justify-between">
                                    <span className="text-sm text-muted-foreground">
                                        {selectedFiles.length} file(s) selected
                                    </span>
                                    {selectedFiles.length > 1 && (
                                        <button
                                            type="button"
                                            onClick={handleClearFiles}
                                            className="text-xs text-red-500 hover:text-red-700 hover:underline"
                                        >
                                            Clear all
                                        </button>
                                    )}
                                </div>
                                <div className="space-y-1 max-h-40 overflow-y-auto">
                                    {selectedFiles.map((file, i) => (
                                        <div
                                            key={`${file.name}-${i}`}
                                            className="flex items-center justify-between gap-2 px-3 py-1.5 rounded-md bg-muted/50 border border-border text-sm"
                                        >
                                            <div className="flex items-center gap-2 min-w-0">
                                                <FileText className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
                                                <span className="truncate">{file.name}</span>
                                                <span className="text-xs text-muted-foreground shrink-0">
                                                    {file.size < 1024
                                                        ? `${file.size} B`
                                                        : file.size < 1048576
                                                            ? `${(file.size / 1024).toFixed(1)} KB`
                                                            : `${(file.size / 1048576).toFixed(1)} MB`}
                                                </span>
                                            </div>
                                            <button
                                                type="button"
                                                onClick={() => handleRemoveFile(i)}
                                                className="p-0.5 rounded hover:bg-destructive/10 text-muted-foreground hover:text-red-500 transition-colors"
                                                title={`Remove ${file.name}`}
                                            >
                                                <X className="w-3.5 h-3.5" />
                                            </button>
                                        </div>
                                    ))}
                                </div>
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
                <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={() => setShowDbWizard(true)}>
                        <Database className="w-4 h-4 mr-2" />
                        External DB
                    </Button>
                    <Button onClick={openWizard}>
                        <Plus className="w-4 h-4 mr-2" />
                        Add Source
                    </Button>
                </div>
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
                                    groupedSources.map(([type, groupSources]) => (
                                        <React.Fragment key={type}>
                                            <TableRow
                                                className="cursor-pointer hover:bg-muted/50 bg-muted/30"
                                                onClick={() => toggleGroup(type)}
                                            >
                                                <TableCell colSpan={5} className="py-2 px-4 font-semibold text-sm">
                                                    <span className="mr-2">{collapsedGroups.has(type) ? "▶" : "▼"}</span>
                                                    {TYPE_LABELS[type] || type} ({groupSources.length})
                                                </TableCell>
                                            </TableRow>
                                            {!collapsedGroups.has(type) && groupSources.map((s) => (
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
                                            ))}
                                        </React.Fragment>
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
                    {configuringSource && (
                        <div className="pt-4 border-t mt-4 gap-2 grid">
                            <Label className="text-[13px] font-medium text-purple-600 dark:text-purple-400">Agentic Automation</Label>
                            <Button 
                                variant="outline" 
                                className="w-full sm:w-auto"
                                onClick={() => {
                                    const src = configuringSource;
                                    setConfiguringSource(null);
                                    setPipelineConfigSource(src);
                                }}
                            >
                                <Sparkles className="w-4 h-4 mr-2 text-purple-600" />
                                Run Full Auto-Pipeline
                            </Button>
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

            {/* ═══ Configure Auto-Pipeline Dialog ═══ */}
            <Dialog open={pipelineConfigSource !== null} onOpenChange={(open) => {
                if (!open) {
                    setPipelineConfigSource(null);
                    setPipelineStarting(false);
                    setPipelineRunStatus(null);
                }
            }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Configure Auto-Pipeline</DialogTitle>
                        <DialogDescription>Review the pipeline settings before running on {pipelineConfigSource?.name}.</DialogDescription>
                    </DialogHeader>
                    
                    <div className="grid gap-4 py-4">
                        <div className="grid gap-4">
                            <div className="grid gap-2">
                                <Label>Provider</Label>
                                <select 
                                    className="h-10 px-3 rounded-md border bg-background text-sm"
                                    value={pipelineSelectedProvider}
                                    disabled={pipelineStarting}
                                    onChange={(e) => {
                                        const p = e.target.value;
                                        setPipelineSelectedProvider(p);
                                        const modelsForProvider = aiModels.filter(m => m.provider === p);
                                        if (modelsForProvider.length > 0) {
                                            setPipelineSelectedModel(modelsForProvider[0].model_id);
                                        } else {
                                            setPipelineSelectedModel("");
                                        }
                                    }}
                                >
                                    {Array.from(new Set(aiModels.map(m => m.provider))).map(provider => (
                                        <option key={provider} value={provider}>{provider}</option>
                                    ))}
                                    {aiModels.length === 0 && (
                                        <>
                                            <option value="google">google</option>
                                            <option value="openai">openai</option>
                                            <option value="ollama">ollama</option>
                                            <option value="heimdall">heimdall</option>
                                        </>
                                    )}
                                </select>
                            </div>
                            <div className="grid gap-2">
                                <Label>Model ID</Label>
                                <select 
                                    className="h-10 px-3 rounded-md border bg-background text-sm"
                                    value={pipelineSelectedModel} 
                                    disabled={pipelineStarting}
                                    onChange={(e) => setPipelineSelectedModel(e.target.value)} 
                                >
                                    {aiModels.length > 0 ? (
                                        aiModels.filter(m => m.provider === pipelineSelectedProvider).map(m => (
                                            <option key={m.model_id} value={m.model_id}>{m.model_id}</option>
                                        ))
                                    ) : (
                                        <option value="">No models available from backend</option>
                                    )}
                                </select>
                            </div>
                        </div>
                        <p className="text-xs text-muted-foreground -mt-2">This AI Model will be used for both Knowledge Graph and QA Extractor steps.</p>

                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                            <label className="flex items-start space-x-3 cursor-pointer">
                                <input 
                                    type="checkbox" 
                                    className="form-checkbox mt-1" 
                                    disabled={pipelineStarting}
                                    checked={pipelineEnablePageIndex} 
                                    onChange={(e) => setPipelineEnablePageIndex(e.target.checked)} 
                                />
                                <div>
                                    <span className="font-medium text-sm">Enable PageIndex Tree Generation</span>
                                    <p className="text-xs text-muted-foreground mt-0.5">Generates a hierarchical semantic tree. This will consume more LLM tokens.</p>
                                </div>
                            </label>
                        </div>

                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                            <label className="flex items-start space-x-3 cursor-pointer">
                                <input 
                                    type="checkbox" 
                                    className="form-checkbox mt-1" 
                                    disabled={pipelineStarting}
                                    checked={pipelineSkipKg} 
                                    onChange={(e) => setPipelineSkipKg(e.target.checked)} 
                                />
                                <div>
                                    <span className="font-medium text-sm">Skip KG Extraction</span>
                                    <p className="text-xs text-muted-foreground mt-0.5">Skip Knowledge Graph extraction. Use when KG data already exists from a previous run.</p>
                                </div>
                            </label>
                        </div>

                        {pipelineStarting && (
                            <div className="mt-4 border rounded-md p-4 bg-muted/50">
                                <h4 className="text-sm font-semibold mb-3 flex items-center">
                                    {pipelineRunStatus?.status === 'completed' || pipelineRunStatus?.status === 'finished' || pipelineRunStatus?.status === 'failed' ? (
                                        (pipelineRunStatus?.status === 'completed' || pipelineRunStatus?.status === 'finished') ? <CheckSquare className="w-4 h-4 mr-2 text-green-500" /> : <X className="w-4 h-4 mr-2 text-red-500" />
                                    ) : (
                                        <Loader2 className="w-4 h-4 mr-2 animate-spin text-primary" /> 
                                    )}
                                    {(pipelineRunStatus?.status === 'completed' || pipelineRunStatus?.status === 'finished') ? 'Pipeline Completed' : pipelineRunStatus?.status === 'failed' ? 'Pipeline Failed' : 'Pipeline Running...'}
                                </h4>
                                {pipelineRunStatus ? (
                                    <div className="space-y-2 text-sm max-h-[200px] overflow-y-auto">
                                        {pipelineRunStatus.steps?.map((step: any) => (
                                            <div key={step.step} className="flex items-center justify-between py-1">
                                                <div className="flex items-center">
                                                    {step.status === 'running' ? <Loader2 className="w-4 h-4 mr-2 animate-spin text-blue-500" /> : 
                                                     step.status === 'failed' ? <X className="w-4 h-4 mr-2 text-red-500" /> :
                                                     (step.status === 'completed' || step.status === 'skipped') ? <CheckSquare className="w-4 h-4 mr-2 text-green-500" /> :
                                                     <Square className="w-4 h-4 mr-2 text-muted-foreground" />}
                                                    <span className={step.status === 'pending' || step.status === 'skipped' ? 'text-muted-foreground' : ''}>{step.name}</span>
                                                </div>
                                                {step.latency_ms > 0 && <span className="text-xs text-muted-foreground">{step.latency_ms}ms</span>}
                                            </div>
                                        ))}
                                        {pipelineRunStatus.status === 'failed' && <p className="text-xs text-red-500 mt-2">{pipelineRunStatus.error}</p>}
                                        {(pipelineRunStatus.status === 'completed' || pipelineRunStatus.status === 'finished') && <p className="text-xs text-green-600 font-medium mt-2">All extraction processes finished successfully.</p>}
                                    </div>
                                ) : (
                                    <div className="text-sm text-muted-foreground">Initializing run...</div>
                                )}
                            </div>
                        )}
                    </div>
                    
                    <DialogFooter>
                        {pipelineStarting && (pipelineRunStatus?.status === 'completed' || pipelineRunStatus?.status === 'finished' || pipelineRunStatus?.status === 'failed') ? (
                            <Button onClick={() => { setPipelineConfigSource(null); setPipelineStarting(false); setPipelineRunStatus(null); loadSources(); }}>Close</Button>
                        ) : pipelineStarting ? (
                            <>
                                <Button variant="outline" onClick={() => { setPipelineConfigSource(null); setPipelineStarting(false); setPipelineRunStatus(null); loadSources(); }}>Run in Background</Button>
                                <Button disabled><Loader2 className="w-4 h-4 mr-2 animate-spin"/> Running...</Button>
                            </>
                        ) : (
                            <>
                                <Button variant="outline" onClick={() => setPipelineConfigSource(null)}>Cancel</Button>
                                <Button 
                                    onClick={async () => {
                                        if (!pipelineConfigSource?.id) return;
                                        setPipelineStarting(true);
                                        setPipelineRunStatus(null);
                                        try {
                                            const res = await runAutoPipeline(pipelineConfigSource.id, {
                                                provider: pipelineSelectedProvider,
                                                model: pipelineSelectedModel,
                                                enablePageIndex: pipelineEnablePageIndex,
                                                skipKg: pipelineSkipKg,
                                            });
                                            // Start Polling
                                            const intervalId = setInterval(async () => {
                                                if (!pipelineConfigSource?.id) return;
                                                try {
                                                    const statusRes = await fetchPipelineStatus(pipelineConfigSource.id);
                                                    setPipelineRunStatus(statusRes);
                                                    if (statusRes.status === 'completed' || statusRes.status === 'failed') {
                                                        clearInterval(intervalId);
                                                        loadSources();
                                                    }
                                                } catch (e) { console.error(e); }
                                            }, 2000);
                                        } catch (err: any) {
                                            alert(`Error: ${err.message}`);
                                            setPipelineStarting(false);
                                        }
                                    }}
                                    disabled={!pipelineSelectedModel}
                                >
                                    <Sparkles className="w-4 h-4 mr-2" /> Start Pipeline
                                </Button>
                            </>
                        )}
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* ═══ Markdown Preview & Edit Dialog ═══ */}
            <Dialog open={previewingSource !== null} onOpenChange={(open) => {
                if (!open) {
                    setPreviewingSource(null);
                    setAiTokensUsed(null);
                    setAiError(null);
                } else {
                    // Load models when dialog opens
                    fetchModels().then((models) => {
                        setAiModels(models);
                        if (models.length > 0 && !aiSelectedModel) {
                            setAiSelectedModel(models[0].model_id);
                        }
                    }).catch(() => { });
                }
            }}>
                <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col h-[80vh]">
                    <DialogHeader>
                        <DialogTitle>Markdown Preview {previewingSource?.name ? `- ${previewingSource.name}` : ""}</DialogTitle>
                        <DialogDescription className="text-muted-foreground">
                            Preview and quick-edit the raw markdown extracted from this source.
                        </DialogDescription>
                    </DialogHeader>

                    {/* 🤖 Extract with AI — shown when content is empty, has errors, or source failed */}
                    {(!previewingSource?.raw_markdown || previewingSource.raw_markdown.trim() === "" || previewingSource.raw_markdown.startsWith("[Error") || previewingSource.last_sync_status === "FAILED") && (
                        <div className="rounded-lg border border-dashed border-blue-300 bg-blue-50/50 dark:bg-blue-950/20 dark:border-blue-800 p-4 space-y-3">
                            <div className="flex items-center gap-2 text-sm font-medium text-blue-700 dark:text-blue-300">
                                <Sparkles className="w-4 h-4" />
                                Extract with AI
                            </div>
                            <p className="text-xs text-muted-foreground">
                                Native extraction failed or content is empty. Use an LLM to extract content from the source file.
                            </p>
                            <div className="flex items-center gap-3 flex-wrap">
                                <div className="grid gap-1">
                                    <Label className="text-xs">Model</Label>
                                    <select
                                        className="h-8 px-2 rounded-md border bg-background text-sm min-w-[180px]"
                                        value={aiSelectedModel}
                                        onChange={(e) => setAiSelectedModel(e.target.value)}
                                    >
                                        {aiModels.map((m) => (
                                            <option key={m.model_id} value={m.model_id}>
                                                {m.model_id} ({m.provider})
                                            </option>
                                        ))}
                                        {aiModels.length === 0 && <option value="">Loading models...</option>}
                                    </select>
                                </div>
                                <div className="grid gap-1">
                                    <Label className="text-xs">Output Format</Label>
                                    <div className="flex gap-1">
                                        <Button
                                            variant={aiOutputFormat === "markdown" ? "default" : "outline"}
                                            size="sm"
                                            className="h-8 text-xs"
                                            onClick={() => setAiOutputFormat("markdown")}
                                        >
                                            Markdown
                                        </Button>
                                        <Button
                                            variant={aiOutputFormat === "table" ? "default" : "outline"}
                                            size="sm"
                                            className="h-8 text-xs"
                                            onClick={() => setAiOutputFormat("table")}
                                        >
                                            Table
                                        </Button>
                                    </div>
                                </div>
                                <div className="grid gap-1">
                                    <Label className="text-xs">&nbsp;</Label>
                                    <Button
                                        size="sm"
                                        className="h-8"
                                        disabled={aiExtracting || !aiSelectedModel || !previewingSource?.id}
                                        onClick={async () => {
                                            if (!previewingSource?.id) return;
                                            setAiExtracting(true);
                                            setAiError(null);
                                            setAiTokensUsed(null);
                                            try {
                                                const result = await extractWithAi(previewingSource.id, aiSelectedModel, aiOutputFormat);
                                                setPreviewingSource((prev) => prev ? { ...prev, raw_markdown: result.content } : null);
                                                setAiTokensUsed(result.tokens_used);
                                            } catch (err: any) {
                                                setAiError(err.message || "Extraction failed");
                                            } finally {
                                                setAiExtracting(false);
                                            }
                                        }}
                                    >
                                        {aiExtracting ? (
                                            <><Loader2 className="w-3 h-3 mr-1 animate-spin" /> Extracting...</>
                                        ) : (
                                            <><Sparkles className="w-3 h-3 mr-1" /> Extract</>
                                        )}
                                    </Button>
                                </div>
                            </div>
                            {aiTokensUsed !== null && (
                                <p className="text-xs text-green-600 dark:text-green-400">✓ Extracted successfully — {aiTokensUsed.toLocaleString()} tokens used</p>
                            )}
                            {aiError && (
                                <p className="text-xs text-red-600 dark:text-red-400">✗ {aiError}</p>
                            )}
                        </div>
                    )}

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

            {/* ═══ External DB Connector Wizard ═══ */}
            <DbConnectorWizard
                open={showDbWizard}
                onOpenChange={setShowDbWizard}
                onImportComplete={(markdown, rowCount) => {
                    console.log(`[DB Import] ${rowCount} rows imported`);
                    loadSources();
                }}
            />
        </div>
    );
}
