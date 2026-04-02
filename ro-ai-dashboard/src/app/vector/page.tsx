"use client";

import { useEffect, useState } from "react";
import { fetchVectorStats, searchVectors } from "@/lib/api";
import Cookies from "js-cookie";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Database, Search, RefreshCw, ArrowLeft, CheckCircle2, AlertCircle, Filter, FileText } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import Link from "next/link";

export default function VectorPage() {
    const [stats, setStats] = useState<any>(null);
    const [loading, setLoading] = useState(true);
    const [searchQuery, setSearchQuery] = useState("");
    const [searchResults, setSearchResults] = useState<any>(null);
    const [searching, setSearching] = useState(false);

    // Filters
    const [showExpired, setShowExpired] = useState(false);

    const loadStats = async () => {
        setLoading(true);
        try {
            const data = await fetchVectorStats();
            setStats(data);
        } catch (error) {
            console.warn("[Vector] Failed to load vector stats:", error);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadStats();
        const interval = setInterval(loadStats, 10000);
        return () => clearInterval(interval);
    }, []);



    const handleSearch = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        if (!searchQuery.trim()) return;
        setSearching(true);
        try {
            const results = await searchVectors(searchQuery);
            setSearchResults(results);
        } catch (error) {
            console.warn("[Vector] Search failed:", error);
        } finally {
            setSearching(false);
        }
    };

    return (
        <div className="container mx-auto p-8">
            <div className="mb-8">
                <Button asChild variant="ghost" size="sm" className="mb-4">
                    <Link href="/">
                        <ArrowLeft className="mr-2 h-4 w-4" />
                        Back to Dashboard
                    </Link>
                </Button>
                <div className="flex justify-between items-end">
                    <div>
                        <h1 className="text-3xl font-bold tracking-tight">Vector Management</h1>
                        <p className="text-muted-foreground">Monitor and manage Qdrant vector storage and indexing.</p>
                    </div>
                    <div className="flex gap-2">
                        <Button variant="outline" size="sm" onClick={loadStats} disabled={loading}>
                            <RefreshCw className={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
                            Refresh
                        </Button>

                    </div>
                </div>
            </div>

            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 mb-8">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Qdrant Points</CardTitle>
                        <Database className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats?.database?.qdrant_points ?? stats?.qdrant?.result?.points_count ?? "-"}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Total Chunks</CardTitle>
                        <Database className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats?.database?.total_chunks ?? "-"}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Chunk Sync</CardTitle>
                        <CheckCircle2 className={`h-4 w-4 ${(stats?.database?.chunk_sync_pct ?? 0) >= 100 ? 'text-green-500' : 'text-yellow-500'}`} />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">
                            {stats?.database?.chunk_sync_pct != null ? `${Math.round(stats.database.chunk_sync_pct)}%` : "-"}
                        </div>
                        <p className="text-xs text-muted-foreground mt-1">
                            {stats?.database?.qdrant_points ?? 0} / {stats?.database?.total_chunks ?? 0} indexed
                        </p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Q/A Pairs</CardTitle>
                        <AlertCircle className={`h-4 w-4 ${stats?.database?.pending_golden > 0 ? 'text-yellow-500' : 'text-muted-foreground'}`} />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{stats?.database?.total_qa ?? "-"}</div>
                        <p className="text-xs text-muted-foreground mt-1">
                            {stats?.database?.indexed_qa ?? 0} indexed, {stats?.database?.pending_golden ?? 0} golden pending
                        </p>
                    </CardContent>
                </Card>
            </div>

            <div className="grid gap-8 md:grid-cols-1 lg:grid-cols-3">
                <div className="lg:col-span-2">
                    <Card className="h-full">
                        <CardHeader>
                            <CardTitle>Vector Search Preview</CardTitle>
                            <CardDescription>Test how your RAG agent will find context by searching the vector space.</CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="flex gap-4 mb-4">
                                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                    <Filter className="h-3 w-3" />
                                    Searching as tenant: <span className="font-mono font-medium text-foreground">{Cookies.get("tenant_id") || "default"}</span>
                                </div>
                                <div className="flex items-center gap-2">
                                    <input
                                        type="checkbox"
                                        id="showExpired"
                                        checked={showExpired}
                                        onChange={(e) => setShowExpired(e.target.checked)}
                                        className="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                                    />
                                    <label htmlFor="showExpired" className="text-sm cursor-pointer">Show Expired Data</label>
                                </div>
                            </div>
                            <form onSubmit={handleSearch} className="flex gap-2 mb-6">
                                <Input
                                    placeholder="Search your knowledge base (e.g., What is sleep apnea?)"
                                    value={searchQuery}
                                    onChange={(e: React.ChangeEvent<HTMLInputElement>) => setSearchQuery(e.target.value)}
                                />
                                <Button type="submit" disabled={searching}>
                                    <Search className={`mr-2 h-4 w-4 ${searching ? 'animate-pulse' : ''}`} />
                                    Search
                                </Button>
                            </form>

                            {searchResults && (
                                <div className="space-y-4">
                                    <h3 className="font-semibold text-sm uppercase tracking-wider text-muted-foreground">Results</h3>
                                    <div className="rounded-md border">
                                        <Table>
                                            <TableHeader>
                                                <TableRow>
                                                    <TableHead className="w-[80px]">Score</TableHead>
                                                    <TableHead>Retrieved Content</TableHead>
                                                    <TableHead className="w-[150px]">Source</TableHead>
                                                </TableRow>
                                            </TableHeader>
                                            <TableBody>
                                                {(searchResults.result?.points ?? searchResults.result ?? []).map((res: any) => (
                                                    <TableRow key={res.id}>
                                                        <TableCell className="font-mono text-xs">
                                                            {(res.score * 100).toFixed(1)}%
                                                        </TableCell>
                                                        <TableCell>
                                                            {res.payload.question ? (
                                                                <>
                                                                    <div className="text-sm font-medium mb-1">Q: {res.payload.question}</div>
                                                                    <div className="text-xs text-muted-foreground line-clamp-2 mb-2">A: {res.payload.answer}</div>
                                                                </>
                                                            ) : (
                                                                <div className="text-sm text-muted-foreground line-clamp-4">{res.payload.content}</div>
                                                            )}
                                                            <div className="flex flex-wrap items-center gap-2 mt-2">
                                                                <Badge variant="outline" className="text-[10px] bg-blue-50/50 text-blue-700">
                                                                    <FileText className="w-3 h-3 mr-1" /> Source #{res.payload.source_id ?? res.payload.source ?? '?'}
                                                                </Badge>
                                                                {res.payload.is_active !== false && (
                                                                    <Badge variant="outline" className="text-[10px] bg-green-50/50 text-green-700">
                                                                        <CheckCircle2 className="w-3 h-3 mr-1" /> Active
                                                                    </Badge>
                                                                )}
                                                            </div>
                                                        </TableCell>
                                                        <TableCell className="align-top">
                                                            <div className="text-xs font-mono">ID: {res.id}</div>
                                                            <div className="text-[10px] text-muted-foreground">Chunk #{res.payload.chunk_id ?? res.payload.chunk ?? '?'}</div>
                                                        </TableCell>
                                                    </TableRow>
                                                ))}
                                                {(searchResults.result?.points ?? searchResults.result ?? []).length === 0 && (
                                                    <TableRow>
                                                        <TableCell colSpan={3} className="text-center py-8 text-muted-foreground">
                                                            No matches found for your query.
                                                        </TableCell>
                                                    </TableRow>
                                                )}
                                            </TableBody>
                                        </Table>
                                    </div>
                                </div>
                            )}
                            {!searchResults && !searching && (
                                <div className="flex flex-col items-center justify-center py-20 text-muted-foreground border-2 border-dashed rounded-lg">
                                    <Search className="h-12 w-12 mb-4 opacity-20" />
                                    <p>Search the vector database to see retrieved fragments.</p>
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </div>

                <div>
                    <Card className="h-full">
                        <CardHeader>
                            <CardTitle>Collection Config</CardTitle>
                            <CardDescription>
                                {(() => {
                                    const vectors = stats?.qdrant?.result?.config?.params?.vectors;
                                    const dims = vectors?.dense?.size ?? vectors?.size ?? '?';
                                    const hasSparse = !!stats?.qdrant?.result?.config?.params?.sparse_vectors?.bm25;
                                    return `Dense: ${dims} dims${hasSparse ? ' + BM25 Sparse' : ''}`;
                                })()}
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="space-y-4">
                                <div className="p-3 bg-muted rounded-md font-mono text-[10px] overflow-auto max-h-[400px]">
                                    <pre>{JSON.stringify(stats?.qdrant?.result?.config, null, 2)}</pre>
                                </div>
                                <div className="flex items-center gap-2 text-xs font-medium text-green-600 bg-green-50 p-2 rounded border border-green-100">
                                    <CheckCircle2 className="h-3 w-3" />
                                    Hybrid Search Ready (Sparse Vectors)
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    );
}
