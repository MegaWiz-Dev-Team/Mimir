"use client";

/**
 * Benchmark Registry — Sprint 40 B-36f
 *
 * Lists every benchmark dataset available to the current tenant + global.
 * For each: source, scoring_fn (rubric type), n items, license info,
 * and whether `eir` (or current default agent) has a run on it yet.
 */

import { useEffect, useState } from "react";
import Link from "next/link";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ArrowLeft, RefreshCw, BookOpen, Database, ExternalLink } from "lucide-react";
import { authFetch, API_BASE_URL, fetchBenchmarkDatasets, BenchmarkDataset } from "@/lib/api";

const SCORING_FN_LABEL: Record<string, { label: string; tooltip: string; color: string }> = {
    healthbench_likert: {
        label: "HBp%",
        tooltip: "4-dim Likert (acc/comp/rel/safe) normalized to 0-100. Used by HealthBench-Pro and Mimir-internal benchmarks.",
        color: "bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300",
    },
    mcq_accuracy: {
        label: "Acc% (MCQ)",
        tooltip: "Exact-match accuracy on multiple-choice questions. % of items where model picked the correct option.",
        color: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300",
    },
    binary_yes_no: {
        label: "Acc% (Y/N/Maybe)",
        tooltip: "Accuracy on Y/N/Maybe questions over a context document. Same scale as MCQ accuracy.",
        color: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300",
    },
    paper_rubric_pct: {
        label: "Rubric%",
        tooltip: "% of physician-authored rubric criteria met. Used by HealthBench paper-original (gpt-4.1 grader).",
        color: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300",
    },
};

export default function BenchmarksPage() {
    const [datasets, setDatasets] = useState<BenchmarkDataset[]>([]);
    const [loading, setLoading] = useState(true);

    const load = async () => {
        setLoading(true);
        try {
            const data = await fetchBenchmarkDatasets();
            setDatasets(data);
        } catch (e) {
            console.error("Failed to load benchmarks", e);
        } finally {
            setLoading(false);
        }
    };
    useEffect(() => { load(); }, []);

    const fmtDate = (iso: string | null) => iso
        ? new Date(iso).toLocaleString("sv-SE", { hour12: false }).slice(0, 16)
        : "—";

    return (
        <div className="container mx-auto p-8 max-w-7xl">
            <div className="flex justify-between items-end mb-8">
                <div>
                    <Button asChild variant="ghost" size="sm" className="mb-1">
                        <Link href="/"><ArrowLeft className="mr-1 h-4 w-4" /> Back</Link>
                    </Button>
                    <h1 className="text-3xl font-bold tracking-tight">Benchmarks</h1>
                    <p className="text-muted-foreground">
                        All evaluation datasets registered in Mimir — internal HealthBench-Pro plus
                        downloaded medical benchmarks (MedQA · MedMCQA · PubMedQA · HealthBench · MedXpertQA)
                    </p>
                </div>
                <Button variant="outline" size="sm" onClick={load} disabled={loading}>
                    <RefreshCw className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`} /> Refresh
                </Button>
            </div>

            {/* Scoring legend — explain what HBp% / Acc% / Rubric% mean */}
            <Card className="mb-6">
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm">📐 Scoring functions</CardTitle>
                </CardHeader>
                <CardContent className="text-xs space-y-1">
                    {Object.entries(SCORING_FN_LABEL).map(([k, v]) => (
                        <div key={k} className="flex items-start gap-2">
                            <span className={`px-2 py-0.5 rounded font-mono text-[10px] ${v.color} shrink-0`}>{v.label}</span>
                            <span className="text-muted-foreground">{v.tooltip}</span>
                        </div>
                    ))}
                </CardContent>
            </Card>

            {/* Group by tenant_id (global vs current) */}
            {(() => {
                const global = datasets.filter(d => d.tenant_id === "__global__");
                const tenant = datasets.filter(d => d.tenant_id !== "__global__");
                return (
                    <>
                        {global.length > 0 && (
                            <div className="mb-8">
                                <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase tracking-wide">🌐 Global benchmarks (shared)</h2>
                                <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                                    {global.map(d => <DatasetCard key={d.id} d={d} fmtDate={fmtDate} />)}
                                </div>
                            </div>
                        )}
                        {tenant.length > 0 && (
                            <div>
                                <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase tracking-wide">🏠 Tenant benchmarks</h2>
                                <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                                    {tenant.map(d => <DatasetCard key={d.id} d={d} fmtDate={fmtDate} />)}
                                </div>
                            </div>
                        )}
                        {!loading && datasets.length === 0 && (
                            <Card><CardContent className="p-8 text-center text-muted-foreground">
                                No benchmarks registered. Run <code>scripts/load_medical_benchmarks_to_db.py</code>.
                            </CardContent></Card>
                        )}
                    </>
                );
            })()}
        </div>
    );
}

function DatasetCard({ d, fmtDate }: {
    d: BenchmarkDataset;
    fmtDate: (iso: string | null) => string;
}) {
    const sf = SCORING_FN_LABEL[d.scoring_fn] || { label: d.scoring_fn, tooltip: "", color: "bg-gray-100 text-gray-700" };
    return (
        <Card>
            <CardHeader className="pb-3">
                <div className="flex items-start justify-between gap-2">
                    <div className="flex items-start gap-2 min-w-0">
                        <div className="w-9 h-9 rounded-lg flex items-center justify-center bg-violet-100 dark:bg-violet-900/30 shrink-0">
                            <BookOpen className="w-5 h-5 text-violet-700 dark:text-violet-400" />
                        </div>
                        <div className="min-w-0">
                            <CardTitle className="text-base truncate" title={d.name}>{d.name}</CardTitle>
                            <p className="text-xs text-muted-foreground mt-0.5 font-mono truncate">{d.id}</p>
                        </div>
                    </div>
                    <span className={`text-[10px] font-mono px-1.5 py-0.5 rounded ${sf.color} shrink-0`}
                          title={sf.tooltip}>
                        {sf.label}
                    </span>
                </div>
            </CardHeader>
            <CardContent className="space-y-2 text-xs">
                <div><span className="text-muted-foreground">Source:</span> <code className="text-[11px]">{d.source}</code></div>
                <div><span className="text-muted-foreground">Items:</span> <span className="tabular-nums">{d.total_items.toLocaleString()}</span></div>
                <div><span className="text-muted-foreground">Version:</span> v{d.version}</div>
                {d.description && (
                    <div className="text-[11px] text-muted-foreground italic line-clamp-3">{d.description}</div>
                )}
                <div className="text-[10px] text-muted-foreground pt-1 border-t border-gray-100 dark:border-zinc-800">
                    Updated: {fmtDate(d.updated_at)}
                </div>
                <Button asChild size="sm" variant="outline" className="w-full mt-1">
                    <Link href={`/evaluations?benchmark=${encodeURIComponent(d.id)}`}>
                        View runs <ExternalLink className="ml-1 w-3 h-3" />
                    </Link>
                </Button>
            </CardContent>
        </Card>
    );
}
