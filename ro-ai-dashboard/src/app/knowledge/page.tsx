"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { BookOpen, Search, Filter, ChevronLeft, ChevronRight, FileText, Hash, Clock, Layers, ExternalLink } from "lucide-react";
import { fetchChunks, fetchSources, ChunkItem, DataSource } from "@/lib/api";
import Link from "next/link";

export default function KnowledgePage() {
    const [chunks, setChunks] = useState<ChunkItem[]>([]);
    const [sources, setSources] = useState<DataSource[]>([]);
    const [total, setTotal] = useState(0);
    const [page, setPage] = useState(1);
    const [perPage] = useState(20);
    const [search, setSearch] = useState("");
    const [searchDebounce, setSearchDebounce] = useState("");
    const [sourceFilter, setSourceFilter] = useState<number | undefined>();
    const [isLoading, setIsLoading] = useState(true);
    const [selectedChunk, setSelectedChunk] = useState<ChunkItem | null>(null);

    // Debounce search input
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
        } catch {
            setChunks([]);
            setTotal(0);
        } finally {
            setIsLoading(false);
        }
    }, [sourceFilter, searchDebounce, page, perPage]);

    useEffect(() => {
        fetchSources().then(setSources).catch(() => setSources([]));
    }, []);

    useEffect(() => {
        loadChunks();
    }, [loadChunks]);

    // Reset to page 1 when filters change
    useEffect(() => {
        setPage(1);
    }, [searchDebounce, sourceFilter]);

    const totalPages = Math.max(1, Math.ceil(total / perPage));

    const truncateContent = (content: string, maxLen = 120) =>
        content.length > maxLen ? content.slice(0, maxLen) + "…" : content;

    const totalChunks = total;
    const totalTokens = chunks.reduce((sum, c) => sum + (c.token_count || 0), 0);

    return (
        <div className="container mx-auto p-8 space-y-6">
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
                        <span className="font-semibold">{totalChunks}</span> chunks
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
                            <Input
                                placeholder="Search chunks by content..."
                                value={search}
                                onChange={(e) => setSearch(e.target.value)}
                                className="pl-10"
                            />
                        </div>
                        <div className="flex items-center gap-2">
                            <Filter className="w-4 h-4 text-muted-foreground" />
                            <select
                                value={sourceFilter ?? ""}
                                onChange={(e) => setSourceFilter(e.target.value ? Number(e.target.value) : undefined)}
                                className="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm min-w-[180px]"
                            >
                                <option value="">All Sources</option>
                                {sources.map((s) => (
                                    <option key={s.id} value={s.id}>{s.name}</option>
                                ))}
                            </select>
                        </div>
                    </div>
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
                    ) : chunks.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-center">
                            <div className="w-16 h-16 rounded-full bg-blue-50 dark:bg-blue-900/30 flex items-center justify-center mb-4">
                                <BookOpen className="w-8 h-8 text-blue-600 dark:text-blue-400" />
                            </div>
                            <h3 className="text-lg font-semibold mb-2">No chunks found</h3>
                            <p className="text-muted-foreground text-sm max-w-sm">
                                {search || sourceFilter
                                    ? "Try adjusting your search or filter criteria"
                                    : "Add sources and sync them to start building your knowledge base"}
                            </p>
                            {!search && !sourceFilter && (
                                <Link href="/sources">
                                    <Button className="mt-4" variant="outline">
                                        <ExternalLink className="w-4 h-4 mr-2" /> Go to Sources
                                    </Button>
                                </Link>
                            )}
                        </div>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead className="w-16 text-center">#</TableHead>
                                    <TableHead className="w-40">Source</TableHead>
                                    <TableHead>Content Preview</TableHead>
                                    <TableHead className="w-24 text-center">Tokens</TableHead>
                                    <TableHead className="w-36 text-center">Created</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {chunks.map((chunk) => (
                                    <TableRow
                                        key={chunk.id}
                                        className="cursor-pointer hover:bg-muted/50 transition-colors"
                                        onClick={() => setSelectedChunk(chunk)}
                                    >
                                        <TableCell className="text-center font-mono text-sm text-muted-foreground">
                                            {chunk.chunk_index}
                                        </TableCell>
                                        <TableCell>
                                            <div className="flex items-center gap-2">
                                                <FileText className="w-4 h-4 text-muted-foreground shrink-0" />
                                                <span className="text-sm font-medium truncate max-w-[120px]">
                                                    {chunk.source_name || `Source #${chunk.source_id}`}
                                                </span>
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            <p className="text-sm text-muted-foreground line-clamp-2">
                                                {truncateContent(chunk.content)}
                                            </p>
                                        </TableCell>
                                        <TableCell className="text-center">
                                            <span className="text-sm font-mono">
                                                {chunk.token_count ?? "—"}
                                            </span>
                                        </TableCell>
                                        <TableCell className="text-center text-sm text-muted-foreground">
                                            {chunk.created_at
                                                ? new Date(chunk.created_at).toLocaleDateString()
                                                : "—"}
                                        </TableCell>
                                    </TableRow>
                                ))}
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
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={page <= 1}
                                onClick={() => setPage((p) => Math.max(1, p - 1))}
                            >
                                <ChevronLeft className="w-4 h-4" />
                            </Button>
                            <span className="text-sm font-medium px-2">
                                {page} / {totalPages}
                            </span>
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={page >= totalPages}
                                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                            >
                                <ChevronRight className="w-4 h-4" />
                            </Button>
                        </div>
                    </div>
                )}
            </Card>

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
                            <span className="flex items-center gap-1">
                                <Hash className="w-3.5 h-3.5" />
                                {selectedChunk?.token_count ?? "?"} tokens
                            </span>
                            <span className="flex items-center gap-1">
                                <Clock className="w-3.5 h-3.5" />
                                {selectedChunk?.created_at
                                    ? new Date(selectedChunk.created_at).toLocaleString()
                                    : "Unknown"}
                            </span>
                        </div>
                        <div className="p-4 rounded-lg bg-muted/50 border">
                            <pre className="whitespace-pre-wrap text-sm font-mono leading-relaxed">
                                {selectedChunk?.content}
                            </pre>
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
