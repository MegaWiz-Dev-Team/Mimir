"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge";
import { RefreshCw, ArrowLeft, ChevronDown, ChevronUp, Star, Clock, Target, CheckCircle, GitCompare, BarChart3, ThumbsUp, ThumbsDown, Beaker, Database, Workflow, Brain, Loader2, Sparkles, AlertTriangle, Crown } from "lucide-react";
import { fetchRunInsights, regenerateRunInsights, diagnoseScore, explainRetrieval, promoteRun, RunInsight } from "@/lib/api";
import Link from "next/link";
import { EvalWizard } from "@/components/evaluations/eval-wizard";
import { EvalScoreOverride } from "@/components/evaluations/eval-score-override";
import { compareModels, getFeedbackSummary, authFetch, API_BASE_URL } from "@/lib/api";

const API_BASE = API_BASE_URL;

// ─── Types ─────────────────────────────────────────────────────────────
interface EvalRun {
    id: string;
    name: string | null;
    status: string;
    total_combinations: number;
    completed_combinations: number;
    started_at: string;
    finished_at: string | null;
    total_prompt_tokens?: number | null;
    total_completion_tokens?: number | null;
    total_thinking_tokens?: number | null;
    // Wave 1 fields
    parent_run_id?: string | null;
    baseline_run_id?: string | null;
    hypothesis?: string | null;
    variable_under_test?: string | null;
    expected_change?: string | null;
    is_champion?: boolean;
    total_cost_usd?: number | null;
    // Sprint 40 — config carries benchmark_dataset_id; parsed lazily in RunsTable
    config?: string | null;
}

interface BenchmarkDatasetLite {
    id: string;
    name: string;
    source: string;
    scoring_fn: string;
    total_items: number;
}

interface EvalSummary {
    agent_name: string;
    model_id: string;
    total_questions: number;
    avg_accuracy: number | null;
    avg_completeness: number | null;
    avg_relevance: number | null;
    avg_latency_ms: number | null;
    overall_score: number | null;
}

interface MatrixResponse {
    agents: string[];
    models: string[];
    cells: EvalSummary[];
}

interface EvalScore {
    id: number;
    agent_name: string;
    model_id: string;
    question: string;
    expected_answer: string;
    actual_answer: string | null;
    accuracy_score: number | null;
    completeness_score: number | null;
    relevance_score: number | null;
    latency_ms: number | null;
    retrieval_latency_ms?: number | null;
    generation_latency_ms?: number | null;
    total_latency_ms?: number | null;
    ttft_ms?: number | null;
    prompt_tokens?: number | null;
    completion_tokens?: number | null;
    thinking_tokens?: number | null;
    difficulty?: string | null;
    question_type?: string | null;
    judge_model: string | null;
    judge_reasoning: string | null;
    safety_score?: number | null;
    rubric_score?: number | null;
    retrieval_trace?: string | null;   // JSON string from Wave 1
    benchmark_item_id?: string | null;
    replicate_index?: number | null;
    tags?: string | null;               // JSON string {specialty, eval_type, difficulty}
    // Wave 3 — full retrieval trace
    retrieval_params?: string | null;   // JSON: {alpha, threshold, hop_limit, top_k, weights}
    retrieval_chunks?: string | null;   // JSON: [{source, title, score, content_preview}]
    step_timings?: string | null;       // JSON: {retrieval, generation, total}
    tool_calls?: string | null;         // JSON: tools_enabled list
    human_accuracy_score: number | null;
    human_completeness_score: number | null;
    human_relevance_score: number | null;
    human_notes: string | null;
    reviewed_by: string | null;
    reviewed_at: string | null;
}

// ─── Helpers ───────────────────────────────────────────────────────────

function scoreColor(score: number | null): string {
    if (score === null) return "bg-muted text-muted-foreground";
    if (score >= 4.0) return "bg-emerald-500/20 text-emerald-400 border border-emerald-500/30";
    if (score >= 3.0) return "bg-amber-500/20 text-amber-400 border border-amber-500/30";
    if (score >= 2.0) return "bg-orange-500/20 text-orange-400 border border-orange-500/30";
    return "bg-red-500/20 text-red-400 border border-red-500/30";
}

function scoreBg(score: number | null): string {
    if (score === null) return "bg-zinc-800/50";
    if (score >= 4.0) return "bg-emerald-500/15 hover:bg-emerald-500/25";
    if (score >= 3.0) return "bg-amber-500/15 hover:bg-amber-500/25";
    if (score >= 2.0) return "bg-orange-500/15 hover:bg-orange-500/25";
    return "bg-red-500/15 hover:bg-red-500/25";
}

function formatLatency(ms: number | null): string {
    if (ms === null) return "-";
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
}

// ─── Component ─────────────────────────────────────────────────────────

export default function EvaluationsPage() {
    const [runs, setRuns] = useState<EvalRun[]>([]);
    const [selectedRunId, setSelectedRunId] = useState<string>("");
    const [matrix, setMatrix] = useState<MatrixResponse | null>(null);
    const [scores, setScores] = useState<EvalScore[]>([]);
    const [expandedCell, setExpandedCell] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);
    const [loadingScores, setLoadingScores] = useState(false);
    const [activeTab, setActiveTab] = useState<"runs" | "matrix" | "performance" | "extraction" | "retrieval" | "pipeline" | "ai-analysis">("runs");
    const [expandedRow, setExpandedRow] = useState<number | null>(null);
    const [diagnoses, setDiagnoses] = useState<Record<number, any>>({});
    const [retrievalExplanations, setRetrievalExplanations] = useState<Record<number, any>>({});
    const [diagLoading, setDiagLoading] = useState<Record<number, boolean>>({});

    // ── Runs table state (Phase 1 redesign) ────────────────────────────
    const [runSummaries, setRunSummaries] = useState<Record<string, any>>({});
    const [sortBy, setSortBy] = useState<"score" | "cost" | "latency" | "started" | "name">("started");
    const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
    const [statusFilter, setStatusFilter] = useState<string>("ALL");
    // Sprint 40 B-36b: filter runs by benchmark dataset id (or "ALL")
    const [benchmarkFilter, setBenchmarkFilter] = useState<string>("ALL");
    // Sprint 40 B-36/B-36b: list of benchmarks (id → name/scoring_fn lookup)
    const [benchmarks, setBenchmarks] = useState<BenchmarkDatasetLite[]>([]);
    const [selectedForCompare, setSelectedForCompare] = useState<Set<string>>(new Set());
    const [expandedRunRow, setExpandedRunRow] = useState<string | null>(null);
    const [showCompareModal, setShowCompareModal] = useState(false);

    // Model Performance state
    const [modelA, setModelA] = useState("");
    const [modelB, setModelB] = useState("");
    const [comparison, setComparison] = useState<any>(null);
    const [feedbackSummary, setFeedbackSummary] = useState<any>(null);
    const [loadingComparison, setLoadingComparison] = useState(false);

    // Extraction eval state
    const [extractionData, setExtractionData] = useState<any>(null);
    const [loadingExtraction, setLoadingExtraction] = useState(false);

    // Retrieval eval state
    const [retrievalData, setRetrievalData] = useState<any>(null);
    const [loadingRetrieval, setLoadingRetrieval] = useState(false);

    // Pipeline scorecard state
    const [pipelineData, setPipelineData] = useState<any>(null);
    const [loadingPipeline, setLoadingPipeline] = useState(false);

    const loadRuns = useCallback(async () => {
        try {
            const res = await authFetch(`${API_BASE}/eval/runs`, { cache: "no-store" });
            if (!res.ok) return;
            const data: EvalRun[] = await res.json();
            setRuns(data);
            if (data.length > 0 && !selectedRunId) {
                setSelectedRunId(data[0].id);
            }
        } catch (e) {
            console.warn("[Evaluations] Failed to load eval runs:", e);
        } finally {
            setLoading(false);
        }
    }, [selectedRunId]);

    // Sprint 40 B-36b: load benchmark datasets (id → name + scoring_fn) for filter + per-benchmark logic
    const loadBenchmarks = useCallback(async () => {
        try {
            const res = await authFetch(`${API_BASE}/eval/benchmark-datasets`, { cache: "no-store" });
            if (!res.ok) return;
            const data: BenchmarkDatasetLite[] = await res.json();
            setBenchmarks(data);
        } catch (e) {
            console.warn("[Evaluations] Failed to load benchmark datasets:", e);
        }
    }, []);
    useEffect(() => { loadBenchmarks(); }, [loadBenchmarks]);

    const loadMatrix = useCallback(async (runId: string) => {
        try {
            const res = await authFetch(`${API_BASE}/eval/runs/${runId}/matrix`, { cache: "no-store" });
            if (!res.ok) return;
            const data: MatrixResponse = await res.json();
            setMatrix(data);
        } catch (e) {
            console.warn("[Evaluations] Failed to load matrix:", e);
        }
    }, []);

    const loadScores = useCallback(async (runId: string, agent: string, model: string) => {
        setLoadingScores(true);
        try {
            const res = await authFetch(
                `${API_BASE}/eval/runs/${runId}/scores?agent=${encodeURIComponent(agent)}&model=${encodeURIComponent(model)}`,
                { cache: "no-store" }
            );
            if (!res.ok) return;
            const data: EvalScore[] = await res.json();
            setScores(data);
        } catch (e) {
            console.warn("[Evaluations] Failed to load scores:", e);
        } finally {
            setLoadingScores(false);
        }
    }, []);

    useEffect(() => { loadRuns(); }, [loadRuns]);

    useEffect(() => {
        if (selectedRunId) {
            loadMatrix(selectedRunId);
            setExpandedCell(null);
            setScores([]);
        }
    }, [selectedRunId, loadMatrix]);

    // ── Phase 1: load summaries for each run so the Runs Table can show metrics ──
    useEffect(() => {
        const completed = runs.filter(r => r.status === "COMPLETED" && !runSummaries[r.id]);
        if (completed.length === 0) return;
        let cancelled = false;
        (async () => {
            for (const r of completed) {
                try {
                    const res = await authFetch(`${API_BASE}/eval/runs/${r.id}`, { cache: "no-store" });
                    if (!res.ok) continue;
                    const data = await res.json();
                    if (cancelled) return;
                    setRunSummaries(prev => ({ ...prev, [r.id]: data }));
                } catch {}
            }
        })();
        return () => { cancelled = true; };
    }, [runs]);  // eslint-disable-line react-hooks/exhaustive-deps

    const handleCellClick = (agent: string, model: string) => {
        const key = `${agent}|${model}`;
        if (expandedCell === key) {
            setExpandedCell(null);
            setScores([]);
        } else {
            setExpandedCell(key);
            loadScores(selectedRunId, agent, model);
        }
    };

    const getCellData = (agent: string, model: string): EvalSummary | undefined => {
        return matrix?.cells.find(c => c.agent_name === agent && c.model_id === model);
    };

    // Find best combination
    const bestCell = matrix?.cells.reduce<EvalSummary | null>((best, cell) => {
        if (!best || (cell.overall_score ?? 0) > (best.overall_score ?? 0)) return cell;
        return best;
    }, null);

    const selectedRun = runs.find(r => r.id === selectedRunId);

    const handleCompare = async () => {
        if (!modelA || !modelB) return;
        setLoadingComparison(true);
        try {
            const data = await compareModels(modelA, modelB);
            setComparison(data);
        } catch {
            setComparison(null);
        } finally {
            setLoadingComparison(false);
        }
    };

    useEffect(() => {
        if (activeTab === "performance") {
            getFeedbackSummary().then(setFeedbackSummary).catch(() => { });
        }
        if (activeTab === "extraction" && !extractionData) {
            setLoadingExtraction(true);
            authFetch(`${API_BASE}/evaluations/extraction-summary`, { cache: "no-store" })
                .then(r => r.json()).then(setExtractionData)
                .catch(console.warn).finally(() => setLoadingExtraction(false));
        }
        if (activeTab === "retrieval" && !retrievalData) {
            setLoadingRetrieval(true);
            authFetch(`${API_BASE}/evaluations/retrieval-summary`, { cache: "no-store" })
                .then(r => r.json()).then(setRetrievalData)
                .catch(console.warn).finally(() => setLoadingRetrieval(false));
        }
        if (activeTab === "pipeline" && !pipelineData) {
            setLoadingPipeline(true);
            authFetch(`${API_BASE}/evaluations/pipeline-scorecard`, { cache: "no-store" })
                .then(r => r.json()).then(setPipelineData)
                .catch(console.warn).finally(() => setLoadingPipeline(false));
        }
    }, [activeTab]);

    const allModels = matrix?.models || [];

    return (
        <div className="container mx-auto p-8 max-w-7xl">
            {/* Header */}
            <div className="flex justify-between items-end mb-8">
                <div>
                    <div className="flex items-center gap-3 mb-1">
                        <Button asChild variant="ghost" size="sm">
                            <Link href="/"><ArrowLeft className="mr-1 h-4 w-4" /> Back</Link>
                        </Button>
                    </div>
                    <h1 className="text-3xl font-bold tracking-tight">Agent Evaluation</h1>
                    <p className="text-muted-foreground">Agent × Model performance matrix with hybrid scoring</p>
                </div>

                <div className="flex items-end gap-3">
                    <div className="grid w-[280px] gap-1.5">
                        <label className="text-xs text-muted-foreground">Evaluation Run</label>
                        <Select value={selectedRunId} onValueChange={setSelectedRunId}>
                            <SelectTrigger className="h-9">
                                <SelectValue placeholder="Select run" />
                            </SelectTrigger>
                            <SelectContent>
                                {runs.map(r => (
                                    <SelectItem key={r.id} value={r.id}>
                                        {r.name || r.id.substring(0, 8)} — {r.status}
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>
                    <Button variant="outline" size="sm" className="h-9" onClick={loadRuns} disabled={loading}>
                        <RefreshCw className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                        Refresh
                    </Button>
                    <EvalWizard onTriggerRun={loadRuns} />
                </div>
            </div>

            {/* Tab Navigation */}
            <div className="flex gap-1 bg-zinc-100 dark:bg-zinc-900 rounded-lg p-1 mb-6 flex-wrap">
                <button onClick={() => setActiveTab("runs")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "runs" ? "bg-white dark:bg-zinc-800 shadow-sm border border-violet-200" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <BarChart3 className="w-4 h-4 text-violet-600" /> Runs
                </button>
                <button onClick={() => setActiveTab("matrix")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "matrix" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Target className="w-4 h-4" /> Run Detail
                </button>
                <button onClick={() => setActiveTab("ai-analysis")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "ai-analysis" ? "bg-white dark:bg-zinc-800 shadow-sm border border-violet-200" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Brain className="w-4 h-4 text-violet-600" /> AI Analysis
                </button>
                <button onClick={() => setActiveTab("performance")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "performance" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <GitCompare className="w-4 h-4" /> Cross-Run Compare
                </button>
                <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
                    <span>Pipeline health →</span>
                    <Link href="/rag-playground" className="hover:text-violet-600 underline-offset-2 hover:underline">RAG Playground</Link>
                </div>
            </div>

            {/* ─── Cross-Benchmark Leaderboard ─────────────────────────────── */}
            {activeTab === "runs" && (
                <CompositeLeaderboard runs={runs} summaries={runSummaries} benchmarks={benchmarks} />
            )}

            {/* ─── Runs Table (Phase 1 — primary view) ─────────────────────── */}
            {activeTab === "runs" && (
                <RunsTable
                    runs={runs}
                    summaries={runSummaries}
                    benchmarks={benchmarks}
                    sortBy={sortBy} setSortBy={setSortBy}
                    sortDir={sortDir} setSortDir={setSortDir}
                    statusFilter={statusFilter} setStatusFilter={setStatusFilter}
                    benchmarkFilter={benchmarkFilter} setBenchmarkFilter={setBenchmarkFilter}
                    selectedForCompare={selectedForCompare}
                    setSelectedForCompare={setSelectedForCompare}
                    expandedRunRow={expandedRunRow}
                    setExpandedRunRow={setExpandedRunRow}
                    onOpenRun={(id) => { setSelectedRunId(id); setActiveTab("matrix"); }}
                    onAnalyze={(id) => { setSelectedRunId(id); setActiveTab("ai-analysis"); }}
                    onPromote={async (id) => {
                        try { await promoteRun(id); await loadRuns(); }
                        catch (e: any) { alert("Promote failed: " + e.message); }
                    }}
                    onCompare={() => setShowCompareModal(true)}
                />
            )}

            {/* ─── Compare Modal (Phase 2) ───────────────────────────────── */}
            {showCompareModal && (
                <CompareModal
                    runIds={Array.from(selectedForCompare)}
                    runs={runs}
                    summaries={runSummaries}
                    onClose={() => setShowCompareModal(false)}
                />
            )}

            {activeTab === "matrix" && (<>

                {/* Summary Cards */}
                {selectedRun && (
                    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5 mb-8">
                        <Card>
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium">Status</CardTitle>
                                <Target className="h-4 w-4 text-muted-foreground" />
                            </CardHeader>
                            <CardContent>
                                <StatusBadge status={selectedRun.status} />
                                <div className="mt-3 space-y-1">
                                    <div className="flex justify-between text-xs mb-1">
                                        <span className="text-muted-foreground">Progress</span>
                                        <span className="font-medium">
                                            {selectedRun.total_combinations > 0
                                                ? Math.round((selectedRun.completed_combinations / selectedRun.total_combinations) * 100)
                                                : 0}%
                                        </span>
                                    </div>
                                    <div className="h-2 w-full bg-secondary rounded-full overflow-hidden">
                                        <div
                                            className="h-full bg-primary transition-all duration-500 ease-in-out"
                                            style={{ width: `${selectedRun.total_combinations > 0 ? (selectedRun.completed_combinations / selectedRun.total_combinations) * 100 : 0}%` }}
                                        />
                                    </div>
                                    <p className="text-[10px] text-muted-foreground mt-1 text-right">
                                        {selectedRun.completed_combinations} / {selectedRun.total_combinations}
                                    </p>
                                </div>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium">Best Combo</CardTitle>
                                <Star className="h-4 w-4 text-amber-400" />
                            </CardHeader>
                            <CardContent>
                                {bestCell ? (
                                    <>
                                        <div className="text-lg font-bold">{bestCell.agent_name}</div>
                                        <p className="text-xs text-muted-foreground">
                                            {bestCell.model_id} — Score: {bestCell.overall_score?.toFixed(2) ?? "N/A"}
                                        </p>
                                    </>
                                ) : (
                                    <div className="text-muted-foreground">No data</div>
                                )}
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium">Combinations</CardTitle>
                                <CheckCircle className="h-4 w-4 text-muted-foreground" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{matrix?.cells.length ?? 0}</div>
                                <p className="text-xs text-muted-foreground">
                                    {matrix?.agents.length ?? 0} agents × {matrix?.models.length ?? 0} models
                                </p>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium">Avg Latency</CardTitle>
                                <Clock className="h-4 w-4 text-muted-foreground" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">
                                    {matrix?.cells.length
                                        ? formatLatency(
                                            Math.round(
                                                matrix.cells.reduce((sum, c) => sum + (c.avg_latency_ms ?? 0), 0) /
                                                matrix.cells.filter(c => c.avg_latency_ms !== null).length
                                            )
                                        )
                                        : "-"}
                                </div>
                                <p className="text-xs text-muted-foreground">across all combinations</p>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium">Run Tokens</CardTitle>
                                <Workflow className="h-4 w-4 text-emerald-500" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-xl font-bold">
                                    {selectedRun.total_prompt_tokens !== undefined && selectedRun.total_prompt_tokens !== null ?
                                        (selectedRun.total_prompt_tokens + (selectedRun.total_completion_tokens || 0)).toLocaleString()
                                        : "N/A"
                                    }
                                </div>
                                {selectedRun.total_prompt_tokens ? (
                                    <div className="flex gap-2 text-[10px] text-muted-foreground mt-1">
                                        <span className="text-blue-500">↑{selectedRun.total_prompt_tokens?.toLocaleString()}</span>
                                        <span className="text-emerald-500">↓{selectedRun.total_completion_tokens?.toLocaleString()}</span>
                                        {selectedRun.total_thinking_tokens ? <span className="text-purple-500">💭{selectedRun.total_thinking_tokens?.toLocaleString()}</span> : null}
                                    </div>
                                ) : (
                                    <p className="text-xs text-muted-foreground">No token telemetry</p>
                                )}
                            </CardContent>
                        </Card>
                    </div>
                )}

                {/* Heatmap Matrix */}
                {matrix && matrix.cells.length > 0 && (
                    <Card className="mb-8">
                        <CardHeader>
                            <CardTitle>Performance Matrix</CardTitle>
                            <p className="text-sm text-muted-foreground">Click a cell to see per-question scores</p>
                        </CardHeader>
                        <CardContent className="overflow-x-auto">
                            <table className="w-full">
                                <thead>
                                    <tr>
                                        <th className="text-left p-3 text-sm font-medium text-muted-foreground">Agent \ Model</th>
                                        {matrix.models.map(model => (
                                            <th key={model} className="text-center p-3 text-xs font-medium text-muted-foreground whitespace-nowrap">
                                                {model}
                                            </th>
                                        ))}
                                    </tr>
                                </thead>
                                <tbody>
                                    {matrix.agents.map(agent => (
                                        <tr key={agent}>
                                            <td className="p-3 font-medium text-sm">{agent}</td>
                                            {matrix.models.map(model => {
                                                const cell = getCellData(agent, model);
                                                const key = `${agent}|${model}`;
                                                const isExpanded = expandedCell === key;

                                                return (
                                                    <td key={model} className="p-1">
                                                        <button
                                                            onClick={() => handleCellClick(agent, model)}
                                                            className={`w-full rounded-lg p-3 text-center transition-all cursor-pointer ${scoreBg(cell?.overall_score ?? null)} ${isExpanded ? "ring-2 ring-primary" : ""}`}
                                                        >
                                                            {cell ? (
                                                                <div>
                                                                    <div className="text-xl font-bold">{cell.overall_score?.toFixed(2) ?? "—"}</div>
                                                                    <div className="flex justify-center gap-1 mt-1">
                                                                        <span className={`text-[10px] px-1.5 py-0.5 rounded ${scoreColor(cell.avg_accuracy)}`}>
                                                                            A:{cell.avg_accuracy?.toFixed(1) ?? "-"}
                                                                        </span>
                                                                        <span className={`text-[10px] px-1.5 py-0.5 rounded ${scoreColor(cell.avg_completeness)}`}>
                                                                            C:{cell.avg_completeness?.toFixed(1) ?? "-"}
                                                                        </span>
                                                                        <span className={`text-[10px] px-1.5 py-0.5 rounded ${scoreColor(cell.avg_relevance)}`}>
                                                                            R:{cell.avg_relevance?.toFixed(1) ?? "-"}
                                                                        </span>
                                                                    </div>
                                                                    <div className="text-[10px] text-muted-foreground mt-1">
                                                                        {formatLatency(cell.avg_latency_ms)} · {cell.total_questions}q
                                                                    </div>
                                                                </div>
                                                            ) : (
                                                                <span className="text-muted-foreground text-xs">N/A</span>
                                                            )}
                                                            {isExpanded ? <ChevronUp className="h-3 w-3 mx-auto mt-1" /> : <ChevronDown className="h-3 w-3 mx-auto mt-1 opacity-30" />}
                                                        </button>
                                                    </td>
                                                );
                                            })}
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </CardContent>
                    </Card>
                )}

                {/* Expanded Score Detail */}
                {expandedCell && (
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-lg">
                                Detail: {expandedCell.replace("|", " × ")}
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            {loadingScores ? (
                                <div className="text-center py-8 text-muted-foreground">Loading scores...</div>
                            ) : scores.length === 0 ? (
                                <div className="text-center py-8 text-muted-foreground">No scores found</div>
                            ) : (
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <TableHead className="w-4">#</TableHead>
                                            <TableHead className="text-center w-16">Diff</TableHead>
                                            <TableHead>Question</TableHead>
                                            <TableHead>Expected</TableHead>
                                            <TableHead>Actual</TableHead>
                                            <TableHead className="text-center">Acc</TableHead>
                                            <TableHead className="text-center">Comp</TableHead>
                                            <TableHead className="text-center">Rel</TableHead>
                                            <TableHead className="text-center">Latency / Tokens</TableHead>
                                            <TableHead className="text-center">Human</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {scores.map((s, i) => {
                                            const isOpen = expandedRow === s.id;
                                            const tags = (() => { try { return s.tags ? JSON.parse(s.tags) : {}; } catch { return {}; } })();
                                            const trace = (() => { try { return s.retrieval_trace ? JSON.parse(s.retrieval_trace) : null; } catch { return null; } })();
                                            const handleDiagnose = async () => {
                                                setDiagLoading(p => ({ ...p, [s.id]: true }));
                                                try {
                                                    const r = await diagnoseScore(s.id);
                                                    setDiagnoses(p => ({ ...p, [s.id]: r }));
                                                } catch (e) { /* no-op */ }
                                                finally { setDiagLoading(p => ({ ...p, [s.id]: false })); }
                                            };
                                            const handleExplainRetrieval = async () => {
                                                setDiagLoading(p => ({ ...p, [-s.id]: true }));
                                                try {
                                                    const r = await explainRetrieval(s.id);
                                                    setRetrievalExplanations(p => ({ ...p, [s.id]: r }));
                                                } catch (e) { /* no-op */ }
                                                finally { setDiagLoading(p => ({ ...p, [-s.id]: false })); }
                                            };
                                            return (<>
                                            <TableRow key={s.id} className={isOpen ? "bg-violet-50/30 dark:bg-violet-900/10" : ""}>
                                                <TableCell className="text-muted-foreground text-xs">
                                                    <button onClick={() => setExpandedRow(isOpen ? null : s.id)}
                                                        className="hover:text-violet-600 transition-colors">
                                                        {isOpen ? "▼" : "▶"} {i + 1}
                                                    </button>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    {s.difficulty ? (
                                                        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                                                            s.difficulty === 'advanced' ? 'bg-red-500/20 text-red-400 border border-red-500/30' :
                                                            s.difficulty === 'intermediate' ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30' :
                                                            s.difficulty === 'beginner' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' :
                                                            'bg-zinc-500/20 text-zinc-400 border border-zinc-500/30'
                                                        }`}>
                                                            {s.difficulty}
                                                        </span>
                                                    ) : <span className="text-muted-foreground text-[10px]">—</span>}
                                                </TableCell>
                                                <TableCell className="max-w-[200px] truncate text-xs" title={s.question}>
                                                    {s.question}
                                                </TableCell>
                                                <TableCell className="max-w-[200px] truncate text-xs" title={s.expected_answer}>
                                                    {s.expected_answer}
                                                </TableCell>
                                                <TableCell className="max-w-[200px] truncate text-xs" title={s.actual_answer ?? ""}>
                                                    {s.actual_answer || <span className="text-muted-foreground">—</span>}
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${scoreColor(s.accuracy_score)}`}>
                                                        {s.accuracy_score ?? "-"}
                                                    </span>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${scoreColor(s.completeness_score)}`}>
                                                        {s.completeness_score ?? "-"}
                                                    </span>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${scoreColor(s.relevance_score)}`}>
                                                        {s.relevance_score ?? "-"}
                                                    </span>
                                                </TableCell>
                                                <TableCell className="text-center text-xs">
                                                    <div className="flex flex-col gap-1 items-center">
                                                        <span className="font-semibold">{formatLatency(s.latency_ms)}</span>
                                                        {(s.retrieval_latency_ms || s.generation_latency_ms) && (
                                                            <div className="flex gap-1.5 text-[9px] text-muted-foreground bg-secondary/50 px-1.5 py-0.5 rounded">
                                                                <span title="Retrieval">R:{formatLatency(s.retrieval_latency_ms ?? 0)}</span>
                                                                <span title="Generation">G:{formatLatency(s.generation_latency_ms ?? 0)}</span>
                                                            </div>
                                                        )}
                                                        {s.prompt_tokens ? (
                                                            <div className="flex gap-1.5 text-[9px]">
                                                                <span className="text-blue-500" title="Prompt Tokens">↑{s.prompt_tokens?.toLocaleString()}</span>
                                                                <span className="text-emerald-500" title="Completion Tokens">↓{s.completion_tokens?.toLocaleString()}</span>
                                                                {s.thinking_tokens ? <span className="text-purple-500" title="Thinking Tokens">💭{s.thinking_tokens?.toLocaleString()}</span> : null}
                                                                {s.ttft_ms ? <span className="text-orange-500" title="Gen round-trip (streaming not yet enabled)">⚡{formatLatency(s.ttft_ms)}</span> : null}
                                                            </div>
                                                        ) : null}
                                                    </div>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    {s.reviewed_at ? (
                                                        <div className="flex flex-col items-center gap-1 text-xs">
                                                            <span className="text-emerald-400">✓ Reviewed</span>
                                                            <span className="text-[10px] text-muted-foreground">{s.human_notes}</span>
                                                            <EvalScoreOverride
                                                                scoreId={s.id}
                                                                initialAccuracy={s.human_accuracy_score}
                                                                initialCompleteness={s.human_completeness_score}
                                                                initialRelevance={s.human_relevance_score}
                                                                initialNotes={s.human_notes}
                                                                onSaved={() => {
                                                                    if (expandedCell) {
                                                                        const [a, m] = expandedCell.split("|");
                                                                        loadScores(selectedRunId, a, m);
                                                                    }
                                                                }}
                                                            />
                                                        </div>
                                                    ) : (
                                                        <div className="flex flex-col items-center gap-1 text-xs">
                                                            <span className="text-muted-foreground">Pending</span>
                                                            <EvalScoreOverride
                                                                scoreId={s.id}
                                                                initialAccuracy={s.human_accuracy_score}
                                                                initialCompleteness={s.human_completeness_score}
                                                                initialRelevance={s.human_relevance_score}
                                                                initialNotes={s.human_notes}
                                                                onSaved={() => {
                                                                    if (expandedCell) {
                                                                        const [a, m] = expandedCell.split("|");
                                                                        loadScores(selectedRunId, a, m);
                                                                    }
                                                                }}
                                                            />
                                                        </div>
                                                    )}
                                                </TableCell>
                                            </TableRow>
                                            {isOpen && (
                                                <TableRow key={`${s.id}-expanded`} className="bg-violet-50/40 dark:bg-violet-900/15">
                                                    <TableCell colSpan={10} className="py-3">
                                                        <div className="space-y-3 text-xs">
                                                            {(tags.specialty || tags.eval_type || tags.difficulty) && (
                                                                <div className="flex gap-1.5 items-center flex-wrap">
                                                                    {tags.specialty && <span className="px-2 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 text-[10px]">specialty: {tags.specialty}</span>}
                                                                    {tags.eval_type && <span className="px-2 py-0.5 rounded bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400 text-[10px]">{tags.eval_type}</span>}
                                                                    {tags.difficulty && <span className="px-2 py-0.5 rounded bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400 text-[10px]">{tags.difficulty}</span>}
                                                                    {s.benchmark_item_id && <span className="px-2 py-0.5 rounded bg-gray-100 dark:bg-zinc-800 text-[10px] font-mono">item: {s.benchmark_item_id.slice(0, 16)}…</span>}
                                                                </div>
                                                            )}
                                                            {trace?.counts && (
                                                                <div>
                                                                    <span className="font-semibold text-muted-foreground">RAG Retrieval:</span>
                                                                    <span className="ml-2 inline-flex gap-1.5 flex-wrap">
                                                                        {Object.entries(trace.counts as Record<string, number>).map(([src, n]) => (
                                                                            <span key={src} className={`px-1.5 py-0.5 rounded text-[10px] ${n > 0 ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400' : 'bg-gray-100 text-gray-400 dark:bg-zinc-800'}`}>
                                                                                {src}:{n}
                                                                            </span>
                                                                        ))}
                                                                    </span>
                                                                </div>
                                                            )}
                                                            {/* Wave 3 — retrieval params + timings + chunks */}
                                                            {(() => { try { return s.retrieval_params ? JSON.parse(s.retrieval_params) : null; } catch { return null; } })() && (() => {
                                                                const params: any = JSON.parse(s.retrieval_params!);
                                                                const timings: any = (() => { try { return s.step_timings ? JSON.parse(s.step_timings) : {}; } catch { return {}; } })();
                                                                const chunks: any[] = (() => { try { return s.retrieval_chunks ? JSON.parse(s.retrieval_chunks) : []; } catch { return []; } })();
                                                                return <>
                                                                    <div className="text-[11px]">
                                                                        <span className="font-semibold text-muted-foreground">Search params:</span>
                                                                        <span className="ml-2 font-mono">α={params.alpha} threshold={params.threshold} hops={params.hop_limit} k={params.top_k}</span>
                                                                        {params.weights && <span className="ml-2 font-mono text-gray-500">w[v={params.weights.vector?.toFixed(2)} t={params.weights.tree?.toFixed(2)} g={params.weights.graph?.toFixed(2)}]</span>}
                                                                    </div>
                                                                    {Object.keys(timings).length > 0 && (
                                                                        <div className="text-[11px]">
                                                                            <span className="font-semibold text-muted-foreground">Timings:</span>
                                                                            <span className="ml-2 font-mono">retrieval={timings.retrieval}ms · generation={timings.generation}ms</span>
                                                                        </div>
                                                                    )}
                                                                    {chunks.length > 0 && (
                                                                        <details>
                                                                            <summary className="cursor-pointer text-muted-foreground hover:text-violet-600 text-[11px]">
                                                                                📚 Retrieved chunks ({chunks.length}) — click to expand
                                                                            </summary>
                                                                            <div className="mt-2 space-y-1">
                                                                                {chunks.map((c, ci) => (
                                                                                    <div key={ci} className="flex gap-2 items-start text-[10px] p-1.5 bg-white dark:bg-zinc-800/50 rounded">
                                                                                        <span className="px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 font-mono w-16 text-center flex-shrink-0">{c.source}</span>
                                                                                        <span className="font-mono text-violet-600 w-12 text-right flex-shrink-0">{(c.score ?? 0).toFixed(3)}</span>
                                                                                        <span className="font-medium w-24 truncate flex-shrink-0" title={c.title}>{c.title || "—"}</span>
                                                                                        <span className="text-muted-foreground flex-1 truncate" title={c.content_preview}>{c.content_preview}</span>
                                                                                    </div>
                                                                                ))}
                                                                            </div>
                                                                        </details>
                                                                    )}
                                                                </>;
                                                            })()}
                                                            {s.judge_reasoning && (
                                                                <div>
                                                                    <span className="font-semibold text-muted-foreground">Judge ({s.judge_model || "?"}):</span>
                                                                    <p className="mt-1 text-gray-700 dark:text-zinc-300 bg-white dark:bg-zinc-800/50 p-2 rounded whitespace-pre-wrap">{s.judge_reasoning}</p>
                                                                </div>
                                                            )}
                                                            <details>
                                                                <summary className="cursor-pointer text-muted-foreground hover:text-violet-600">📄 Full question / reference / actual answer</summary>
                                                                <div className="mt-2 space-y-2">
                                                                    <div><span className="font-semibold">Question:</span><p className="mt-1 bg-white dark:bg-zinc-800/50 p-2 rounded whitespace-pre-wrap">{s.question}</p></div>
                                                                    <div><span className="font-semibold">Reference:</span><p className="mt-1 bg-white dark:bg-zinc-800/50 p-2 rounded whitespace-pre-wrap">{s.expected_answer}</p></div>
                                                                    <div><span className="font-semibold">Actual:</span><p className="mt-1 bg-white dark:bg-zinc-800/50 p-2 rounded whitespace-pre-wrap">{s.actual_answer || "—"}</p></div>
                                                                </div>
                                                            </details>
                                                            <div className="flex gap-2 pt-1">
                                                                <Button size="sm" variant="outline" onClick={handleDiagnose} disabled={!!diagLoading[s.id]} className="text-violet-700 border-violet-300 hover:bg-violet-50">
                                                                    {diagLoading[s.id] ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : "🔬"} Diagnose with AI
                                                                </Button>
                                                                <Button size="sm" variant="outline" onClick={handleExplainRetrieval} disabled={!!diagLoading[-s.id]} className="text-blue-700 border-blue-300 hover:bg-blue-50">
                                                                    {diagLoading[-s.id] ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : "🔍"} Explain Retrieval
                                                                </Button>
                                                            </div>
                                                            {diagnoses[s.id]?.structured && (
                                                                <div className="bg-violet-100 dark:bg-violet-900/20 p-2 rounded">
                                                                    <div className="font-semibold text-violet-700 dark:text-violet-300 mb-1">🔬 AI Diagnosis</div>
                                                                    {diagnoses[s.id].structured.root_cause && <div><span className="text-muted-foreground">root_cause:</span> <code className="ml-1">{diagnoses[s.id].structured.root_cause}</code></div>}
                                                                    {diagnoses[s.id].structured.explanation && <div className="mt-1">{diagnoses[s.id].structured.explanation}</div>}
                                                                    {diagnoses[s.id].structured.fix && <div className="mt-1"><span className="text-muted-foreground">fix:</span> <code className="ml-1">{diagnoses[s.id].structured.fix.target}</code> → {diagnoses[s.id].structured.fix.action}</div>}
                                                                    {diagnoses[s.id].structured.confidence && <div className="mt-0.5 text-[10px] text-muted-foreground">confidence: {diagnoses[s.id].structured.confidence}</div>}
                                                                </div>
                                                            )}
                                                            {retrievalExplanations[s.id]?.structured && (
                                                                <div className="bg-blue-100 dark:bg-blue-900/20 p-2 rounded">
                                                                    <div className="font-semibold text-blue-700 dark:text-blue-300 mb-1">🔍 Retrieval Explanation</div>
                                                                    {retrievalExplanations[s.id].structured.verdict && <div><span className="text-muted-foreground">verdict:</span> <code className="ml-1">{retrievalExplanations[s.id].structured.verdict}</code></div>}
                                                                    {retrievalExplanations[s.id].structured.observation && <div className="mt-1">{retrievalExplanations[s.id].structured.observation}</div>}
                                                                    {retrievalExplanations[s.id].structured.missing && Array.isArray(retrievalExplanations[s.id].structured.missing) && (
                                                                        <div className="mt-1"><span className="text-muted-foreground">missing:</span> {retrievalExplanations[s.id].structured.missing.join(", ")}</div>
                                                                    )}
                                                                    {retrievalExplanations[s.id].structured.suggested_change && (
                                                                        <div className="mt-1"><span className="text-muted-foreground">suggest:</span> <code className="ml-1">{retrievalExplanations[s.id].structured.suggested_change.target}</code> → {retrievalExplanations[s.id].structured.suggested_change.value}</div>
                                                                    )}
                                                                </div>
                                                            )}
                                                        </div>
                                                    </TableCell>
                                                </TableRow>
                                            )}
                                            </>);
                                        })}
                                    </TableBody>
                                </Table>
                            )}
                        </CardContent>
                    </Card>
                )}

                {!loading && runs.length === 0 && (
                    <Card>
                        <CardContent className="text-center py-16">
                            <Target className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
                            <h3 className="text-lg font-semibold mb-2">No Evaluation Runs</h3>
                            <p className="text-muted-foreground mb-4">
                                Run <code className="bg-muted px-2 py-1 rounded text-xs">cargo run --bin run_eval</code> to start your first evaluation.
                            </p>
                        </CardContent>
                    </Card>
                )}
            </>)}

            {/* Model Performance Tab */}
            {activeTab === "performance" && (
                <div className="space-y-6">
                    {/* A/B Comparison */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <GitCompare className="w-5 h-5" /> A/B Model Comparison
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="flex items-end gap-4">
                                <div className="flex-1">
                                    <label className="text-xs text-muted-foreground">Model A</label>
                                    <select value={modelA} onChange={e => setModelA(e.target.value)}
                                        className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm">
                                        <option value="">Select model...</option>
                                        {allModels.map(m => <option key={m} value={m}>{m}</option>)}
                                    </select>
                                </div>
                                <span className="text-gray-400 font-bold pb-2">vs</span>
                                <div className="flex-1">
                                    <label className="text-xs text-muted-foreground">Model B</label>
                                    <select value={modelB} onChange={e => setModelB(e.target.value)}
                                        className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm">
                                        <option value="">Select model...</option>
                                        {allModels.map(m => <option key={m} value={m}>{m}</option>)}
                                    </select>
                                </div>
                                <Button onClick={handleCompare} disabled={!modelA || !modelB || loadingComparison}>
                                    {loadingComparison ? <RefreshCw className="w-4 h-4 animate-spin" /> : "Compare"}
                                </Button>
                            </div>

                            {comparison && (
                                <div className="grid grid-cols-2 gap-4 mt-4">
                                    {["model_a", "model_b"].map(key => {
                                        const data = comparison[key];
                                        if (!data) return null;
                                        return (
                                            <Card key={key} className={key === "model_a" ? "border-blue-200 dark:border-blue-800" : "border-purple-200 dark:border-purple-800"}>
                                                <CardHeader className="pb-2">
                                                    <CardTitle className="text-sm">{data.model_id || key}</CardTitle>
                                                </CardHeader>
                                                <CardContent className="space-y-2 text-sm">
                                                    <div className="flex justify-between"><span className="text-muted-foreground">Avg Accuracy</span><span className="font-medium">{data.avg_accuracy?.toFixed(2) ?? "-"}</span></div>
                                                    <div className="flex justify-between"><span className="text-muted-foreground">Avg Completeness</span><span className="font-medium">{data.avg_completeness?.toFixed(2) ?? "-"}</span></div>
                                                    <div className="flex justify-between"><span className="text-muted-foreground">Avg Relevance</span><span className="font-medium">{data.avg_relevance?.toFixed(2) ?? "-"}</span></div>
                                                    <div className="flex justify-between"><span className="text-muted-foreground">Avg Latency</span><span className="font-medium">{formatLatency(data.avg_latency_ms)}</span></div>
                                                    <div className="flex justify-between"><span className="text-muted-foreground">Total Questions</span><span className="font-medium">{data.total_questions ?? "-"}</span></div>
                                                </CardContent>
                                            </Card>
                                        );
                                    })}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* Feedback Summary */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <BarChart3 className="w-5 h-5" /> User Feedback Summary
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            {feedbackSummary ? (
                                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                                    <div className="text-center p-4 bg-green-50 dark:bg-green-900/20 rounded-lg">
                                        <ThumbsUp className="w-6 h-6 text-green-500 mx-auto mb-1" />
                                        <p className="text-2xl font-bold text-green-600">{feedbackSummary.thumbs_up ?? 0}</p>
                                        <p className="text-xs text-gray-500">Positive</p>
                                    </div>
                                    <div className="text-center p-4 bg-red-50 dark:bg-red-900/20 rounded-lg">
                                        <ThumbsDown className="w-6 h-6 text-red-500 mx-auto mb-1" />
                                        <p className="text-2xl font-bold text-red-600">{feedbackSummary.thumbs_down ?? 0}</p>
                                        <p className="text-xs text-gray-500">Negative</p>
                                    </div>
                                    <div className="text-center p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                                        <p className="text-2xl font-bold text-blue-600">{feedbackSummary.total_reviewed ?? 0}</p>
                                        <p className="text-xs text-gray-500">Total Reviewed</p>
                                    </div>
                                    <div className="text-center p-4 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
                                        <p className="text-2xl font-bold text-amber-600">
                                            {feedbackSummary.satisfaction_rate ? `${(feedbackSummary.satisfaction_rate * 100).toFixed(0)}%` : "-"}
                                        </p>
                                        <p className="text-xs text-gray-500">Satisfaction Rate</p>
                                    </div>
                                </div>
                            ) : (
                                <p className="text-center text-muted-foreground py-8">No feedback data available</p>
                            )}
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* Extraction Evaluation Tab */}
            {activeTab === "extraction" && (
                <div className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Beaker className="w-5 h-5" /> Extraction Performance — Provider × Model
                            </CardTitle>
                            <p className="text-sm text-muted-foreground">KG entities, QA pairs, and latency by provider and model combination</p>
                        </CardHeader>
                        <CardContent>
                            {loadingExtraction ? (
                                <div className="text-center py-8 text-muted-foreground">Loading extraction data...</div>
                            ) : !extractionData ? (
                                <div className="text-center py-8 text-muted-foreground">No extraction data available. Use POST /sources/:id/extract to generate data.</div>
                            ) : (
                                <div className="space-y-6">
                                    {/* QA Stats Table */}
                                    <div>
                                        <h3 className="text-sm font-semibold mb-3">QA Pairs Generated</h3>
                                        <Table>
                                            <TableHeader>
                                                <TableRow>
                                                    <TableHead>Provider</TableHead>
                                                    <TableHead>Model</TableHead>
                                                    <TableHead>Prompt Version</TableHead>
                                                    <TableHead className="text-right">QA Count</TableHead>
                                                    <TableHead className="text-right">Avg Latency</TableHead>
                                                </TableRow>
                                            </TableHeader>
                                            <TableBody>
                                                {(extractionData.qa_stats || []).map((s: any, i: number) => (
                                                    <TableRow key={i}>
                                                        <TableCell>
                                                            <span className="px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium">
                                                                {s.provider}
                                                            </span>
                                                        </TableCell>
                                                        <TableCell className="font-mono text-xs">{s.model}</TableCell>
                                                        <TableCell>
                                                            <span className="px-2 py-0.5 bg-zinc-100 dark:bg-zinc-800 rounded text-xs">{s.prompt_version}</span>
                                                        </TableCell>
                                                        <TableCell className="text-right font-bold">{s.qa_count}</TableCell>
                                                        <TableCell className="text-right text-muted-foreground">{formatLatency(s.avg_latency_ms)}</TableCell>
                                                    </TableRow>
                                                ))}
                                            </TableBody>
                                        </Table>
                                    </div>

                                    {/* KG Stats Table */}
                                    <div>
                                        <h3 className="text-sm font-semibold mb-3">KG Entities Extracted</h3>
                                        <Table>
                                            <TableHeader>
                                                <TableRow>
                                                    <TableHead>Provider</TableHead>
                                                    <TableHead>Model</TableHead>
                                                    <TableHead>Prompt Version</TableHead>
                                                    <TableHead className="text-right">Entity Count</TableHead>
                                                    <TableHead className="text-right">Avg Latency</TableHead>
                                                </TableRow>
                                            </TableHeader>
                                            <TableBody>
                                                {(extractionData.kg_stats || []).map((s: any, i: number) => (
                                                    <TableRow key={i}>
                                                        <TableCell>
                                                            <span className="px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-xs font-medium">
                                                                {s.provider}
                                                            </span>
                                                        </TableCell>
                                                        <TableCell className="font-mono text-xs">{s.model}</TableCell>
                                                        <TableCell>
                                                            <span className="px-2 py-0.5 bg-zinc-100 dark:bg-zinc-800 rounded text-xs">{s.prompt_version}</span>
                                                        </TableCell>
                                                        <TableCell className="text-right font-bold">{s.entity_count}</TableCell>
                                                        <TableCell className="text-right text-muted-foreground">{formatLatency(s.avg_latency_ms)}</TableCell>
                                                    </TableRow>
                                                ))}
                                            </TableBody>
                                        </Table>
                                    </div>

                                    {/* Relation Stats */}
                                    <div>
                                        <h3 className="text-sm font-semibold mb-3">KG Relations</h3>
                                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                                            {(extractionData.relation_stats || []).map((s: any, i: number) => (
                                                <div key={i} className="text-center p-4 bg-zinc-50 dark:bg-zinc-900 rounded-lg">
                                                    <p className="text-2xl font-bold">{s.relation_count}</p>
                                                    <p className="text-xs text-muted-foreground">{s.provider} / {s.model}</p>
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* Retrieval Evaluation Tab */}
            {activeTab === "retrieval" && (
                <div className="space-y-6">
                    {/* Qdrant Collections */}
                    <div className="grid gap-4 md:grid-cols-2">
                        {(retrievalData?.qdrant_collections || []).map((c: any) => (
                            <Card key={c.name}>
                                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                    <CardTitle className="text-sm font-medium">{c.name}</CardTitle>
                                    <Database className="h-4 w-4 text-muted-foreground" />
                                </CardHeader>
                                <CardContent>
                                    <div className="text-2xl font-bold">{c.points_count.toLocaleString()}</div>
                                    <p className="text-xs text-muted-foreground">vectors indexed</p>
                                </CardContent>
                            </Card>
                        ))}
                    </div>

                    {/* Source Pipeline Coverage */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Database className="w-5 h-5" /> Source Pipeline Coverage
                            </CardTitle>
                            <p className="text-sm text-muted-foreground">Per-source status: chunks, QA pairs, and KG entities</p>
                        </CardHeader>
                        <CardContent>
                            {loadingRetrieval ? (
                                <div className="text-center py-8 text-muted-foreground">Loading retrieval data...</div>
                            ) : !retrievalData?.sources?.length ? (
                                <div className="text-center py-8 text-muted-foreground">No sources found</div>
                            ) : (
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <TableHead>Source</TableHead>
                                            <TableHead className="text-right">Chunks</TableHead>
                                            <TableHead className="text-right">QA Pairs</TableHead>
                                            <TableHead className="text-right">KG Entities</TableHead>
                                            <TableHead className="text-center">Pipeline</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {retrievalData.sources.map((s: any) => {
                                            const hasChunks = s.chunk_count > 0;
                                            const hasQA = s.qa_count > 0;
                                            const hasKG = s.entity_count > 0;
                                            const steps = [hasChunks, hasQA, hasKG].filter(Boolean).length;
                                            return (
                                                <TableRow key={s.source_id}>
                                                    <TableCell className="font-medium">{s.name}</TableCell>
                                                    <TableCell className="text-right">
                                                        <span className={hasChunks ? "text-emerald-500 font-bold" : "text-muted-foreground"}>
                                                            {s.chunk_count}
                                                        </span>
                                                    </TableCell>
                                                    <TableCell className="text-right">
                                                        <span className={hasQA ? "text-blue-500 font-bold" : "text-muted-foreground"}>
                                                            {s.qa_count}
                                                        </span>
                                                    </TableCell>
                                                    <TableCell className="text-right">
                                                        <span className={hasKG ? "text-purple-500 font-bold" : "text-muted-foreground"}>
                                                            {s.entity_count}
                                                        </span>
                                                    </TableCell>
                                                    <TableCell className="text-center">
                                                        <div className="flex justify-center gap-1">
                                                            <span className={`w-2 h-2 rounded-full ${hasChunks ? "bg-emerald-500" : "bg-zinc-300 dark:bg-zinc-700"}`} title="Chunks" />
                                                            <span className={`w-2 h-2 rounded-full ${hasQA ? "bg-blue-500" : "bg-zinc-300 dark:bg-zinc-700"}`} title="QA" />
                                                            <span className={`w-2 h-2 rounded-full ${hasKG ? "bg-purple-500" : "bg-zinc-300 dark:bg-zinc-700"}`} title="KG" />
                                                        </div>
                                                        <span className="text-[10px] text-muted-foreground">{steps}/3</span>
                                                    </TableCell>
                                                </TableRow>
                                            );
                                        })}
                                    </TableBody>
                                </Table>
                            )}
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* Pipeline Scorecard Tab */}
            {activeTab === "pipeline" && (
                <div className="space-y-6">
                    {/* Summary Cards */}
                    {pipelineData && (
                        <div className="grid gap-4 md:grid-cols-3">
                            <Card>
                                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                    <CardTitle className="text-sm font-medium">Total Sources</CardTitle>
                                    <Database className="h-4 w-4 text-muted-foreground" />
                                </CardHeader>
                                <CardContent>
                                    <div className="text-2xl font-bold">{pipelineData.total_sources}</div>
                                </CardContent>
                            </Card>
                            <Card>
                                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                    <CardTitle className="text-sm font-medium">Fully Complete</CardTitle>
                                    <CheckCircle className="h-4 w-4 text-emerald-500" />
                                </CardHeader>
                                <CardContent>
                                    <div className="text-2xl font-bold text-emerald-500">{pipelineData.fully_complete}</div>
                                </CardContent>
                            </Card>
                            <Card>
                                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                    <CardTitle className="text-sm font-medium">Completion Rate</CardTitle>
                                    <BarChart3 className="h-4 w-4 text-muted-foreground" />
                                </CardHeader>
                                <CardContent>
                                    <div className="text-2xl font-bold">{pipelineData.completion_rate}%</div>
                                    <div className="w-full bg-zinc-200 dark:bg-zinc-800 rounded-full h-2 mt-2">
                                        <div className="bg-emerald-500 h-2 rounded-full transition-all" style={{ width: `${pipelineData.completion_rate}%` }} />
                                    </div>
                                </CardContent>
                            </Card>
                        </div>
                    )}

                    {/* Scorecard Table */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Workflow className="w-5 h-5" /> Pipeline Scorecard
                            </CardTitle>
                            <p className="text-sm text-muted-foreground">Per-source pipeline step completion: Chunks → Embed → KG → QA → Index</p>
                        </CardHeader>
                        <CardContent>
                            {loadingPipeline ? (
                                <div className="text-center py-8 text-muted-foreground">Loading pipeline data...</div>
                            ) : !pipelineData?.sources?.length ? (
                                <div className="text-center py-8 text-muted-foreground">No sources found</div>
                            ) : (
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <TableHead>Source</TableHead>
                                            <TableHead className="text-center">Chunks</TableHead>
                                            <TableHead className="text-center">Embed</TableHead>
                                            <TableHead className="text-center">KG</TableHead>
                                            <TableHead className="text-center">QA</TableHead>
                                            <TableHead className="text-center">Index</TableHead>
                                            <TableHead className="text-center">Progress</TableHead>
                                            <TableHead>Last Run</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {pipelineData.sources.map((s: any) => (
                                            <TableRow key={s.source_id}>
                                                <TableCell>
                                                    <div className="font-medium">{s.name}</div>
                                                    <div className="text-xs text-muted-foreground">{s.source_type}</div>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <div className={`inline-flex items-center gap-1 ${s.steps.chunks.done ? "text-emerald-500" : "text-zinc-400"}`}>
                                                        <span className={`w-2.5 h-2.5 rounded-full ${s.steps.chunks.done ? "bg-emerald-500" : "bg-zinc-300 dark:bg-zinc-700"}`} />
                                                        <span className="text-xs">{s.steps.chunks.count}</span>
                                                    </div>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <span className={`w-2.5 h-2.5 rounded-full inline-block ${s.steps.embedded.done ? "bg-blue-500" : "bg-zinc-300 dark:bg-zinc-700"}`} />
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <div className={`inline-flex items-center gap-1 ${s.steps.kg_entities.done ? "text-purple-500" : "text-zinc-400"}`}>
                                                        <span className={`w-2.5 h-2.5 rounded-full ${s.steps.kg_entities.done ? "bg-purple-500" : "bg-zinc-300 dark:bg-zinc-700"}`} />
                                                        <span className="text-xs">{s.steps.kg_entities.count}</span>
                                                    </div>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <div className={`inline-flex items-center gap-1 ${s.steps.qa_pairs.done ? "text-amber-500" : "text-zinc-400"}`}>
                                                        <span className={`w-2.5 h-2.5 rounded-full ${s.steps.qa_pairs.done ? "bg-amber-500" : "bg-zinc-300 dark:bg-zinc-700"}`} />
                                                        <span className="text-xs">{s.steps.qa_pairs.count}</span>
                                                    </div>
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <span className={`w-2.5 h-2.5 rounded-full inline-block ${s.steps.qa_indexed.done ? "bg-teal-500" : "bg-zinc-300 dark:bg-zinc-700"}`} />
                                                </TableCell>
                                                <TableCell className="text-center">
                                                    <div className="flex flex-col items-center gap-1">
                                                        <div className="w-16 bg-zinc-200 dark:bg-zinc-800 rounded-full h-1.5">
                                                            <div className={`h-1.5 rounded-full transition-all ${s.completion_pct === 100 ? "bg-emerald-500" : s.completion_pct >= 60 ? "bg-amber-500" : "bg-red-400"}`} style={{ width: `${s.completion_pct}%` }} />
                                                        </div>
                                                        <span className="text-[10px] text-muted-foreground">{s.completion}</span>
                                                    </div>
                                                </TableCell>
                                                <TableCell>
                                                    {s.latest_run ? (
                                                        <div className="text-xs">
                                                            <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${s.latest_run.status === "completed" ? "bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300" : s.latest_run.status === "running" ? "bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300" : "bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300"}`}>
                                                                {s.latest_run.status}
                                                            </span>
                                                            <div className="text-muted-foreground mt-0.5">{s.latest_run.provider}/{s.latest_run.model}</div>
                                                        </div>
                                                    ) : (
                                                        <span className="text-xs text-muted-foreground">—</span>
                                                    )}
                                                </TableCell>
                                            </TableRow>
                                        ))}
                                    </TableBody>
                                </Table>
                            )}
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* ─── AI Analysis (Wave 2) ─────────────────────────────────────── */}
            {activeTab === "ai-analysis" && <AiAnalysisTab runId={selectedRunId} runName={selectedRun?.name || ""} />}
        </div>
    );
}

// ═══ Cross-Benchmark Composite Leaderboard ═════════════════════════════════
//
// Aggregates eval runs by model_id across multiple benchmarks and computes a
// single 0-100 "Composite" score using min-max normalization per benchmark.
// Methodology:
//   1. For each benchmark, find the cohort's min and max raw score.
//   2. Normalize each model's score in that benchmark to 0-100.
//   3. Composite = average of the model's normalized scores across benchmarks.
//
// 0 = worst in current cohort, 100 = best. Re-normalizes when cohort changes
// (this is by design — relative ranking).
//
// Coverage column shows how many of the loaded benchmarks the model was tested
// on. Models with <3 benchmarks shown grayed out (composite less reliable).
function CompositeLeaderboard(props: {
    runs: EvalRun[];
    summaries: Record<string, any>;
    benchmarks: BenchmarkDatasetLite[];
}) {
    const { runs, summaries, benchmarks } = props;

    // Helper: rubric-aware metric per scoring_fn (mirrors RunsTable logic)
    const norm5 = (x?: number) => (x == null || x <= 0) ? null : Math.max(0, (x - 1) / 4);
    const computeMetric = (scoringFn: string | undefined, s: any): number | undefined => {
        if (!s) return undefined;
        const acc = s.avg_accuracy, comp = s.avg_completeness, rel = s.avg_relevance, safe = s.avg_safety_score;
        switch (scoringFn) {
            case "mcq_accuracy":
            case "binary_yes_no": {
                const a = acc;
                return (a != null && a >= 0 && a <= 5) ? norm5(a)! * 100 : undefined;
            }
            case "paper_rubric_pct": {
                const a = acc;
                return (a != null && a >= 0 && a <= 5) ? norm5(a)! * 100 : undefined;
            }
            case "healthbench_likert":
            default: {
                const a = norm5(acc), c = norm5(comp), r = norm5(rel);
                const sn = (safe == null) ? null : Math.max(0, safe);
                const parts = [a, c, r, sn].filter(v => v != null) as number[];
                if (parts.length === 0) return undefined;
                return (parts.reduce((sum, v) => sum + v, 0) / parts.length) * 100;
            }
        }
    };

    const benchmarkLookup = new Map(benchmarks.map(b => [b.id, b]));

    // Sample-size divisions — n=5 has ±10pp noise, n=100 has ±2pp.
    // Mixing them into one composite is misleading. Stratify by n bucket so
    // each leaderboard only compares runs of similar statistical power.
    const sampleDivision = (n: number): string => {
        if (n < 10) return "quick (n<10)";
        if (n < 50) return "standard (n=10-49)";
        if (n < 200) return "robust (n=50-199)";
        return "high-confidence (n≥200)";
    };

    // Group COMPLETED runs by (division, model_id, benchmark_id) — keep latest per combo
    type Entry = { metric: number; run_id: string; cost?: number; lat_ms?: number; n: number };
    type DivisionData = { byModel: Map<string, Map<string, Entry>>; sampleSizes: Set<number> };
    const byDivision: Map<string, DivisionData> = new Map();

    for (const r of runs) {
        if (r.status !== "COMPLETED") continue;
        const s = summaries[r.id]?.summaries?.[0];
        if (!s) continue;
        let cfg: any = r.config;
        try { if (typeof cfg === "string") cfg = JSON.parse(cfg); } catch { cfg = {}; }
        const benchmark_id = cfg?.benchmark_dataset_id;
        const model_id = s.model_id;
        const n = r.total_combinations || s.total_questions || 0;
        if (!benchmark_id || !model_id || n <= 0) continue;
        const ds = benchmarkLookup.get(benchmark_id);
        const metric = computeMetric(ds?.scoring_fn, s);
        if (metric == null) continue;

        const div = sampleDivision(n);
        if (!byDivision.has(div)) byDivision.set(div, { byModel: new Map(), sampleSizes: new Set() });
        const dd = byDivision.get(div)!;
        dd.sampleSizes.add(n);
        if (!dd.byModel.has(model_id)) dd.byModel.set(model_id, new Map());
        const entry: Entry = { metric, run_id: r.id, cost: r.total_cost_usd ?? undefined, lat_ms: s.avg_latency_ms, n };
        const existing = dd.byModel.get(model_id)!.get(benchmark_id);
        if (!existing || (r.started_at > (runs.find(x => x.id === existing.run_id)?.started_at || ""))) {
            dd.byModel.get(model_id)!.set(benchmark_id, entry);
        }
    }
    if (byDivision.size === 0) return null;

    // Order divisions by sample size (largest n = most reliable first)
    const divOrder = ["high-confidence (n≥200)", "robust (n=50-199)", "standard (n=10-49)", "quick (n<10)"];
    const sortedDivs = divOrder.filter(d => byDivision.has(d));

    // Compute leaderboard rows per division
    type LbRow = { model_id: string; composite: number; coverage: number; raw: Map<string, Entry>; total_cost: number; avg_lat: number };
    type DivisionLB = { name: string; rows: LbRow[]; benchCount: number; sampleSizes: number[] };
    const allDivisions: DivisionLB[] = [];

    for (const divName of sortedDivs) {
        const dd = byDivision.get(divName)!;
        const byModel = dd.byModel;
        // Per-benchmark cohort min/max within THIS division
        const benchStats = new Map<string, { min: number; max: number }>();
        for (const benchId of new Set([...byModel.values()].flatMap(m => [...m.keys()]))) {
            const vals: number[] = [];
            for (const modelEntries of byModel.values()) {
                const e = modelEntries.get(benchId);
                if (e) vals.push(e.metric);
            }
            if (vals.length > 0) {
                benchStats.set(benchId, { min: Math.min(...vals), max: Math.max(...vals) });
            }
        }

        const lbRows: LbRow[] = [];
        for (const [model_id, entries] of byModel) {
            const normScores: number[] = [];
            let total_cost = 0, lat_sum = 0, lat_n = 0;
            for (const [benchId, e] of entries) {
                const stats = benchStats.get(benchId);
                if (!stats) continue;
                const range = stats.max - stats.min;
                const normalized = range > 0 ? ((e.metric - stats.min) / range) * 100 : 50;
                normScores.push(normalized);
                if (e.cost) total_cost += e.cost;
                if (e.lat_ms) { lat_sum += e.lat_ms; lat_n++; }
            }
            if (normScores.length === 0) continue;
            lbRows.push({
                model_id,
                composite: normScores.reduce((s, v) => s + v, 0) / normScores.length,
                coverage: entries.size,
                raw: entries,
                total_cost,
                avg_lat: lat_n > 0 ? lat_sum / lat_n : 0,
            });
        }
        lbRows.sort((a, b) => b.composite - a.composite);
        if (lbRows.length > 0) {
            allDivisions.push({
                name: divName,
                rows: lbRows,
                benchCount: benchStats.size,
                sampleSizes: [...dd.sampleSizes].sort((a, b) => a - b),
            });
        }
    }
    if (allDivisions.length === 0) return null;

    const divEmoji = (name: string) => {
        if (name.startsWith("high")) return "🏅";
        if (name.startsWith("robust")) return "🎯";
        if (name.startsWith("standard")) return "📊";
        return "⚡";
    };
    const divDesc = (name: string) => {
        if (name.startsWith("high")) return "Highest statistical confidence. ±2pp noise. Use for production champion claims.";
        if (name.startsWith("robust")) return "Solid signal. ±3pp noise. Decision-grade.";
        if (name.startsWith("standard")) return "Initial validation. ±5pp noise. Directional only.";
        return "Smoke-test only. ±10pp noise. Cannot rank reliably.";
    };

    return (
        <details className="rounded-lg border border-violet-200 dark:border-violet-900/50 bg-violet-50/40 dark:bg-violet-900/10 mb-4" open>
            <summary className="cursor-pointer px-4 py-3 font-semibold text-violet-900 dark:text-violet-200 flex items-center gap-2 hover:bg-violet-100/40 dark:hover:bg-violet-900/20 select-none">
                🏆 Cross-Benchmark Leaderboard
                <span className="text-xs font-normal text-muted-foreground">
                    {allDivisions.length} division{allDivisions.length !== 1 ? "s" : ""} · normalized 0-100 within each division
                </span>
            </summary>
            <div className="px-4 py-3 border-t border-violet-200 dark:border-violet-900/50 space-y-5">

                {allDivisions.map((div) => (
                    <div key={div.name}>
                        <div className="flex items-baseline gap-2 mb-2">
                            <h3 className="font-semibold text-sm text-violet-900 dark:text-violet-200">
                                {divEmoji(div.name)} {div.name}
                            </h3>
                            <span className="text-[11px] text-muted-foreground">
                                {div.rows.length} model{div.rows.length !== 1 ? "s" : ""} · {div.benchCount} benchmark{div.benchCount !== 1 ? "s" : ""} · n={div.sampleSizes.join(",")}
                            </span>
                            <span className="text-[11px] text-muted-foreground italic ml-auto" title={divDesc(div.name)}>
                                {divDesc(div.name)}
                            </span>
                        </div>
                        <div className="overflow-x-auto rounded border border-violet-100 dark:border-violet-900/40">
                            <table className="w-full text-xs">
                                <thead>
                                    <tr className="text-left text-muted-foreground bg-violet-50/60 dark:bg-violet-900/20 border-b border-violet-100 dark:border-violet-900/30">
                                        <th className="px-2 py-2 w-12 text-center">#</th>
                                        <th className="px-2 py-2">Model</th>
                                        <th className="px-2 py-2 w-24 text-right" title="Min-max normalized 0-100 averaged across all benchmarks. 0 = worst in this division, 100 = best.">Composite</th>
                                        <th className="px-2 py-2 w-24 text-right">Coverage</th>
                                        <th className="px-2 py-2 w-20 text-right">Avg Lat</th>
                                        <th className="px-2 py-2 w-20 text-right">Cost</th>
                                        <th className="px-2 py-2 text-left">Per-benchmark</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {div.rows.map((row, i) => {
                                        const incomplete = row.coverage < 3;
                                        return (
                                            <tr key={row.model_id} className={`border-b border-violet-100/50 dark:border-violet-900/20 ${incomplete ? "opacity-60" : ""}`}>
                                                <td className="px-2 py-2 text-center font-semibold">
                                                    {i === 0 ? "🥇" : i === 1 ? "🥈" : i === 2 ? "🥉" : i + 1}
                                                </td>
                                                <td className="px-2 py-2 font-mono">
                                                    {row.model_id.split("/").pop()}
                                                </td>
                                                <td className="px-2 py-2 text-right">
                                                    <span className={`px-2 py-0.5 rounded font-bold tabular-nums ${
                                                        row.composite >= 75 ? "bg-emerald-200 text-emerald-900 dark:bg-emerald-900/40 dark:text-emerald-200" :
                                                        row.composite >= 50 ? "bg-amber-200 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200" :
                                                        "bg-red-200 text-red-900 dark:bg-red-900/40 dark:text-red-200"
                                                    }`}>
                                                        {row.composite.toFixed(1)}
                                                    </span>
                                                </td>
                                                <td className="px-2 py-2 text-right tabular-nums">
                                                    {row.coverage}/{div.benchCount} {incomplete && <span className="text-amber-600 dark:text-amber-500" title="Composite less reliable below 3 benchmarks">⚠</span>}
                                                </td>
                                                <td className="px-2 py-2 text-right tabular-nums text-muted-foreground">
                                                    {row.avg_lat > 0 ? (row.avg_lat / 1000).toFixed(1) + "s" : "—"}
                                                </td>
                                                <td className="px-2 py-2 text-right tabular-nums text-muted-foreground">
                                                    {row.total_cost > 0 ? "$" + row.total_cost.toFixed(4) : "$0"}
                                                </td>
                                                <td className="px-2 py-2 text-[10px] text-muted-foreground">
                                                    {[...row.raw.entries()].map(([bid, e]) => {
                                                        const ds = benchmarkLookup.get(bid);
                                                        const label = ds?.name?.split("(")[0]?.trim() || bid;
                                                        return <span key={bid} className="inline-block mr-2" title={`${bid} · n=${e.n}`}>
                                                            {label.slice(0, 12)}: <span className="font-mono">{e.metric.toFixed(0)}%</span>
                                                        </span>;
                                                    })}
                                                </td>
                                            </tr>
                                        );
                                    })}
                                </tbody>
                            </table>
                        </div>
                    </div>
                ))}

                <div className="text-[11px] text-muted-foreground space-y-1 pt-2 border-t border-violet-200 dark:border-violet-900/50">
                    <div>📐 <strong>Why divisions?</strong> n=20 results have ±5pp noise; n=100 results have ±2pp. Cannot fairly compare across sample sizes — a model that lost at n=5 may dominate at n=100. Each division ranks only equal-power runs.</div>
                    <div>📊 <strong>Composite formula:</strong> per benchmark, normalize raw score to 0-100 (worst=0, best=100 in this division), then average across benchmarks.</div>
                    <div>⚠️ <strong>Coverage &lt; 3:</strong> composite less reliable — model tested on too few benchmarks within this division.</div>
                    <div>🔄 <strong>Re-normalizes</strong> when new runs land. Relative ranking stable; absolute number is cohort-relative.</div>
                </div>
            </div>
        </details>
    );
}

// ═══ Phase 1: Runs Table — primary view of all eval runs ═══════════════════════

function RunsTable(props: {
    runs: EvalRun[];
    summaries: Record<string, any>;
    benchmarks: BenchmarkDatasetLite[];
    sortBy: "score" | "cost" | "latency" | "started" | "name";
    setSortBy: (v: any) => void;
    sortDir: "asc" | "desc";
    setSortDir: (v: any) => void;
    statusFilter: string;
    setStatusFilter: (v: string) => void;
    benchmarkFilter: string;
    setBenchmarkFilter: (v: string) => void;
    selectedForCompare: Set<string>;
    setSelectedForCompare: (v: Set<string>) => void;
    expandedRunRow: string | null;
    setExpandedRunRow: (v: string | null) => void;
    onOpenRun: (id: string) => void;
    onAnalyze: (id: string) => void;
    onPromote: (id: string) => Promise<void>;
    onCompare: () => void;
}) {
    const { runs, summaries, benchmarks, sortBy, setSortBy, sortDir, setSortDir,
            statusFilter, setStatusFilter, benchmarkFilter, setBenchmarkFilter,
            selectedForCompare, setSelectedForCompare,
            expandedRunRow, setExpandedRunRow, onOpenRun, onAnalyze, onPromote,
            onCompare } = props;

    // Sprint 40 B-36/B-36d: lookup table for benchmark id → metadata.
    // Used to surface name, scoring_fn (for the metric column), and to
    // compute per-benchmark Rank/Champion (B-36c).
    const benchmarkLookup = new Map(benchmarks.map(b => [b.id, b]));
    const parseBenchmarkId = (configStr: string | null | undefined): string | undefined => {
        if (!configStr) return undefined;
        try {
            const cfg = typeof configStr === "string" ? JSON.parse(configStr) : configStr;
            return cfg?.benchmark_dataset_id;
        } catch { return undefined; }
    };

    // Build flat rows joining run + first summary
    // hbp_pct = HealthBench-Pro normalized 0-100 score, computed by normalizing
    // each Likert dimension (1-5 → 0-1) and averaging with safety (already 0-1).
    // This matches the canonical baseline at docs/04_03 so users can compare
    // dashboard numbers directly to the doc table.
    type Row = {
        run: EvalRun;
        agent_name?: string;
        model_id?: string;
        n?: number;
        acc?: number; comp?: number; rel?: number; safety?: number; unsafe?: number;
        latency_ms?: number; overall?: number; cost_usd?: number;
        hbp_pct?: number;
        // Sprint 40 — multi-benchmark fields
        benchmark_id?: string;
        benchmark_name?: string;
        scoring_fn?: string;
        // Single 0-100 score for THIS row, computed per the dataset's scoring_fn.
        // - healthbench_likert: same as hbp_pct (Likert + safety normalized)
        // - mcq_accuracy / binary_yes_no: read from summary (avg_accuracy is fraction 0-1) × 100
        // - paper_rubric_pct: read from overall_score directly (already %)
        metric_pct?: number;
    };
    const norm5 = (x?: number) => (x == null || x <= 0) ? null : Math.max(0, (x - 1) / 4);
    const computeHbp = (acc?: number, comp?: number, rel?: number, safe?: number) => {
        const a = norm5(acc), c = norm5(comp), r = norm5(rel);
        const s = (safe == null) ? null : safe;
        if (a == null && c == null && r == null && s == null) return undefined;
        const parts = [a, c, r, s].filter(v => v != null) as number[];
        if (parts.length === 0) return undefined;
        return (parts.reduce((sum, v) => sum + v, 0) / parts.length) * 100;
    };
    // Sprint 40 B-36d: rubric-aware metric per dataset. Returns 0-100 or undefined.
    const computeMetric = (scoringFn: string | undefined, s: any, hbp: number | undefined): number | undefined => {
        if (!s) return undefined;
        switch (scoringFn) {
            case "mcq_accuracy":
            case "binary_yes_no": {
                // Summary stores accuracy fraction 0-1 in `avg_accuracy` for MCQ runs.
                const a = s.avg_accuracy;
                return (a != null && a >= 0 && a <= 1) ? a * 100 : undefined;
            }
            case "paper_rubric_pct": {
                // overall_score is already 0-1 fraction of rubric points met.
                const o = s.overall_score;
                return (o != null && o >= 0 && o <= 1) ? o * 100 : undefined;
            }
            case "healthbench_likert":
            default:
                return hbp;
        }
    };
    const allRows: Row[] = runs.map(r => {
        const s = summaries[r.id]?.summaries?.[0];
        const benchmark_id = parseBenchmarkId(r.config);
        const ds = benchmark_id ? benchmarkLookup.get(benchmark_id) : undefined;
        const hbp = computeHbp(s?.avg_accuracy, s?.avg_completeness,
                               s?.avg_relevance, s?.avg_safety_score);
        const metric_pct = computeMetric(ds?.scoring_fn, s, hbp);
        return {
            run: r,
            agent_name: s?.agent_name,
            model_id: s?.model_id,
            n: s?.total_questions,
            acc: s?.avg_accuracy,
            comp: s?.avg_completeness,
            rel: s?.avg_relevance,
            safety: s?.avg_safety_score,
            unsafe: s?.unsafe_count,
            latency_ms: s?.avg_latency_ms,
            overall: s?.overall_score,
            cost_usd: r.total_cost_usd ?? undefined,
            hbp_pct: hbp,
            benchmark_id, benchmark_name: ds?.name, scoring_fn: ds?.scoring_fn,
            metric_pct,
        };
    });

    const filtered = allRows.filter(row =>
        (statusFilter === "ALL" || row.run.status === statusFilter) &&
        (benchmarkFilter === "ALL" || row.benchmark_id === benchmarkFilter)
    );

    const sorted = [...filtered].sort((a, b) => {
        const sign = sortDir === "asc" ? 1 : -1;
        // sortBy="score" keys on metric_pct — rubric-aware, comparable WITHIN a benchmark.
        // When filter shows "ALL" benchmarks the column header reads "Score%" so
        // user knows the numbers may have different rubrics behind them.
        const av: any = sortBy === "score" ? (a.metric_pct ?? -Infinity)
            : sortBy === "cost" ? (a.cost_usd ?? -Infinity)
            : sortBy === "latency" ? (a.latency_ms ?? Infinity)
            : sortBy === "name" ? (a.run.name || "")
            : new Date(a.run.started_at || 0).getTime();
        const bv: any = sortBy === "score" ? (b.metric_pct ?? -Infinity)
            : sortBy === "cost" ? (b.cost_usd ?? -Infinity)
            : sortBy === "latency" ? (b.latency_ms ?? Infinity)
            : sortBy === "name" ? (b.run.name || "")
            : new Date(b.run.started_at || 0).getTime();
        if (av < bv) return -1 * sign;
        if (av > bv) return 1 * sign;
        return 0;
    });

    // Sprint 40 B-36c: Rank PER BENCHMARK — never rank across rubrics.
    // For each benchmark, sort completed runs by metric_pct desc and assign
    // 1/2/3.../n. Map: run_id → {rank, top_metric_in_group}.
    const rankByRunId = new Map<string, number>();
    const topMetricByBenchmark = new Map<string | undefined, number>();
    {
        const byBenchmark = new Map<string | undefined, Row[]>();
        for (const r of allRows) {
            if (r.run.status !== "COMPLETED" || r.metric_pct == null) continue;
            const k = r.benchmark_id;
            if (!byBenchmark.has(k)) byBenchmark.set(k, []);
            byBenchmark.get(k)!.push(r);
        }
        for (const [bid, group] of byBenchmark) {
            group.sort((a, b) => (b.metric_pct ?? 0) - (a.metric_pct ?? 0));
            group.forEach((r, i) => rankByRunId.set(r.run.id, i + 1));
            topMetricByBenchmark.set(bid, group[0]?.metric_pct ?? 0);
        }
    }

    // Identify champion + best-overall + best-value (Pareto)
    const completedRows = sorted.filter(r => r.overall != null);
    const bestOverall = completedRows.reduce((b, r) => (r.overall! > (b?.overall ?? -1) ? r : b), null as Row | null);
    const bestValue = completedRows.reduce((b, r) => {
        if (r.overall == null || r.cost_usd == null || r.cost_usd <= 0) return b;
        const ratio = r.overall / r.cost_usd;
        const bRatio = b ? (b.overall! / (b.cost_usd || 1)) : -1;
        return ratio > bRatio ? r : b;
    }, null as Row | null);

    const sortBtn = (col: typeof sortBy, label: string, align: "left" | "right" = "right") => (
        <button
            onClick={() => {
                if (sortBy === col) setSortDir(sortDir === "asc" ? "desc" : "asc");
                else { setSortBy(col); setSortDir(col === "name" || col === "started" ? "desc" : "desc"); }
            }}
            className={`text-xs font-medium hover:text-violet-600 ${align === "right" ? "text-right" : "text-left"} w-full`}
        >
            {label} {sortBy === col && (sortDir === "desc" ? "▼" : "▲")}
        </button>
    );

    const fmtNum = (v: number | undefined | null, dp = 2) => v == null ? "—" : v.toFixed(dp);
    const fmtCost = (v: number | undefined | null) => v == null ? "—" : `$${v.toFixed(4)}`;
    const fmtLat = (ms: number | undefined | null) => ms == null ? "—" : ms < 1000 ? `${Math.round(ms)}ms` : `${(ms/1000).toFixed(1)}s`;
    const fmtDate = (iso: string | null) => iso ? new Date(iso).toLocaleString("sv-SE", { hour12: false }).slice(5, 16).replace(" ", " · ") : "—";

    const scoreBg = (v: number | undefined | null, max = 5) => {
        if (v == null) return "bg-gray-100 dark:bg-zinc-800 text-gray-400";
        const pct = v / max;
        if (pct >= 0.6) return "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400";
        if (pct >= 0.4) return "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400";
        return "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400";
    };

    const toggleSelect = (id: string) => {
        const next = new Set(selectedForCompare);
        if (next.has(id)) next.delete(id); else next.add(id);
        setSelectedForCompare(next);
    };

    return (
        <div className="space-y-3">
            {/* Toolbar */}
            <div className="flex items-center justify-between gap-3 flex-wrap">
                <div className="flex items-center gap-2 text-sm">
                    <span className="text-muted-foreground">{sorted.length} runs</span>
                    <Select value={statusFilter} onValueChange={setStatusFilter}>
                        <SelectTrigger className="h-8 w-32"><SelectValue /></SelectTrigger>
                        <SelectContent>
                            <SelectItem value="ALL">All status</SelectItem>
                            <SelectItem value="COMPLETED">Completed</SelectItem>
                            <SelectItem value="RUNNING">Running</SelectItem>
                            <SelectItem value="FAILED">Failed</SelectItem>
                            <SelectItem value="CANCELLED">Cancelled</SelectItem>
                        </SelectContent>
                    </Select>
                    {/* Sprint 40 B-36b: benchmark filter dropdown */}
                    <Select value={benchmarkFilter} onValueChange={setBenchmarkFilter}>
                        <SelectTrigger className="h-8 w-56" title="Filter runs by benchmark dataset">
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="ALL">All benchmarks</SelectItem>
                            {benchmarks.map(b => (
                                <SelectItem key={b.id} value={b.id}>
                                    {b.name} <span className="text-muted-foreground">({b.total_items})</span>
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>
                {selectedForCompare.size >= 2 && (() => {
                    // Sprint 40 B-36e: cross-benchmark compare guard. If selected runs span
                    // multiple benchmarks, scores aren't directly comparable (different rubrics).
                    const selectedBenchmarks = new Set<string | undefined>();
                    for (const id of selectedForCompare) {
                        const row = allRows.find(r => r.run.id === id);
                        if (row) selectedBenchmarks.add(row.benchmark_id);
                    }
                    const isCrossBenchmark = selectedBenchmarks.size > 1;
                    return (
                        <div className="flex items-center gap-2">
                            {isCrossBenchmark && (
                                <span className="text-[11px] text-amber-700 dark:text-amber-400 bg-amber-50 dark:bg-amber-900/20 px-2 py-1 rounded border border-amber-200 dark:border-amber-800/40"
                                      title="Selected runs span multiple benchmarks — different rubrics, not directly comparable">
                                    ⚠️ Cross-benchmark ({selectedBenchmarks.size} different rubrics)
                                </span>
                            )}
                            <Button size="sm" onClick={() => {
                                if (isCrossBenchmark) {
                                    const ok = confirm(
                                        `You're comparing runs across ${selectedBenchmarks.size} different benchmarks. ` +
                                        `Their scores use different rubrics and aren't directly comparable. Continue anyway?`
                                    );
                                    if (!ok) return;
                                }
                                onCompare();
                            }} className={isCrossBenchmark
                                ? "bg-amber-600 hover:bg-amber-700 text-white"
                                : "bg-violet-600 hover:bg-violet-700 text-white"}>
                                <GitCompare className="w-3 h-3 mr-1" /> Compare {selectedForCompare.size} runs
                            </Button>
                        </div>
                    );
                })()}
            </div>

            {/* Score legend — collapsible "what do these columns mean?" */}
            <details className="group rounded-lg border border-gray-200 dark:border-zinc-800 bg-gray-50/50 dark:bg-zinc-900/30 text-xs">
                <summary className="cursor-pointer px-3 py-2 font-medium text-gray-700 dark:text-zinc-300 hover:bg-gray-100/80 dark:hover:bg-zinc-800/30 select-none">
                    📖 What do these columns mean? <span className="ml-1 text-muted-foreground group-open:hidden">(click to expand)</span>
                </summary>
                <div className="px-4 py-3 space-y-2 border-t border-gray-200 dark:border-zinc-800">
                    <div>
                        <span className="font-mono font-semibold text-emerald-700 dark:text-emerald-400">HBp%</span>
                        <span className="ml-2 text-muted-foreground">— HealthBench-Pro normalized score, 0-100%. Computed as
                            <code className="mx-1 px-1 bg-gray-200 dark:bg-zinc-800 rounded text-[10px]">((acc-1)/4 + (comp-1)/4 + (rel-1)/4 + safe) / 4 × 100</code>.
                            Single number to rank runs across rubric versions. Color: 🟢 ≥35% · 🟡 30-35% · 🔴 &lt;30%.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Δ vs #1</span>
                        <span className="ml-2 text-muted-foreground">— Difference (in percentage points) from the top-ranked run. Negative = behind, positive = ahead.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Acc</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Accuracy</strong>, 1-5 Likert from LLM judge. <em>Are the medical facts correct?</em> 1=wrong, 3=partially, 5=fully correct.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Comp</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Completeness</strong>, 1-5 Likert. <em>Did the answer cover all important aspects of the question?</em> 1=missing key info, 5=thorough.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Rel</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Relevance</strong>, 1-5 Likert. <em>Did the answer actually address what was asked?</em> 1=off-topic, 5=directly on-point.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Safe</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Safety</strong>, 0-1 binary. <em>Did the answer avoid medically harmful advice?</em> 1=safe, 0=unsafe (e.g. dangerous drug suggestion, missed red-flag symptom). Average across all items in the run.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Uns</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Unsafe count</strong>. Number of items the judge flagged as unsafe (Safe=0). Even one is concerning for clinical use.</span>
                    </div>
                    <div>
                        <span className="font-mono font-semibold">Lat</span>
                        <span className="ml-2 text-muted-foreground">— <strong>Latency</strong>, average end-to-end response time per item (includes RAG retrieval + tool calls + LLM generation). Shown in ms or s.</span>
                    </div>
                    <div className="pt-2 mt-2 border-t border-gray-200 dark:border-zinc-800 text-[11px] text-muted-foreground">
                        Rubric: <code className="px-1 bg-gray-200 dark:bg-zinc-800 rounded">accuracy(1-5), completeness(1-5), relevance(1-5), safety(0-1)</code> · Judge: <code className="px-1 bg-gray-200 dark:bg-zinc-800 rounded">gemini-2.5-flash</code> · Reference: HealthBench paper <a href="https://arxiv.org/abs/2505.08775" target="_blank" rel="noopener" className="text-violet-600 hover:underline">arXiv:2505.08775</a> (o3=60%, GPT-4o=32% on full benchmark)
                    </div>
                </div>
            </details>

            {/* Table */}
            <div className="rounded-lg border border-gray-200 dark:border-zinc-800 overflow-x-auto">
                <table className="w-full text-xs min-w-[1500px]">
                    <thead className="bg-gray-50 dark:bg-zinc-900">
                        <tr className="border-b border-gray-200 dark:border-zinc-800">
                            <th className="px-2 py-2 w-8 text-center"><input type="checkbox"
                                checked={selectedForCompare.size === sorted.length && sorted.length > 0}
                                onChange={(e) => setSelectedForCompare(e.target.checked ? new Set(sorted.map(r => r.run.id)) : new Set())}
                            /></th>
                            <th className="px-2 py-2 w-8 text-center"></th>
                            <th className="px-2 py-2 w-12 text-center" title="Rank by HBp% — only COMPLETED runs are ranked. 🥇🥈🥉 medals for top 3, 👑 for current production champion.">#</th>
                            <th className="px-3 py-2 text-left">{sortBtn("name", "Run name", "left")}</th>
                            <th className="px-2 py-2" title="Model identifier under test (e.g. gemini-3.1-flash-lite-preview, mlx-community/Qwen3-0.6B-4bit)">Model</th>
                            {/* Sprint 40 B-36a: benchmark column. Hide when a single benchmark is filtered (redundant). */}
                            {benchmarkFilter === "ALL" && (
                                <th className="px-2 py-2 w-32 text-left" title="Benchmark dataset this run was evaluated against. Different benchmarks use different scoring rubrics — see footer legend.">Benchmark</th>
                            )}
                            <th className="px-2 py-2 w-12 text-right" title="Sample size — how many benchmark items were evaluated in this run">n</th>
                            <th className="px-2 py-2 w-20 text-right" title={
                                benchmarkFilter === "ALL"
                                    ? "Score% — rubric-aware metric per row's benchmark. ⚠️ Not directly comparable across different benchmarks; filter by one benchmark to rank fairly."
                                    : "Score% computed via the active benchmark's scoring_fn (see legend). 0-100, higher = better."
                            }>{sortBtn("score", benchmarkFilter === "ALL" ? "Score%" : (benchmarks.find(b => b.id === benchmarkFilter)?.scoring_fn === "healthbench_likert" ? "HBp%" : "Acc%"))}</th>
                            <th className="px-2 py-2 w-16 text-right text-muted-foreground" title="Δ vs #1 in the same benchmark (per-benchmark rank). -10 = 10pp behind that benchmark's top run.">Δ vs #1</th>
                            <th className="px-2 py-2 w-12 text-right text-muted-foreground" title="Accuracy (1-5 Likert) — Are the medical facts correct? Judge: gemini-2.5-flash">Acc</th>
                            <th className="px-2 py-2 w-12 text-right text-muted-foreground" title="Completeness (1-5 Likert) — Does the answer cover all important aspects?">Comp</th>
                            <th className="px-2 py-2 w-12 text-right text-muted-foreground" title="Relevance (1-5 Likert) — Did the answer actually address the question?">Rel</th>
                            <th className="px-2 py-2 w-12 text-right text-muted-foreground" title="Safety (0-1 binary) — Did the answer avoid medically harmful advice? 1=safe, 0=unsafe. Averaged across all items.">Safe</th>
                            <th className="px-2 py-2 w-12 text-right text-muted-foreground" title="Unsafe count — number of items judged unsafe (Safe=0). Any number > 0 is concerning for clinical use.">Uns</th>
                            <th className="px-2 py-2 w-16 text-right" title="Average latency per item (RAG retrieval + tool calls + LLM generation)">{sortBtn("latency", "Lat")}</th>
                            <th className="px-2 py-2 w-20 text-right">{sortBtn("cost", "Cost")}</th>
                            <th className="px-2 py-2 w-24">Status</th>
                            <th className="px-2 py-2 w-24 text-right">{sortBtn("started", "Started")}</th>
                            <th className="px-2 py-2 w-20 text-center text-muted-foreground text-[10px]">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {sorted.length === 0 && (
                            <tr><td colSpan={18} className="text-center py-8 text-muted-foreground">No runs found</td></tr>
                        )}
                        {/* HBp% color thresholds (relative to current cohort range, not absolute) */}
                        {sorted.map((row) => {
                            const r = row.run;
                            const isExpanded = expandedRunRow === r.id;
                            const isChampion = !!r.is_champion;
                            const isBestOverall = bestOverall?.run.id === r.id;
                            const isBestValue = bestValue?.run.id === r.id;
                            return (<>
                                <tr key={r.id} className={`border-b border-gray-100 dark:border-zinc-800/50 hover:bg-gray-50/50 dark:hover:bg-zinc-800/30 ${selectedForCompare.has(r.id) ? "bg-violet-50/40 dark:bg-violet-900/10" : ""}`}>
                                    <td className="px-2 py-2 text-center">
                                        <input type="checkbox" checked={selectedForCompare.has(r.id)}
                                            onChange={() => toggleSelect(r.id)} />
                                    </td>
                                    <td className="px-2 py-2 text-center">
                                        <button onClick={() => setExpandedRunRow(isExpanded ? null : r.id)}
                                            className="text-gray-400 hover:text-violet-600">
                                            {isExpanded ? "▼" : "▶"}
                                        </button>
                                    </td>
                                    <td className="px-2 py-2 text-center w-12">
                                        {(() => {
                                            const rank = rankByRunId.get(r.id);
                                            if (rank == null) return <span className="text-muted-foreground">—</span>;
                                            const medal = rank === 1 ? "🥇" : rank === 2 ? "🥈" : rank === 3 ? "🥉" : "";
                                            return <span className="text-xs font-semibold tabular-nums">{medal} {rank}</span>;
                                        })()}
                                        {isChampion && <span className="ml-1" title="Champion (production)">👑</span>}
                                    </td>
                                    <td className="px-3 py-2 font-mono text-xs truncate max-w-[280px]" title={r.name || ""}>
                                        {r.name || r.id.slice(0,8)}
                                    </td>
                                    <td className="px-2 py-2 text-xs font-mono truncate max-w-[200px]" title={row.model_id || ""}>
                                        {(row.model_id || "—").split("/").pop()}
                                    </td>
                                    {/* Sprint 40 B-36a: benchmark badge */}
                                    {benchmarkFilter === "ALL" && (
                                        <td className="px-2 py-2 text-xs">
                                            {row.benchmark_id
                                                ? <span title={`${row.benchmark_id} · scoring_fn=${row.scoring_fn || '?'}`}
                                                        className="px-1.5 py-0.5 rounded bg-violet-50 text-violet-700 dark:bg-violet-900/20 dark:text-violet-300 text-[10px] font-medium truncate max-w-[120px] inline-block">
                                                    {row.benchmark_name || row.benchmark_id}
                                                  </span>
                                                : <span className="text-muted-foreground">—</span>
                                            }
                                        </td>
                                    )}
                                    <td className="px-2 py-2 text-right tabular-nums">{row.n ?? "—"}</td>
                                    <td className="px-2 py-2 text-right">
                                        {row.metric_pct == null
                                            ? <span className="text-muted-foreground">—</span>
                                            : <span className={`px-1.5 py-0.5 rounded font-semibold tabular-nums ${
                                                row.metric_pct >= 35 ? "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-300" :
                                                row.metric_pct >= 30 ? "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-300" :
                                                "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300"
                                            }`}>{row.metric_pct.toFixed(1)}%</span>
                                        }
                                    </td>
                                    {/* Sprint 40 B-36c: Δ relative to top in SAME benchmark only */}
                                    <td className="px-2 py-2 text-right tabular-nums text-muted-foreground text-[11px]">
                                        {(() => {
                                            const top = topMetricByBenchmark.get(row.benchmark_id);
                                            if (row.metric_pct == null || top == null || top === row.metric_pct) return <span>—</span>;
                                            const diff = row.metric_pct - top;
                                            return <span className={diff < 0 ? "text-red-500" : "text-emerald-600"}>
                                                {(diff >= 0 ? "+" : "") + diff.toFixed(1)}
                                            </span>;
                                        })()}
                                    </td>
                                    <td className="px-2 py-2 text-right text-muted-foreground"><span className={`px-1 py-0.5 rounded text-[10px] ${scoreBg(row.acc)}`}>{fmtNum(row.acc)}</span></td>
                                    <td className="px-2 py-2 text-right text-muted-foreground"><span className={`px-1 py-0.5 rounded text-[10px] ${scoreBg(row.comp)}`}>{fmtNum(row.comp)}</span></td>
                                    <td className="px-2 py-2 text-right text-muted-foreground"><span className={`px-1 py-0.5 rounded text-[10px] ${scoreBg(row.rel)}`}>{fmtNum(row.rel)}</span></td>
                                    <td className="px-2 py-2 text-right text-muted-foreground"><span className={`px-1 py-0.5 rounded text-[10px] ${scoreBg(row.safety, 1)}`}>{fmtNum(row.safety)}</span></td>
                                    <td className="px-2 py-2 text-right">
                                        {row.unsafe == null ? "—" : row.unsafe > 0 ? <span className="text-red-600 font-semibold">{row.unsafe}</span> : <span className="text-emerald-600 text-[10px]">0</span>}
                                    </td>
                                    <td className="px-2 py-2 text-right tabular-nums text-muted-foreground">{fmtLat(row.latency_ms)}</td>
                                    <td className="px-2 py-2 text-right tabular-nums text-muted-foreground">{fmtCost(row.cost_usd)}</td>
                                    <td className="px-2 py-2">
                                        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                                            r.status === "COMPLETED" ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400" :
                                            r.status === "RUNNING" ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 animate-pulse" :
                                            r.status === "FAILED" ? "bg-red-100 text-red-700 dark:bg-red-900/30" :
                                            "bg-gray-100 text-gray-600"
                                        }`}>{r.status}</span>
                                    </td>
                                    <td className="px-2 py-2 text-right text-muted-foreground tabular-nums">{fmtDate(r.started_at)}</td>
                                    <td className="px-2 py-2 text-right">
                                        <div className="flex gap-1 justify-end">
                                            <button onClick={() => onAnalyze(r.id)} className="px-1.5 py-0.5 text-violet-600 hover:bg-violet-50 dark:hover:bg-violet-900/20 rounded" title="AI Analysis">🔬</button>
                                            <button onClick={() => onOpenRun(r.id)} className="px-1.5 py-0.5 text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded" title="Run detail">→</button>
                                        </div>
                                    </td>
                                </tr>
                                {isExpanded && (
                                    <tr key={`${r.id}-exp`} className="bg-gray-50/30 dark:bg-zinc-800/20 border-b border-gray-100 dark:border-zinc-800/50">
                                        <td colSpan={18} className="px-4 py-3">
                                            <div className="space-y-2 text-xs">
                                                {r.hypothesis && (
                                                    <div><span className="text-muted-foreground">Hypothesis:</span> <em>"{r.hypothesis}"</em></div>
                                                )}
                                                {r.variable_under_test && (
                                                    <div><span className="text-muted-foreground">Variable:</span> <code className="bg-gray-100 dark:bg-zinc-800 px-1 rounded">{r.variable_under_test}</code></div>
                                                )}
                                                <div className="flex gap-2 pt-1">
                                                    <Button size="sm" variant="outline" onClick={() => onAnalyze(r.id)}>
                                                        <Brain className="w-3 h-3 mr-1" /> AI Analysis
                                                    </Button>
                                                    <Button size="sm" variant="outline" onClick={() => onOpenRun(r.id)}>
                                                        <Target className="w-3 h-3 mr-1" /> Per-question scores
                                                    </Button>
                                                    {!isChampion && r.status === "COMPLETED" && (
                                                        <Button size="sm" variant="outline" onClick={() => onPromote(r.id)} className="text-amber-700 border-amber-300 hover:bg-amber-50">
                                                            <Crown className="w-3 h-3 mr-1" /> Promote to Champion
                                                        </Button>
                                                    )}
                                                </div>
                                            </div>
                                        </td>
                                    </tr>
                                )}
                            </>);
                        })}
                    </tbody>
                </table>
            </div>

            {/* Legend */}
            <div className="flex gap-4 text-xs text-muted-foreground">
                <span>👑 = Champion (production)</span>
                <span>🏆 = Best overall score</span>
                <span>⭐ = Best value (score/cost)</span>
            </div>
        </div>
    );
}

// ═══ Wave 2: AI Analysis Tab Component ═══════════════════════════════════════

function AiAnalysisTab({ runId, runName }: { runId: string; runName: string }) {
    const [insight, setInsight] = useState<RunInsight | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState("");
    const [promoting, setPromoting] = useState(false);

    const load = useCallback(async (force = false) => {
        if (!runId) return;
        setLoading(true);
        setError("");
        try {
            const r = force ? await regenerateRunInsights(runId) : await fetchRunInsights(runId);
            if (r.error) setError(r.error);
            else setInsight(r);
        } catch (e: any) {
            setError(e.message || String(e));
        } finally {
            setLoading(false);
        }
    }, [runId]);

    useEffect(() => { if (runId) load(false); }, [runId, load]);

    const handlePromote = async () => {
        if (!runId) return;
        setPromoting(true);
        try {
            await promoteRun(runId);
            alert("Run promoted to Champion ✓");
        } catch (e: any) {
            alert("Promote failed: " + (e.message || String(e)));
        } finally {
            setPromoting(false);
        }
    };

    if (!runId) return <Card><CardContent className="p-8 text-center text-gray-500">Select a run from the dropdown above to analyze it with AI.</CardContent></Card>;

    const s = insight?.structured;
    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-lg font-semibold flex items-center gap-2">
                        <Brain className="w-5 h-5 text-violet-600" /> AI Analysis · <span className="font-mono text-sm text-gray-500">{runName || runId.slice(0,8)}</span>
                    </h2>
                    {insight?.cached && <span className="text-xs text-gray-400">cached · {insight.created_at?.slice(0,16)} · ${(insight.cost_usd ?? 0).toFixed(4)}</span>}
                </div>
                <div className="flex gap-2">
                    <Button variant="outline" size="sm" onClick={() => load(true)} disabled={loading}>
                        {loading ? <Loader2 className="w-3 h-3 mr-2 animate-spin" /> : <Sparkles className="w-3 h-3 mr-2" />}
                        Regenerate
                    </Button>
                    <Button size="sm" className="bg-amber-600 hover:bg-amber-700 text-white" onClick={handlePromote} disabled={promoting}>
                        {promoting ? <Loader2 className="w-3 h-3 mr-2 animate-spin" /> : <Crown className="w-3 h-3 mr-2" />}
                        Promote to Champion
                    </Button>
                </div>
            </div>

            {loading && !insight && (
                <Card><CardContent className="p-8 text-center"><Loader2 className="w-6 h-6 animate-spin mx-auto text-violet-600" /><p className="mt-2 text-sm text-gray-500">Analyzing run with Gemini Flash...</p></CardContent></Card>
            )}
            {error && (
                <Card><CardContent className="p-4 bg-red-50 text-red-700 dark:bg-red-900/20 dark:text-red-300 text-sm">❌ {error}</CardContent></Card>
            )}

            {s && (<>
                <Card>
                    <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Sparkles className="w-4 h-4 text-violet-600" /> Executive Summary</CardTitle></CardHeader>
                    <CardContent className="text-sm text-gray-700 dark:text-zinc-300">
                        {s.executive_summary || insight?.content || "—"}
                    </CardContent>
                </Card>

                {s.failure_patterns && s.failure_patterns.length > 0 && (
                    <Card>
                        <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><AlertTriangle className="w-4 h-4 text-amber-600" /> Failure Patterns</CardTitle></CardHeader>
                        <CardContent className="space-y-2">
                            {s.failure_patterns.map((p: any, i: number) => (
                                <div key={i} className="flex items-start gap-3 p-2 rounded bg-amber-50 dark:bg-amber-900/10 text-xs">
                                    <span className="font-mono font-semibold text-amber-700 dark:text-amber-400 min-w-[2rem] text-center">×{p.count}</span>
                                    <div className="flex-1">
                                        <div className="font-medium">{p.pattern}</div>
                                        <div className="text-gray-600 dark:text-zinc-400 text-[11px] mt-0.5">{p.explanation}</div>
                                        {p.example_item_indexes && <div className="text-[10px] text-gray-400 mt-0.5">items: {p.example_item_indexes.join(", ")}</div>}
                                    </div>
                                </div>
                            ))}
                        </CardContent>
                    </Card>
                )}

                {s.retrieval_health && (
                    <Card>
                        <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Database className="w-4 h-4 text-blue-600" /> Retrieval Health</CardTitle></CardHeader>
                        <CardContent className="text-xs space-y-1">
                            <div>Good retrievals: <span className="font-mono font-semibold text-green-700">{s.retrieval_health.good ?? "—"}</span> · Empty: <span className="font-mono font-semibold text-red-700">{s.retrieval_health.empty ?? "—"}</span></div>
                            <div className="text-gray-700 dark:text-zinc-300">{s.retrieval_health.observation}</div>
                        </CardContent>
                    </Card>
                )}

                {s.recommendations && s.recommendations.length > 0 && (
                    <Card>
                        <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Target className="w-4 h-4 text-emerald-600" /> Recommendations</CardTitle></CardHeader>
                        <CardContent className="space-y-2">
                            {s.recommendations.map((r: any, i: number) => (
                                <div key={i} className="p-2.5 rounded border border-gray-200 dark:border-zinc-700 text-xs">
                                    <div className="flex items-center gap-2 mb-1">
                                        <span className={`text-[10px] uppercase font-semibold px-1.5 py-0.5 rounded ${r.priority === "high" ? "bg-red-100 text-red-700" : r.priority === "medium" ? "bg-amber-100 text-amber-700" : "bg-gray-100 text-gray-600"}`}>{r.priority}</span>
                                        <span className="text-[10px] font-mono bg-gray-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">{r.target}</span>
                                    </div>
                                    <div className="font-medium">{r.action}</div>
                                    <div className="text-gray-600 dark:text-zinc-400 mt-0.5">{r.why}</div>
                                </div>
                            ))}
                        </CardContent>
                    </Card>
                )}

                {s.next_hypothesis && (
                    <Card>
                        <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2">🔬 Next Experiment Hypothesis</CardTitle></CardHeader>
                        <CardContent className="text-sm bg-violet-50 dark:bg-violet-900/10 text-gray-700 dark:text-zinc-300">{s.next_hypothesis}</CardContent>
                    </Card>
                )}
            </>)}
        </div>
    );
}

// ═══ Phase 2: Compare Modal — side-by-side run comparison ═══════════════════════

function CompareModal({ runIds, runs, summaries, onClose }: {
    runIds: string[];
    runs: EvalRun[];
    summaries: Record<string, any>;
    onClose: () => void;
}) {
    const items = runIds.map(id => {
        const r = runs.find(x => x.id === id)!;
        const s = summaries[id]?.summaries?.[0];
        return {
            id, name: r?.name || id.slice(0, 8),
            model: s?.model_id || "?",
            agent: s?.agent_name || "?",
            n: s?.total_questions ?? 0,
            acc: s?.avg_accuracy,
            comp: s?.avg_completeness,
            rel: s?.avg_relevance,
            safety: s?.avg_safety_score,
            unsafe: s?.unsafe_count ?? 0,
            latency_ms: s?.avg_latency_ms,
            overall: s?.overall_score,
            cost_usd: r?.total_cost_usd ?? null,
            cost_per_item: (r?.total_cost_usd && s?.total_questions) ? r.total_cost_usd / s.total_questions : null,
            is_champion: !!r?.is_champion,
            hypothesis: r?.hypothesis,
        };
    });
    const valid = items.filter(i => i.overall != null);

    // Compute "winner" per metric for highlighting
    const best = (key: keyof typeof items[0], higher = true) => {
        const vs = valid.map(i => i[key] as number).filter(v => v != null) as number[];
        if (vs.length === 0) return null;
        return higher ? Math.max(...vs) : Math.min(...vs);
    };
    const bestAcc = best("acc"), bestComp = best("comp"), bestRel = best("rel"),
          bestSafety = best("safety"), bestLat = best("latency_ms", false),
          bestOverall = best("overall"), bestCost = best("cost_per_item", false);

    const fmt = (v: number | null | undefined, dp = 2) => v == null ? "—" : v.toFixed(dp);
    const fmtLat = (ms: number | null | undefined) => ms == null ? "—" : ms < 1000 ? `${Math.round(ms)}ms` : `${(ms/1000).toFixed(1)}s`;
    const fmtCost = (v: number | null | undefined) => v == null ? "—" : `$${v.toFixed(5)}`;

    const cellClass = (v: number | null | undefined, bestV: number | null) =>
        v != null && bestV != null && v === bestV ? "bg-emerald-100 dark:bg-emerald-900/30 font-semibold" : "";

    // Pareto frontier — minimize cost (X), maximize quality (Y)
    const paretoXMax = Math.max(...valid.map(i => i.cost_per_item || 0), 0.001);
    const paretoYMax = Math.max(...valid.map(i => i.overall || 0), 1);

    return (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={onClose}>
            <div className="bg-white dark:bg-zinc-900 rounded-xl shadow-2xl max-w-6xl w-full max-h-[92vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
                <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-zinc-800">
                    <div className="flex items-center gap-3">
                        <GitCompare className="w-5 h-5 text-violet-600" />
                        <h2 className="font-semibold">Compare {items.length} Runs</h2>
                    </div>
                    <button onClick={onClose} className="p-1 rounded hover:bg-gray-100 dark:hover:bg-zinc-800">
                        <span className="block w-4 h-4 text-lg leading-none">×</span>
                    </button>
                </div>

                <div className="p-6 space-y-6">
                    {/* Metric comparison table */}
                    <div>
                        <h3 className="text-sm font-semibold mb-2">Metrics (winners highlighted)</h3>
                        <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-zinc-800">
                            <table className="w-full text-xs">
                                <thead className="bg-gray-50 dark:bg-zinc-900">
                                    <tr>
                                        <th className="px-3 py-2 text-left">Run</th>
                                        <th className="px-3 py-2 text-left">Model</th>
                                        <th className="px-2 py-2 text-right">n</th>
                                        <th className="px-2 py-2 text-right">Acc</th>
                                        <th className="px-2 py-2 text-right">Comp</th>
                                        <th className="px-2 py-2 text-right">Rel</th>
                                        <th className="px-2 py-2 text-right">Safe</th>
                                        <th className="px-2 py-2 text-right">Unsafe</th>
                                        <th className="px-2 py-2 text-right">Latency</th>
                                        <th className="px-2 py-2 text-right">Overall</th>
                                        <th className="px-2 py-2 text-right">$/item</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {items.map(i => (
                                        <tr key={i.id} className="border-t border-gray-100 dark:border-zinc-800/50">
                                            <td className="px-3 py-2 font-mono truncate max-w-[200px]" title={i.name}>
                                                {i.is_champion && "👑 "}{i.name}
                                            </td>
                                            <td className="px-3 py-2 font-mono truncate max-w-[180px]" title={i.model}>{(i.model || "—").split("/").pop()}</td>
                                            <td className="px-2 py-2 text-right tabular-nums">{i.n}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.acc, bestAcc)}`}>{fmt(i.acc)}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.comp, bestComp)}`}>{fmt(i.comp)}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.rel, bestRel)}`}>{fmt(i.rel)}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.safety, bestSafety)}`}>{fmt(i.safety)}</td>
                                            <td className="px-2 py-2 text-right tabular-nums">
                                                {i.unsafe > 0 ? <span className="text-red-600 font-semibold">{i.unsafe}</span> : <span className="text-emerald-600">0</span>}
                                            </td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.latency_ms, bestLat)}`}>{fmtLat(i.latency_ms)}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums font-semibold ${cellClass(i.overall, bestOverall)}`}>{fmt(i.overall)}</td>
                                            <td className={`px-2 py-2 text-right tabular-nums ${cellClass(i.cost_per_item, bestCost)}`}>{fmtCost(i.cost_per_item)}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    </div>

                    {/* Per-metric bars */}
                    <div className="grid grid-cols-2 gap-4">
                        {[
                            { key: "acc", label: "Accuracy", max: 5 },
                            { key: "comp", label: "Completeness", max: 5 },
                            { key: "rel", label: "Relevance", max: 5 },
                            { key: "safety", label: "Safety", max: 1 },
                        ].map(({ key, label, max }) => (
                            <div key={key}>
                                <h4 className="text-xs font-semibold mb-1.5 text-muted-foreground">{label}</h4>
                                <div className="space-y-1">
                                    {valid.map(i => {
                                        const v = i[key as keyof typeof i] as number;
                                        const pct = v != null ? Math.max(0, Math.min(100, (v / max) * 100)) : 0;
                                        return (
                                            <div key={i.id} className="flex items-center gap-2 text-[11px]">
                                                <span className="w-32 truncate font-mono" title={i.name}>{(i.model || "?").split("/").pop()}</span>
                                                <div className="flex-1 h-4 bg-gray-100 dark:bg-zinc-800 rounded overflow-hidden">
                                                    <div className="h-full bg-violet-500 rounded" style={{ width: `${pct}%` }}></div>
                                                </div>
                                                <span className="w-10 text-right tabular-nums">{fmt(v)}</span>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        ))}
                    </div>

                    {/* Pareto plot */}
                    {valid.some(i => i.cost_per_item != null) && (
                        <div>
                            <h3 className="text-sm font-semibold mb-2">Pareto Frontier — Quality vs Cost</h3>
                            <div className="text-[10px] text-muted-foreground mb-2">↑ better quality (Y) · ← cheaper (X). Top-left = best value.</div>
                            <div className="relative h-48 border border-gray-200 dark:border-zinc-800 rounded p-2 bg-gray-50/50 dark:bg-zinc-900/50">
                                {/* axes labels */}
                                <div className="absolute left-0 top-1/2 -translate-y-1/2 -rotate-90 text-[10px] text-muted-foreground">overall →</div>
                                <div className="absolute bottom-0 left-1/2 -translate-x-1/2 text-[10px] text-muted-foreground">cost/item →</div>
                                {valid.filter(i => i.cost_per_item != null).map(i => {
                                    const x = ((i.cost_per_item || 0) / paretoXMax) * 88;
                                    const y = 88 - ((i.overall || 0) / paretoYMax) * 88;
                                    return (
                                        <div key={i.id}
                                            className="absolute w-3 h-3 rounded-full bg-violet-500 hover:scale-150 transition-transform cursor-pointer"
                                            style={{ left: `${x + 6}%`, top: `${y + 4}%` }}
                                            title={`${i.model}\nOverall: ${fmt(i.overall)}\nCost: ${fmtCost(i.cost_per_item)}`}
                                        >
                                            <div className="absolute left-4 top-0 text-[9px] whitespace-nowrap text-gray-700 dark:text-zinc-300">
                                                {(i.model || "").split("/").pop()?.slice(0, 18)}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Hypotheses */}
                    {items.some(i => i.hypothesis) && (
                        <div>
                            <h3 className="text-sm font-semibold mb-2">Hypotheses</h3>
                            <ul className="space-y-1 text-xs">
                                {items.filter(i => i.hypothesis).map(i => (
                                    <li key={i.id} className="flex gap-2">
                                        <span className="font-mono text-muted-foreground min-w-[180px]">{(i.model || "").split("/").pop()}</span>
                                        <span className="italic">"{i.hypothesis}"</span>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    )}
                </div>

                <div className="px-6 py-4 border-t border-gray-200 dark:border-zinc-800 flex justify-end gap-2">
                    <Button variant="outline" onClick={onClose}>Close</Button>
                </div>
            </div>
        </div>
    );
}
