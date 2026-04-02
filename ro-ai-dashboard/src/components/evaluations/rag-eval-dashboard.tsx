"use client";

import { useState, useCallback, useEffect, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Loader2, TrendingUp, Target, Zap, ChevronDown, ChevronRight, CheckCircle2, XCircle, FileJson, Wand2 } from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";

// ── Types ──────────────────────────────────────────

interface RagEvalRun {
  id: string;
  name: string | null;
  status: string;
  params: {
    weights: { vector: number; tree: number; graph: number };
    top_k: number;
    vector_alpha: number;
    vector_threshold: number;
    graph_hops: number;
    rerank?: { enabled: boolean; strategy: string; model: string | null; final_top_k: number };
  };
  scores: {
    hit_rate: number;
    mrr: number;
    ndcg: number;
    precision_at_k: number;
    recall_at_k: number;
    avg_latency_ms: number;
    faithfulness: number | null;
    answer_relevancy: number | null;
    context_precision: number | null;
    vector_hit_rate: number | null;
    tree_hit_rate: number | null;
    graph_hit_rate: number | null;
  };
  total_queries: number;
  eval_mode?: string;
  started_at: string;
}

interface RagEvalQuery {
  query: string;
  hit: boolean;
  reciprocal_rank: number;
  ndcg_score: number;
  precision: number;
  recall: number;
  matched_at_rank: number | null;
  vector_contributed: boolean;
  tree_contributed: boolean;
  graph_contributed: boolean;
  top_results: any[];
  generated_answer: string | null;
  faithfulness: number | null;
  answer_relevancy: number | null;
  context_precision: number | null;
  judge_reasoning: string | null;
  total_latency_ms: number;
}

interface RunDetailResponse {
  run: RagEvalRun;
  per_query: RagEvalQuery[];
}

export function RagEvalDashboard() {
  const [runs, setRuns] = useState<RagEvalRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedRunIds, setSelectedRunIds] = useState<string[]>([]);
  const [drillDownData, setDrillDownData] = useState<RunDetailResponse | null>(null);
  const [drillDownLoading, setDrillDownLoading] = useState(false);
  const [expandedQuery, setExpandedQuery] = useState<number | null>(null);

  // AutoTune state
  const [autoTuneOpen, setAutoTuneOpen] = useState(false);
  const [tuneIterations, setTuneIterations] = useState(3);
  const [tuningJobId, setTuningJobId] = useState<string | null>(null);
  const [tuningStatus, setTuningStatus] = useState<any>(null);

  const apiOrigin = API_BASE_URL.replace(/\/api\/v1$/, "");

  const fetchRuns = useCallback(async () => {
    try {
      setLoading(true);
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs?per_page=10`);
      if (resp.ok) {
        const data = await resp.json();
        setRuns(data.runs || []);
        if (data.runs?.length > 0 && selectedRunIds.length === 0) {
          setSelectedRunIds([data.runs[0].id]);
        }
      }
    } catch (e) {
      console.error("Failed to fetch eval runs", e);
    } finally {
      setLoading(false);
    }
  }, [apiOrigin, selectedRunIds.length]);

  useEffect(() => {
    fetchRuns();
  }, [fetchRuns]);

  // Job Polling
  useEffect(() => {
    if (!tuningJobId) return;
    let active = true;
    const poll = async () => {
      if (!active) return;
      try {
        const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/auto-tune/${tuningJobId}`);
        if (resp.ok) {
          const data = await resp.json();
          setTuningStatus(data);
          if (data.status === "completed" || data.status === "failed") {
            setTuningJobId(null);
            fetchRuns();
          }
        }
      } catch (e) {}
    };
    const intv = setInterval(poll, 3000);
    return () => { active = false; clearInterval(intv); };
  }, [tuningJobId, apiOrigin, fetchRuns]);

  const fetchDrillDown = async (runId: string) => {
    try {
      setDrillDownLoading(true);
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${runId}`);
      if (resp.ok) {
        const data = await resp.json();
        setDrillDownData(data);
      }
    } catch (e) {
      console.error("Failed to fetch drill down", e);
    } finally {
      setDrillDownLoading(false);
    }
  };

  const toggleRunSelection = (id: string) => {
    setSelectedRunIds((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      if (prev.length >= 3) return [...prev.slice(1), id];
      return [...prev, id];
    });
  };

  const deployConfig = async (runId: string) => {
    if (!confirm("Are you sure you want to deploy this configuration to your local Agent Config?")) return;
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${runId}/deploy`, { method: "POST" });
      if (resp.ok) {
        alert("Configuration copied! You can now use these params in Agent Studio.");
      }
    } catch (e) {
      console.error("Failed to deploy", e);
    }
  };

  const triggerAutoTune = async () => {
    if (selectedRunsData.length !== 1) return;
    const baseRun = selectedRunsData[0];
    if (!drillDownData || drillDownData.run.id !== baseRun.id) {
      alert("Please drill down into the selected run first so we can extract its dataset.");
      return;
    }
    const evalSet = drillDownData.per_query.map(q => ({
      query: q.query,
      expected_titles: (q as any).expected_titles || [],
      expected_content: (q as any).expected_content || null
    }));
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/auto-tune`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          base_params: baseRun.params,
          eval_set: evalSet,
          iterations: tuneIterations,
          target_metric: "ndcg"
        })
      });
      if (resp.ok) {
        const json = await resp.json();
        setTuningJobId(json.job_id);
        setAutoTuneOpen(false);
      }
    } catch (e) { console.error(e); }
  };

  const selectedRunsData = useMemo(() => {
    return selectedRunIds.map((id) => runs.find((r) => r.id === id)).filter(Boolean) as RagEvalRun[];
  }, [selectedRunIds, runs]);

  if (loading && !runs.length) {
    return <div className="flex justify-center p-8"><Loader2 className="h-8 w-8 animate-spin text-muted-foreground" /></div>;
  }

  // Find best values for comparison highlighting
  const bestScores = {
    hit_rate: Math.max(...selectedRunsData.map(r => r.scores.hit_rate || 0)),
    mrr: Math.max(...selectedRunsData.map(r => r.scores.mrr || 0)),
    ndcg: Math.max(...selectedRunsData.map(r => r.scores.ndcg || 0)),
    faithfulness: Math.max(...selectedRunsData.map(r => r.scores.faithfulness || 0)),
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <CardTitle>Evaluation Run Matrix</CardTitle>
            <CardDescription>Select up to 3 runs to compare their configurations and scoring metrics.</CardDescription>
          </div>
          {tuningJobId ? (
            <Badge className="bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 px-3 py-1.5 flex items-center">
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              Tuning: Iteration {tuningStatus?.current_iteration || 0}/{tuningStatus?.total_iterations || 0}
            </Badge>
          ) : (
            <Button 
              variant="secondary"
              className="gap-2 bg-gradient-to-r from-purple-500/10 to-indigo-500/10 border-purple-500/20 text-purple-600 hover:bg-purple-500/20 
                disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={selectedRunIds.length !== 1}
              onClick={() => {
                if (selectedRunsData.length !== 1) return;
                if (!drillDownData || drillDownData.run.id !== selectedRunsData[0].id) {
                  fetchDrillDown(selectedRunsData[0].id).then(() => setAutoTuneOpen(true));
                } else {
                  setAutoTuneOpen(true);
                }
              }}
              title="Select exactly 1 run to baseline from"
            >
              <Wand2 className="w-4 h-4" /> Auto-Tune
            </Button>
          )}
        </CardHeader>
        <CardContent>
          <div className="flex gap-2 mb-6 overflow-x-auto pb-2">
            {runs.map((r) => (
              <Badge
                key={r.id}
                variant={selectedRunIds.includes(r.id) ? "default" : "outline"}
                className="cursor-pointer whitespace-nowrap"
                onClick={() => toggleRunSelection(r.id)}
              >
                {r.name || new Date(r.started_at).toLocaleString()}
                {r.status === "running" && <Loader2 className="ml-2 h-3 w-3 animate-spin" />}
              </Badge>
            ))}
          </div>

          {selectedRunsData.length > 0 && (
            <div className="overflow-x-auto border rounded-xl">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted/50 text-muted-foreground border-b uppercase text-xs">
                  <tr>
                    <th className="px-4 py-3 font-medium">Metric</th>
                    {selectedRunsData.map((r) => (
                      <th key={r.id} className="px-4 py-3 font-medium">
                        <div className="flex flex-col">
                          <span className="truncate max-w-[150px] text-foreground">{r.name || "Unnamed"}</span>
                          <span className="text-[10px] font-normal opacity-70">
                            {new Date(r.started_at).toLocaleDateString()}
                          </span>
                        </div>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {/* Parameter rows */}
                  <tr className="bg-muted/10"><td colSpan={selectedRunsData.length + 1} className="px-4 py-2 font-semibold">📍 Parameters</td></tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">Weights (V/T/G)</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        {r.params.weights.vector.toFixed(2)} / {r.params.weights.tree.toFixed(2)} / {r.params.weights.graph.toFixed(2)}
                      </td>
                    ))}
                  </tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">Top K</td>
                    {selectedRunsData.map((r) => <td key={r.id} className="px-4 py-3">{r.params.top_k}</td>)}
                  </tr>

                  {/* Retrieval primary metrics */}
                  <tr className="bg-muted/10"><td colSpan={selectedRunsData.length + 1} className="px-4 py-2 font-semibold">🔍 Retrieval Quality</td></tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">Hit Rate@K</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        <span className={cn(r.scores.hit_rate === bestScores.hit_rate && bestScores.hit_rate > 0 ? "font-bold text-amber-500" : "")}>
                          {(r.scores.hit_rate * 100).toFixed(1)}%
                          {r.scores.hit_rate === bestScores.hit_rate && bestScores.hit_rate > 0 && " 🏆"}
                        </span>
                      </td>
                    ))}
                  </tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">MRR</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        <span className={cn(r.scores.mrr === bestScores.mrr && bestScores.mrr > 0 ? "font-bold text-amber-500" : "")}>
                          {r.scores.mrr.toFixed(3)}
                        </span>
                      </td>
                    ))}
                  </tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">NDCG@K</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        <span className={cn(r.scores.ndcg === bestScores.ndcg && bestScores.ndcg > 0 ? "font-bold text-amber-500" : "")}>
                          {r.scores.ndcg.toFixed(3)}
                        </span>
                      </td>
                    ))}
                  </tr>

                  {/* Generation metrics */}
                  <tr className="bg-muted/10"><td colSpan={selectedRunsData.length + 1} className="px-4 py-2 font-semibold">🧠 LLM Judge (Generation)</td></tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">Faithfulness (0-10)</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        {r.scores.faithfulness !== null ? (
                          <span className={cn(r.scores.faithfulness === bestScores.faithfulness && bestScores.faithfulness > 0 ? "font-bold text-emerald-500" : "")}>
                            {r.scores.faithfulness.toFixed(1)}
                          </span>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </td>
                    ))}
                  </tr>
                  <tr>
                    <td className="px-4 py-3 font-medium">Answer Relevancy (0-10)</td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-3">
                        {r.scores.answer_relevancy !== null ? r.scores.answer_relevancy.toFixed(1) : <span className="text-muted-foreground">-</span>}
                      </td>
                    ))}
                  </tr>

                  {/* Actions */}
                  <tr className="bg-muted/5 border-t">
                    <td className="px-4 py-4"></td>
                    {selectedRunsData.map((r) => (
                      <td key={r.id} className="px-4 py-4">
                        <div className="flex flex-col gap-2">
                          <Button size="sm" variant="outline" onClick={() => fetchDrillDown(r.id)}>
                            <FileJson className="w-4 h-4 mr-1.5" /> Drill Down
                          </Button>
                          <Button size="sm" onClick={() => deployConfig(r.id)}>
                            <Zap className="w-4 h-4 mr-1.5" /> Deploy
                          </Button>
                        </div>
                      </td>
                    ))}
                  </tr>
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Drill Down View */}
      {drillDownLoading && <div className="p-8 flex justify-center"><Loader2 className="h-8 w-8 animate-spin" /></div>}
      
      {drillDownData && !drillDownLoading && (
        <Card className="border-emerald-500/20 shadow-lg">
          <CardHeader className="bg-emerald-500/5">
            <CardTitle className="flex items-center gap-2">
              <Target className="h-5 w-5 text-emerald-500" />
              Per-Query Analysis 
              <span className="text-muted-foreground font-normal text-sm ml-2">
                ({drillDownData.run.name || "Run"})
              </span>
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div className="divide-y">
              {drillDownData.per_query.map((q, idx) => (
                <div key={idx} className="p-4 hover:bg-muted/30 transition-colors">
                  <div 
                    className="flex items-center justify-between cursor-pointer"
                    onClick={() => setExpandedQuery(expandedQuery === idx ? null : idx)}
                  >
                    <div className="flex items-center gap-3">
                      {q.hit ? <CheckCircle2 className="h-5 w-5 text-emerald-500" /> : <XCircle className="h-5 w-5 text-rose-500" />}
                      <span className="font-medium text-sm">{q.query}</span>
                      {q.faithfulness !== null && (
                        <Badge variant="outline" className={cn(
                          q.faithfulness >= 8 ? "border-emerald-500/50 text-emerald-500" : 
                          q.faithfulness >= 5 ? "border-amber-500/50 text-amber-500" : "border-rose-500/50 text-rose-500"
                        )}>
                          Faith: {q.faithfulness}/10
                        </Badge>
                      )}
                    </div>
                    <div className="flex items-center gap-4 text-sm text-muted-foreground">
                      <span>RR: {q.reciprocal_rank.toFixed(2)}</span>
                      <span>Rank: {q.matched_at_rank || '-'}</span>
                      <div className="flex gap-1">
                        {q.vector_contributed && <span className="bg-blue-500/20 text-blue-500 px-1.5 rounded text-xs">V</span>}
                        {q.tree_contributed && <span className="bg-emerald-500/20 text-emerald-500 px-1.5 rounded text-xs">T</span>}
                        {q.graph_contributed && <span className="bg-purple-500/20 text-purple-500 px-1.5 rounded text-xs">G</span>}
                      </div>
                      {expandedQuery === idx ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                    </div>
                  </div>
                  
                  {expandedQuery === idx && (
                    <div className="mt-4 pl-8 pr-4 grid grid-cols-1 lg:grid-cols-2 gap-6 animate-in slide-in-from-top-2">
                      <div className="space-y-4">
                        <div>
                          <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Top Context</h4>
                          <div className="space-y-2">
                            {q.top_results.slice(0, 3).map((tr, i) => (
                              <div key={i} className="text-sm bg-muted/50 p-2 rounded border flex items-center justify-between">
                                <span className="truncate">{tr.title}</span>
                                <Badge variant="secondary" className="text-[10px]">{tr.score.toFixed(3)}</Badge>
                              </div>
                            ))}
                          </div>
                        </div>
                      </div>
                      
                      {q.generated_answer && (
                        <div className="space-y-4">
                          <div>
                            <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Generated Answer</h4>
                            <div className="text-sm bg-background border p-3 rounded-md italic">
                              {q.generated_answer}
                            </div>
                          </div>
                          {q.judge_reasoning && (
                            <div>
                              <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Judge Reasoning</h4>
                              <p className="text-sm text-muted-foreground">{q.judge_reasoning}</p>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <Dialog open={autoTuneOpen} onOpenChange={setAutoTuneOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-purple-600">
              <Wand2 className="w-5 h-5" /> Sub-Agent Auto-Tuner
            </DialogTitle>
            <DialogDescription>
              Initialize an autonomous agent loop that analyzes failure modes and iteratively tunes parameters (Weights, Top K, Alpha) to maximize NDCG.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label>Optimization Iterations</Label>
              <Input type="number" min={1} max={10} value={tuneIterations} onChange={(e) => setTuneIterations(Number(e.target.value))} />
              <p className="text-[10px] text-muted-foreground">More iterations = better results but higher latency/cost.</p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAutoTuneOpen(false)}>Cancel</Button>
            <Button onClick={triggerAutoTune} className="bg-purple-600 hover:bg-purple-700 text-white">Start Tuning 🪄</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
