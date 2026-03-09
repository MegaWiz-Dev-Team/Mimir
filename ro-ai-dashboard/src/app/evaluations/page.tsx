"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge";
import { RefreshCw, ArrowLeft, ChevronDown, ChevronUp, Star, Clock, Target, CheckCircle, GitCompare, BarChart3, ThumbsUp, ThumbsDown, Beaker, Database, Workflow } from "lucide-react";
import Link from "next/link";
import { EvalWizard } from "@/components/evaluations/eval-wizard";
import { EvalScoreOverride } from "@/components/evaluations/eval-score-override";
import { compareModels, getFeedbackSummary } from "@/lib/api";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api";

// ─── Types ─────────────────────────────────────────────────────────────
interface EvalRun {
    id: string;
    name: string | null;
    status: string;
    total_combinations: number;
    completed_combinations: number;
    started_at: string;
    finished_at: string | null;
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
    judge_model: string | null;
    judge_reasoning: string | null;
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
    const [activeTab, setActiveTab] = useState<"matrix" | "performance" | "extraction" | "retrieval" | "pipeline">("matrix");

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
            const res = await fetch(`${API_BASE}/eval/runs`, { cache: "no-store" });
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

    const loadMatrix = useCallback(async (runId: string) => {
        try {
            const res = await fetch(`${API_BASE}/eval/runs/${runId}/matrix`, { cache: "no-store" });
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
            const res = await fetch(
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
            fetch(`${API_BASE}/evaluations/extraction-summary`, { cache: "no-store" })
                .then(r => r.json()).then(setExtractionData)
                .catch(console.warn).finally(() => setLoadingExtraction(false));
        }
        if (activeTab === "retrieval" && !retrievalData) {
            setLoadingRetrieval(true);
            fetch(`${API_BASE}/evaluations/retrieval-summary`, { cache: "no-store" })
                .then(r => r.json()).then(setRetrievalData)
                .catch(console.warn).finally(() => setLoadingRetrieval(false));
        }
        if (activeTab === "pipeline" && !pipelineData) {
            setLoadingPipeline(true);
            fetch(`${API_BASE}/evaluations/pipeline-scorecard`, { cache: "no-store" })
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
            <div className="flex gap-1 bg-zinc-100 dark:bg-zinc-900 rounded-lg p-1 mb-6">
                <button onClick={() => setActiveTab("matrix")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "matrix" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Target className="w-4 h-4" /> Eval Matrix
                </button>
                <button onClick={() => setActiveTab("performance")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "performance" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <GitCompare className="w-4 h-4" /> Model Performance
                </button>
                <button onClick={() => setActiveTab("extraction")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "extraction" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Beaker className="w-4 h-4" /> Extraction
                </button>
                <button onClick={() => setActiveTab("retrieval")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "retrieval" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Database className="w-4 h-4" /> Retrieval
                </button>
                <button onClick={() => setActiveTab("pipeline")}
                    className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === "pipeline" ? "bg-white dark:bg-zinc-800 shadow-sm" : "text-gray-500 hover:text-gray-700"
                        }`}>
                    <Workflow className="w-4 h-4" /> Pipeline
                </button>
            </div>

            {activeTab === "matrix" && (<>

                {/* Summary Cards */}
                {selectedRun && (
                    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 mb-8">
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
                                            <TableHead>Question</TableHead>
                                            <TableHead>Expected</TableHead>
                                            <TableHead>Actual</TableHead>
                                            <TableHead className="text-center">Acc</TableHead>
                                            <TableHead className="text-center">Comp</TableHead>
                                            <TableHead className="text-center">Rel</TableHead>
                                            <TableHead className="text-center">Latency</TableHead>
                                            <TableHead className="text-center">Human</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {scores.map((s, i) => (
                                            <TableRow key={s.id}>
                                                <TableCell className="text-muted-foreground text-xs">{i + 1}</TableCell>
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
                                                    {formatLatency(s.latency_ms)}
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
                                        ))}
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
        </div>
    );
}
