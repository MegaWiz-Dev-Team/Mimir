"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import {
    BarChart3, Database, MessageSquare, Cpu, Share2,
    Loader2, RefreshCw, AlertTriangle, ArrowUpDown, ChevronDown
} from "lucide-react";
import {
    fetchCoverageOverview, fetchCoverageSources, fetchCoverageGaps,
    CoverageOverview, SourceCoverage, CoverageGaps,
} from "@/lib/api";

// ─── Score color helper ────────────────────────────────────────────────────────

function scoreColor(score: number): string {
    if (score >= 75) return "text-green-600 dark:text-green-400";
    if (score >= 50) return "text-yellow-600 dark:text-yellow-400";
    if (score >= 25) return "text-amber-600 dark:text-amber-400";
    return "text-red-600 dark:text-red-400";
}

function scoreBg(score: number): string {
    if (score >= 75) return "bg-green-100 dark:bg-green-900/30";
    if (score >= 50) return "bg-yellow-100 dark:bg-yellow-900/30";
    if (score >= 25) return "bg-amber-100 dark:bg-amber-900/30";
    return "bg-red-100 dark:bg-red-900/30";
}

function blindspotLabel(spot: string): string {
    const map: Record<string, string> = {
        no_chunks: "No Chunks",
        no_qa_pairs: "No QA",
        low_vector_coverage: "Low Vectors",
        no_kg_entities: "No KG",
        high_dedup_ratio: "High Dedup",
        stale_data: "Stale",
    };
    return map[spot] || spot;
}

type SortKey = "name" | "coverage_score" | "chunk_count" | "qa_count" | "kg_entity_count";

export default function CoveragePage() {
    const [overview, setOverview] = useState<CoverageOverview | null>(null);
    const [sources, setSources] = useState<SourceCoverage[]>([]);
    const [gaps, setGaps] = useState<CoverageGaps | null>(null);
    const [loading, setLoading] = useState(true);
    const [sortKey, setSortKey] = useState<SortKey>("coverage_score");
    const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
    const [gapFilter, setGapFilter] = useState<string | null>(null);

    const loadData = useCallback(async () => {
        setLoading(true);
        try {
            const [ov, src, gp] = await Promise.all([
                fetchCoverageOverview(),
                fetchCoverageSources(),
                fetchCoverageGaps(),
            ]);
            setOverview(ov);
            setSources(src);
            setGaps(gp);
        } catch (error) {
            console.warn("[Coverage] Failed to load:", error);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadData();
    }, [loadData]);

    const handleSort = (key: SortKey) => {
        if (sortKey === key) {
            setSortDir(d => d === "asc" ? "desc" : "asc");
        } else {
            setSortKey(key);
            setSortDir("desc");
        }
    };

    // Filter and sort sources
    const filteredSources = sources
        .filter(s => {
            if (!gapFilter) return true;
            if (gapFilter === "missing_chunks") return s.chunk_count === 0;
            if (gapFilter === "missing_qa") return s.qa_count === 0;
            if (gapFilter === "missing_vectors") return s.vector_coverage_pct === 0;
            if (gapFilter === "missing_kg") return s.kg_entity_count === 0;
            if (gapFilter === "stale") return s.blindspots.includes("stale_data");
            if (gapFilter === "high_dedup") return s.blindspots.includes("high_dedup_ratio");
            return true;
        })
        .sort((a, b) => {
            const aVal = a[sortKey] ?? 0;
            const bVal = b[sortKey] ?? 0;
            if (typeof aVal === "string" && typeof bVal === "string") {
                return sortDir === "asc" ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal);
            }
            return sortDir === "asc" ? (aVal as number) - (bVal as number) : (bVal as number) - (aVal as number);
        });

    // KPI card data
    const kpiCards = overview ? [
        {
            title: "Source Coverage",
            value: overview.total_sources > 0
                ? `${Math.round((overview.sources_with_chunks / overview.total_sources) * 100)}%`
                : "0%",
            subtitle: `${overview.sources_with_chunks} / ${overview.total_sources} sources`,
            icon: Database,
            color: "text-blue-500",
            bg: "bg-blue-50 dark:bg-blue-950/30",
        },
        {
            title: "QA Coverage",
            value: overview.total_sources > 0
                ? `${Math.round((overview.sources_with_qa / overview.total_sources) * 100)}%`
                : "0%",
            subtitle: `${overview.sources_with_qa} / ${overview.total_sources} sources`,
            icon: MessageSquare,
            color: "text-emerald-500",
            bg: "bg-emerald-50 dark:bg-emerald-950/30",
        },
        {
            title: "Vector Coverage",
            value: overview.total_sources > 0
                ? `${Math.round((overview.sources_with_vectors / overview.total_sources) * 100)}%`
                : "0%",
            subtitle: `${overview.sources_with_vectors} / ${overview.total_sources} sources`,
            icon: Cpu,
            color: "text-purple-500",
            bg: "bg-purple-50 dark:bg-purple-950/30",
        },
        {
            title: "KG Coverage",
            value: overview.total_sources > 0
                ? `${Math.round((overview.sources_with_kg / overview.total_sources) * 100)}%`
                : "0%",
            subtitle: `${overview.sources_with_kg} / ${overview.total_sources} sources`,
            icon: Share2,
            color: "text-amber-500",
            bg: "bg-amber-50 dark:bg-amber-950/30",
        },
    ] : [];

    // Pipeline stages
    const pipelineStages = overview ? [
        { label: "Ingested", count: overview.pipeline_stages.ingested, color: "bg-blue-500" },
        { label: "Chunked", count: overview.pipeline_stages.chunked, color: "bg-emerald-500" },
        { label: "QA Generated", count: overview.pipeline_stages.qa_generated, color: "bg-violet-500" },
        { label: "Vectorized", count: overview.pipeline_stages.vectorized, color: "bg-purple-500" },
        { label: "KG Extracted", count: overview.pipeline_stages.kg_extracted, color: "bg-amber-500" },
    ] : [];

    const maxStageCount = pipelineStages.length > 0
        ? Math.max(...pipelineStages.map(s => s.count), 1)
        : 1;

    // Gap counts
    const gapItems = gaps ? [
        { key: "missing_chunks", label: "Missing Chunks", count: gaps.sources_missing_chunks.length, icon: "📦" },
        { key: "missing_qa", label: "Missing QA", count: gaps.sources_missing_qa.length, icon: "❓" },
        { key: "missing_vectors", label: "Missing Vectors", count: gaps.sources_missing_vectors.length, icon: "🔍" },
        { key: "missing_kg", label: "Missing KG", count: gaps.sources_missing_kg.length, icon: "🕸️" },
        { key: "stale", label: "Stale Sources", count: gaps.stale_sources.length, icon: "⏰" },
        { key: "high_dedup", label: "High Dedup", count: gaps.high_dedup_sources.length, icon: "📋" },
    ] : [];

    // Sort header component
    const SortHeader = ({ label, sortField }: { label: string; sortField: SortKey }) => (
        <TableHead
            className="cursor-pointer hover:text-foreground select-none"
            onClick={() => handleSort(sortField)}
        >
            <div className="flex items-center gap-1">
                {label}
                {sortKey === sortField && (
                    <ChevronDown className={`w-3 h-3 transition-transform ${sortDir === "asc" ? "rotate-180" : ""}`} />
                )}
            </div>
        </TableHead>
    );

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <BarChart3 className="w-6 h-6 text-emerald-500" />
                        Coverage Analytics
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1">
                        Track knowledge coverage, identify blind-spots, and monitor pipeline health
                    </p>
                </div>
                <Button variant="outline" size="sm" onClick={loadData} disabled={loading}>
                    <RefreshCw className={`w-4 h-4 mr-1.5 ${loading ? "animate-spin" : ""}`} />
                    Refresh
                </Button>
            </div>

            {loading ? (
                <div className="flex items-center justify-center py-20">
                    <Loader2 className="w-8 h-8 animate-spin text-emerald-500" />
                </div>
            ) : (
                <>
                    {/* KPI Cards */}
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                        {kpiCards.map((kpi) => {
                            const Icon = kpi.icon;
                            return (
                                <Card key={kpi.title} className="relative overflow-hidden">
                                    <CardContent className="p-5">
                                        <div className="flex items-center justify-between">
                                            <div>
                                                <p className="text-sm text-muted-foreground">{kpi.title}</p>
                                                <p className="text-2xl font-bold mt-1">{kpi.value}</p>
                                                <p className="text-xs text-muted-foreground mt-1">{kpi.subtitle}</p>
                                            </div>
                                            <div className={`p-3 rounded-full ${kpi.bg}`}>
                                                <Icon className={`w-5 h-5 ${kpi.color}`} />
                                            </div>
                                        </div>
                                    </CardContent>
                                </Card>
                            );
                        })}
                    </div>

                    {/* Overall Score */}
                    {overview && (
                        <Card>
                            <CardContent className="p-5">
                                <div className="flex items-center justify-between">
                                    <div>
                                        <p className="text-sm text-muted-foreground">Overall Coverage Score</p>
                                        <p className={`text-3xl font-bold mt-1 ${scoreColor(overview.overall_score)}`}>
                                            {overview.overall_score.toFixed(1)}%
                                        </p>
                                    </div>
                                    <div className="w-64 hidden md:block">
                                        <div className="h-3 bg-muted rounded-full overflow-hidden">
                                            <div
                                                className={`h-full rounded-full transition-all ${overview.overall_score >= 75 ? "bg-green-500" :
                                                    overview.overall_score >= 50 ? "bg-yellow-500" :
                                                        overview.overall_score >= 25 ? "bg-amber-500" : "bg-red-500"
                                                    }`}
                                                style={{ width: `${Math.min(overview.overall_score, 100)}%` }}
                                            />
                                        </div>
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                    )}

                    {/* Pipeline Flow */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2 text-lg">
                                <ArrowUpDown className="w-5 h-5 text-emerald-500" />
                                Pipeline Flow
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="space-y-3">
                                {pipelineStages.map((stage, i) => (
                                    <div key={stage.label} className="flex items-center gap-3">
                                        <div className="w-28 text-sm text-right text-muted-foreground flex-shrink-0">
                                            {stage.label}
                                        </div>
                                        <div className="flex-1 flex items-center gap-3">
                                            <div className="flex-1 h-6 bg-muted rounded-full overflow-hidden">
                                                <div
                                                    className={`h-full ${stage.color} rounded-full transition-all flex items-center justify-center`}
                                                    style={{
                                                        width: `${Math.max((stage.count / maxStageCount) * 100, stage.count > 0 ? 8 : 0)}%`,
                                                        minWidth: stage.count > 0 ? "2rem" : 0,
                                                    }}
                                                >
                                                    {stage.count > 0 && (
                                                        <span className="text-[10px] font-bold text-white">{stage.count}</span>
                                                    )}
                                                </div>
                                            </div>
                                            {i < pipelineStages.length - 1 && (
                                                <span className="text-muted-foreground text-xs">→</span>
                                            )}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </CardContent>
                    </Card>

                    {/* Gap Analysis Panel */}
                    {gapItems.length > 0 && (
                        <Card>
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-lg">
                                    <AlertTriangle className="w-5 h-5 text-amber-500" />
                                    Gap Analysis
                                    {gapFilter && (
                                        <Button variant="ghost" size="sm" className="ml-2 text-xs h-6" onClick={() => setGapFilter(null)}>
                                            Clear filter ✕
                                        </Button>
                                    )}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
                                    {gapItems.map((gap) => (
                                        <button
                                            key={gap.key}
                                            onClick={() => setGapFilter(gapFilter === gap.key ? null : gap.key)}
                                            className={`p-3 rounded-lg border text-center transition-all hover:shadow-sm cursor-pointer ${gapFilter === gap.key
                                                ? "border-emerald-500 bg-emerald-50 dark:bg-emerald-900/20"
                                                : "border-border hover:border-muted-foreground"
                                                }`}
                                        >
                                            <div className="text-lg mb-1">{gap.icon}</div>
                                            <div className="text-2xl font-bold">{gap.count}</div>
                                            <div className="text-xs text-muted-foreground mt-0.5">{gap.label}</div>
                                        </button>
                                    ))}
                                </div>
                            </CardContent>
                        </Card>
                    )}

                    {/* Per-Source Coverage Table */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2 text-lg">
                                <Database className="w-5 h-5 text-blue-500" />
                                Per-Source Coverage
                                {gapFilter && (
                                    <Badge variant="secondary" className="ml-2 text-xs">
                                        Filtered: {gapItems.find(g => g.key === gapFilter)?.label}
                                    </Badge>
                                )}
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            {filteredSources.length > 0 ? (
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <SortHeader label="Name" sortField="name" />
                                            <TableHead>Type</TableHead>
                                            <TableHead>Status</TableHead>
                                            <SortHeader label="Chunks" sortField="chunk_count" />
                                            <SortHeader label="QA" sortField="qa_count" />
                                            <TableHead className="text-right">Vectors</TableHead>
                                            <SortHeader label="KG" sortField="kg_entity_count" />
                                            <SortHeader label="Score" sortField="coverage_score" />
                                            <TableHead>Blind-spots</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {filteredSources.map((src) => (
                                            <TableRow key={src.source_id}>
                                                <TableCell className="font-medium">{src.name}</TableCell>
                                                <TableCell>
                                                    <Badge variant="outline" className="text-xs">{src.source_type}</Badge>
                                                </TableCell>
                                                <TableCell>
                                                    <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${src.status === "COMPLETED"
                                                        ? "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"
                                                        : src.status === "FAILED"
                                                            ? "bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300"
                                                            : src.status === "RUNNING"
                                                                ? "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
                                                                : "bg-gray-100 text-gray-700 dark:bg-gray-900/40 dark:text-gray-300"
                                                        }`}>
                                                        {src.status}
                                                    </span>
                                                </TableCell>
                                                <TableCell className="text-right">{src.chunk_count.toLocaleString()}</TableCell>
                                                <TableCell className="text-right">{src.qa_count.toLocaleString()}</TableCell>
                                                <TableCell className="text-right">
                                                    {src.vector_coverage_pct > 0 ? `${src.vector_coverage_pct.toFixed(0)}%` : "—"}
                                                </TableCell>
                                                <TableCell className="text-right">{src.kg_entity_count.toLocaleString()}</TableCell>
                                                <TableCell>
                                                    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-bold ${scoreBg(src.coverage_score)} ${scoreColor(src.coverage_score)}`}>
                                                        {src.coverage_score}%
                                                    </span>
                                                </TableCell>
                                                <TableCell>
                                                    <div className="flex flex-wrap gap-1">
                                                        {src.blindspots.length === 0 ? (
                                                            <span className="text-xs text-green-600 dark:text-green-400">✓ All clear</span>
                                                        ) : (
                                                            src.blindspots.map((spot) => (
                                                                <Badge key={spot} variant="destructive" className="text-[10px] px-1.5 py-0">
                                                                    {blindspotLabel(spot)}
                                                                </Badge>
                                                            ))
                                                        )}
                                                    </div>
                                                </TableCell>
                                            </TableRow>
                                        ))}
                                    </TableBody>
                                </Table>
                            ) : (
                                <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
                                    <BarChart3 className="w-10 h-10 mb-3 opacity-50" />
                                    <p className="text-sm">
                                        {gapFilter ? "No sources match this filter" : "No data sources found"}
                                    </p>
                                    <p className="text-xs mt-1">
                                        {gapFilter ? "Try clearing the filter" : "Add data sources to see coverage analytics"}
                                    </p>
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </>
            )}
        </div>
    );
}
