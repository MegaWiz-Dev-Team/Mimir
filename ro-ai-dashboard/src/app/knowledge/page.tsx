"use client";

import { useState, useEffect, useCallback, Fragment } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { BookOpen, Search, Filter, ChevronLeft, ChevronRight, FileText, Hash, Clock, Layers, ExternalLink, Sparkles, RefreshCw, CheckCircle2, AlertCircle, X } from "lucide-react";
import { fetchChunks, fetchSources, generateQaForChunks, ChunkItem, DataSource } from "@/lib/api";
import Link from "next/link";

// ─── Helpers ──────────────────────────────────────────────────────────────────

/** Unescape common escaped content (\\n → newline, \\t → tab, \\\" → ", etc.) */
function unescapeContent(raw: string): string {
    return raw
        .replace(/\\\\n/g, "\n")
        .replace(/\\n/g, "\n")
        .replace(/\\\\t/g, "\t")
        .replace(/\\t/g, "\t")
        .replace(/\\\\"/g, '"')
        .replace(/\\"/g, '"')
        .replace(/\\u0026/g, "&")
        .replace(/\\u003c/g, "<")
        .replace(/\\u003e/g, ">");
}

/** Check if a chunk contains mostly non-text garbage */
function isGarbageChunk(content: string): boolean {
    const trimmed = content.trim();
    if (/^[\[{]/.test(trimmed) && /["\\{}[\]]{10,}/.test(trimmed.slice(0, 200))) return true;
    if ((trimmed.match(/\\"/g) || []).length > 10) return true;
    if (/\/_next\/static\/chunks\//.test(trimmed)) return true;
    const nonWord = (trimmed.match(/[^a-zA-Z0-9\s.,!?;:\-()]/g) || []).length;
    if (nonWord / trimmed.length > 0.5 && trimmed.length > 50) return true;
    return false;
}

/** Highlight search terms in text */
function highlightText(text: string, searchTerm: string): React.ReactNode {
    if (!searchTerm || searchTerm.length < 2) return text;
    const escaped = searchTerm.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const parts = text.split(new RegExp(`(${escaped})`, "gi"));
    if (parts.length <= 1) return text;
    return parts.map((part, i) =>
        part.toLowerCase() === searchTerm.toLowerCase()
            ? <mark key={i} className="bg-yellow-200 dark:bg-yellow-700/50 px-0.5 rounded">{part}</mark>
            : <Fragment key={i}>{part}</Fragment>
    );
}

/** Deterministic color for a source name */
function sourceColor(name: string): string {
    const colors = [
        "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
        "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
        "bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300",
        "bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300",
        "bg-rose-100 text-rose-700 dark:bg-rose-900/40 dark:text-rose-300",
        "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/40 dark:text-cyan-300",
        "bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300",
        "bg-teal-100 text-teal-700 dark:bg-teal-900/40 dark:text-teal-300",
    ];
    let hash = 0;
    for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
    return colors[Math.abs(hash) % colors.length];
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function KnowledgePage() {
    const [chunks, setChunks] = useState<ChunkItem[]>([]);
    const [sources, setSources] = useState<DataSource[]>([]);
    const [total, setTotal] = useState(0);
    const [totalTokens, setTotalTokens] = useState(0);
    const [page, setPage] = useState(1);
    const [perPage] = useState(20);
    const [search, setSearch] = useState("");
    const [searchDebounce, setSearchDebounce] = useState("");
    const [sourceFilter, setSourceFilter] = useState<number | undefined>();
    const [isLoading, setIsLoading] = useState(true);
    const [selectedChunk, setSelectedChunk] = useState<ChunkItem | null>(null);
    const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
    const [qaRunning, setQaRunning] = useState(false);
    const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);
    const [showGarbage, setShowGarbage] = useState(false);

    // Debounce search
    useEffect(() => {
        const t = setTimeout(() => setSearchDebounce(search), 300);
        return () => clearTimeout(t);
    }, [search]);

    const loadChunks = useCallback(async () => {
        setIsLoading(true);
        try {
            const data = await fetchChunks({
                source_id: sourceFilter,
                search: searchDebounce || undefined,
                page,
                per_page: perPage,
            });
            setChunks(data.chunks);
            setTotal(data.total);
            setTotalTokens(data.total_tokens ?? 0);
        } catch {
            setChunks([]);
            setTotal(0);
            setTotalTokens(0);
        } finally {
            setIsLoading(false);
        }
    }, [sourceFilter, searchDebounce, page, perPage]);

    useEffect(() => { fetchSources().then(setSources).catch(() => setSources([])); }, []);
    useEffect(() => { loadChunks(); }, [loadChunks]);
    useEffect(() => { setPage(1); }, [searchDebounce, sourceFilter]);
    useEffect(() => { if (toast) { const t = setTimeout(() => setToast(null), 4000); return () => clearTimeout(t); } }, [toast]);

    const totalPages = Math.max(1, Math.ceil(total / perPage));
    const displayChunks = showGarbage ? chunks : chunks.filter(c => !isGarbageChunk(c.content));
    const garbageCount = chunks.length - chunks.filter(c => !isGarbageChunk(c.content)).length;

    const truncateContent = (content: string, maxLen = 100) => {
        const clean = unescapeContent(content);
        const firstLine = clean.split("\n").find(l => l.trim().length > 10) || clean.split("\n")[0] || clean;
        return firstLine.length > maxLen ? firstLine.slice(0, maxLen) + "…" : firstLine;
    };

    // ─── Selection Logic ──────────────────────────────────────────────────────
    const toggleSelect = (id: number) => {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    };

    const toggleSelectAll = () => {
        if (selectedIds.size === displayChunks.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(displayChunks.map(c => c.id)));
        }
    };

    const handleGenerateQa = async () => {
        if (selectedIds.size === 0) return;
        setQaRunning(true);
        try {
            const result = await generateQaForChunks(Array.from(selectedIds));
            setToast({ message: result.message || `QA generation started for ${selectedIds.size} chunks`, type: "success" });
            setSelectedIds(new Set());
            setTimeout(loadChunks, 3000);
        } catch (err: any) {
            setToast({ message: err?.message || "Failed to start QA generation", type: "error" });
        } finally {
            setQaRunning(false);
        }
    };

    // Clear selection when page/filter changes
    useEffect(() => { setSelectedIds(new Set()); }, [page, searchDebounce, sourceFilter]);

    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* Toast */}
            {toast && (
                <div className={`fixed top-4 right-4 z-50 flex items-center gap-2 px-4 py-3 rounded-lg shadow-lg transition-all animate-in slide-in-from-top-2 ${toast.type === "success" ? "bg-green-600 text-white" : "bg-red-600 text-white"
                    }`}>
                    {toast.type === "success" ? <CheckCircle2 className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
                    <span className="text-sm font-medium">{toast.message}</span>
                </div>
            )}

            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold flex items-center gap-2">
                        <BookOpen className="w-6 h-6 text-blue-600" />
                        Knowledge Base
                    </h1>
                    <p className="text-muted-foreground text-sm mt-1">
                        Browse, search, and manage all chunks in your knowledge store
                    </p>
                </div>
                <div className="flex items-center gap-3 text-sm text-muted-foreground">
                    <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted">
                        <Layers className="w-4 h-4" />
                        <span className="font-semibold">{total.toLocaleString()}</span> chunks
                    </div>
                    <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted">
                        <Hash className="w-4 h-4" />
                        <span className="font-semibold">{totalTokens.toLocaleString()}</span> tokens
                    </div>
                </div>
            </div>

            {/* Filters */}
            <Card>
                <CardContent className="py-4">
                    <div className="flex items-center gap-4">
                        <div className="relative flex-1">
                            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                            <Input placeholder="Search chunks by content..." value={search} onChange={(e) => setSearch(e.target.value)} className="pl-10" />
                        </div>
                        <div className="flex items-center gap-2">
                            <Filter className="w-4 h-4 text-muted-foreground" />
                            <select
                                value={sourceFilter ?? ""}
                                onChange={(e) => setSourceFilter(e.target.value ? Number(e.target.value) : undefined)}
                                className="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm min-w-[180px]"
                            >
                                <option value="">All Sources</option>
                                {sources.map((s) => (<option key={s.id} value={s.id}>{s.name}</option>))}
                            </select>
                        </div>
                    </div>
                    {garbageCount > 0 && (
                        <div className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
                            <button onClick={() => setShowGarbage(!showGarbage)} className="hover:text-foreground transition-colors underline decoration-dotted">
                                {showGarbage ? "Hide" : "Show"} {garbageCount} non-text chunks
                            </button>
                            <span className="text-muted-foreground/50">(raw JSON, JS bundles, web artifacts)</span>
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* Table */}
            <Card>
                <CardContent className="p-0">
                    {isLoading ? (
                        <div className="flex items-center justify-center py-16 text-muted-foreground">
                            <div className="animate-spin w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full mr-3" />
                            Loading chunks...
                        </div>
                    ) : displayChunks.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-center">
                            <div className="w-16 h-16 rounded-full bg-blue-50 dark:bg-blue-900/30 flex items-center justify-center mb-4">
                                <BookOpen className="w-8 h-8 text-blue-600 dark:text-blue-400" />
                            </div>
                            <h3 className="text-lg font-semibold mb-2">No chunks found</h3>
                            <p className="text-muted-foreground text-sm max-w-sm">
                                {search || sourceFilter ? "Try adjusting your search or filter criteria" : "Add sources and sync them to start building your knowledge base"}
                            </p>
                            {!search && !sourceFilter && (
                                <Link href="/sources"><Button className="mt-4" variant="outline"><ExternalLink className="w-4 h-4 mr-2" /> Go to Sources</Button></Link>
                            )}
                        </div>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead className="w-12 text-center">
                                        <input
                                            type="checkbox"
                                            checked={selectedIds.size === displayChunks.length && displayChunks.length > 0}
                                            onChange={toggleSelectAll}
                                            className="w-4 h-4 rounded border-gray-300"
                                            aria-label="Select all chunks"
                                        />
                                    </TableHead>
                                    <TableHead className="w-16 text-center">#</TableHead>
                                    <TableHead className="w-44">Source</TableHead>
                                    <TableHead>Content Preview</TableHead>
                                    <TableHead className="w-24 text-center">Tokens</TableHead>
                                    <TableHead className="w-36 text-center">Created</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {displayChunks.map((chunk) => {
                                    const isGarbage = isGarbageChunk(chunk.content);
                                    const isSelected = selectedIds.has(chunk.id);
                                    return (
                                        <TableRow key={chunk.id} className={`hover:bg-muted/50 transition-colors ${isGarbage ? "opacity-50" : ""} ${isSelected ? "bg-blue-50 dark:bg-blue-900/20" : ""}`}>
                                            <TableCell className="text-center" onClick={(e) => e.stopPropagation()}>
                                                <input
                                                    type="checkbox"
                                                    checked={isSelected}
                                                    onChange={() => toggleSelect(chunk.id)}
                                                    className="w-4 h-4 rounded border-gray-300"
                                                    aria-label={`Select chunk ${chunk.chunk_index}`}
                                                />
                                            </TableCell>
                                            <TableCell className="text-center font-mono text-sm text-muted-foreground cursor-pointer" onClick={() => setSelectedChunk(chunk)}>
                                                {chunk.chunk_index}
                                            </TableCell>
                                            <TableCell className="cursor-pointer" onClick={() => setSelectedChunk(chunk)}>
                                                <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${sourceColor(chunk.source_name)}`}>
                                                    <FileText className="w-3 h-3 shrink-0" />
                                                    <span className="truncate max-w-[100px]">{chunk.source_name || `Source #${chunk.source_id}`}</span>
                                                </span>
                                            </TableCell>
                                            <TableCell className="cursor-pointer" onClick={() => setSelectedChunk(chunk)}>
                                                <p className="text-sm text-muted-foreground line-clamp-2">
                                                    {isGarbage
                                                        ? <span className="italic text-muted-foreground/60">[non-text content]</span>
                                                        : highlightText(truncateContent(chunk.content), searchDebounce)}
                                                </p>
                                            </TableCell>
                                            <TableCell className="text-center cursor-pointer" onClick={() => setSelectedChunk(chunk)}>
                                                <span className="text-sm font-mono">{chunk.token_count ?? "—"}</span>
                                            </TableCell>
                                            <TableCell className="text-center text-sm text-muted-foreground cursor-pointer" onClick={() => setSelectedChunk(chunk)}>
                                                {chunk.created_at ? new Date(chunk.created_at).toLocaleDateString() : "—"}
                                            </TableCell>
                                        </TableRow>
                                    );
                                })}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>

                {/* Pagination */}
                {totalPages > 1 && (
                    <div className="flex items-center justify-between px-6 py-4 border-t">
                        <p className="text-sm text-muted-foreground">
                            Showing {(page - 1) * perPage + 1}–{Math.min(page * perPage, total)} of {total} chunks
                        </p>
                        <div className="flex items-center gap-2">
                            <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => setPage((p) => Math.max(1, p - 1))}>
                                <ChevronLeft className="w-4 h-4" />
                            </Button>
                            <span className="text-sm font-medium px-2">{page} / {totalPages}</span>
                            <Button variant="outline" size="sm" disabled={page >= totalPages} onClick={() => setPage((p) => Math.min(totalPages, p + 1))}>
                                <ChevronRight className="w-4 h-4" />
                            </Button>
                        </div>
                    </div>
                )}
            </Card>

            {/* Floating Action Bar */}
            {selectedIds.size > 0 && (
                <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 flex items-center gap-4 px-6 py-3 bg-card border shadow-xl rounded-full">
                    <span className="text-sm font-medium">
                        {selectedIds.size} chunk{selectedIds.size > 1 ? "s" : ""} selected
                    </span>
                    <Button
                        size="sm"
                        disabled={qaRunning}
                        onClick={handleGenerateQa}
                        aria-label="Generate QA"
                    >
                        {qaRunning ? <RefreshCw className="w-4 h-4 mr-1 animate-spin" /> : <Sparkles className="w-4 h-4 mr-1" />}
                        {qaRunning ? "Running..." : "Generate QA"}
                    </Button>
                    <button onClick={() => setSelectedIds(new Set())} className="text-muted-foreground hover:text-foreground">
                        <X className="w-4 h-4" />
                    </button>
                </div>
            )}

            {/* Chunk Detail Dialog */}
            <Dialog open={!!selectedChunk} onOpenChange={() => setSelectedChunk(null)}>
                <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <Layers className="w-5 h-5 text-blue-600" />
                            Chunk #{selectedChunk?.chunk_index} — {selectedChunk?.source_name || `Source #${selectedChunk?.source_id}`}
                        </DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="flex items-center gap-4 text-sm text-muted-foreground">
                            <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${sourceColor(selectedChunk?.source_name || "")}`}>
                                {selectedChunk?.source_name}
                            </span>
                            <span className="flex items-center gap-1"><Hash className="w-3.5 h-3.5" />{selectedChunk?.token_count ?? "?"} tokens</span>
                            <span className="flex items-center gap-1"><Clock className="w-3.5 h-3.5" />{selectedChunk?.created_at ? new Date(selectedChunk.created_at).toLocaleString() : "Unknown"}</span>
                        </div>
                        <div className="p-4 rounded-lg bg-muted/50 border">
                            <div className="whitespace-pre-wrap text-sm leading-relaxed prose prose-sm dark:prose-invert max-w-none">
                                {selectedChunk?.content && unescapeContent(selectedChunk.content)}
                            </div>
                        </div>
                        {selectedChunk?.metadata_json && (
                            <div>
                                <h4 className="text-sm font-medium mb-2">Metadata</h4>
                                <pre className="p-3 rounded-lg bg-muted/30 border text-xs font-mono overflow-x-auto">
                                    {JSON.stringify(selectedChunk.metadata_json, null, 2)}
                                </pre>
                            </div>
                        )}
                    </div>
                </DialogContent>
            </Dialog>
        </div>
    );
}
