"use client";

import { useState, useCallback, useEffect, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Loader2, TrendingUp, Target, Zap, ChevronDown, ChevronRight, CheckCircle2, XCircle, FileJson, Wand2, ServerCog, Trash2 } from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip as RechartsTooltip, ResponsiveContainer } from "recharts";
import { Send } from "lucide-react";

// ── Types ──────────────────────────────────────────

interface BootstrapCI {
  mean: number;
  ci_lower: number;
  ci_upper: number;
}

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
  bootstrap_ci?: {
    hit_rate: BootstrapCI;
    mrr: BootstrapCI;
    ndcg: BootstrapCI;
    n_queries: number;
    n_resamples: number;
    confidence_level: number;
  };
  total_queries: number;
  eval_mode?: string;
  dataset_id?: string | null;
  dataset_name?: string | null;
  started_at: string;
  is_baseline?: boolean;
  regression_detected?: boolean;
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

export interface Agent {
  id: string;
  name: string;
  description: string;
}

export function RagEvalDashboard() {
  const [runs, setRuns] = useState<RagEvalRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedRunIds, setSelectedRunIds] = useState<string[]>([]);
  const [drillDownData, setDrillDownData] = useState<RunDetailResponse | null>(null);
  const [drillDownLoading, setDrillDownLoading] = useState(false);
  const [expandedQuery, setExpandedQuery] = useState<number | null>(null);
  const [drillDownFilter, setDrillDownFilter] = useState<"all" | "passed" | "failed">("all");

  const [autoTuneOpen, setAutoTuneOpen] = useState(false);
  const [tuneIterations, setTuneIterations] = useState(3);
  const [tunerProvider, setTunerProvider] = useState<string>("default");
  const [tunerModelId, setTunerModelId] = useState<string>("");
  const [tuningJobId, setTuningJobId] = useState<string | null>(null);
  const [tuningStatus, setTuningStatus] = useState<any>(null);
  
  const [tuneMaxTokens, setTuneMaxTokens] = useState<number | "">("");
  const [tuneMinAccuracy, setTuneMinAccuracy] = useState<number | "">("");
  const [tuneMaxLatency, setTuneMaxLatency] = useState<number | "">("");
  const [tuneMaxTokenBudget, setTuneMaxTokenBudget] = useState<number | "">("");

  const [diffModalOpen, setDiffModalOpen] = useState(false);
  const [diffData, setDiffData] = useState<any>(null);
  
  // Pagination state
  const [currentPage, setCurrentPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  
  // Inline notification (replaces alert())
  const [notification, setNotification] = useState<{type: 'success' | 'error'; message: string} | null>(null);
  
  const [availableModels, setAvailableModels] = useState<any[]>([]);
  
  // Tuning Chat state
  const [tuningChat, setTuningChat] = useState<{role: string; content: string}[]>([
    { role: "system", content: "Agent Overseer ready. Waiting for tuning job to begin." }
  ]);
  const [chatInput, setChatInput] = useState("");
  const [chatSending, setChatSending] = useState(false);

  // Filters and Actions
  const [datasetFilter, setDatasetFilter] = useState("all");

  // Deploy target state
  const [deployModalOpen, setDeployModalOpen] = useState(false);
  const [deployTargetId, setDeployTargetId] = useState<string>("");
  const [agentsList, setAgentsList] = useState<Agent[]>([]);
  const [deployingRunId, setDeployingRunId] = useState<string | null>(null);
  const [isDeploying, setIsDeploying] = useState(false);

  const apiOrigin = API_BASE_URL.replace(/\/api\/v1$/, "");

  // ── Filtered drill-down (T3.6) ──────────────────
  const filteredDrillDown = useMemo(() => {
    if (!drillDownData) return [];
    if (drillDownFilter === "all") return drillDownData.per_query;
    return drillDownData.per_query.filter(q => drillDownFilter === "passed" ? q.hit : !q.hit);
  }, [drillDownData, drillDownFilter]);

  const PER_PAGE = 20;

  const fetchRuns = useCallback(async (page = 1, append = false) => {
    try {
      if (!append) setLoading(true);
      else setLoadingMore(true);
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs?per_page=${PER_PAGE}&page=${page}`);
      if (resp.ok) {
        const data = await resp.json();
        const newRuns = data.runs || [];
        if (append) {
          setRuns(prev => [...prev, ...newRuns]);
        } else {
          setRuns(newRuns);
          if (newRuns.length > 0 && selectedRunIds.length === 0) {
            setSelectedRunIds([newRuns[0].id]);
          }
        }
        setHasMore(newRuns.length >= PER_PAGE);
        setCurrentPage(page);
      }
    } catch (e) {
      console.error("Failed to fetch eval runs", e);
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  }, [apiOrigin, selectedRunIds.length]);

  useEffect(() => {
    fetchRuns();
  }, [fetchRuns]);

  // Auto-dismiss notifications after 4s
  useEffect(() => {
    if (notification) {
      const timer = setTimeout(() => setNotification(null), 4000);
      return () => clearTimeout(timer);
    }
  }, [notification]);

  useEffect(() => {
    async function loadModels() {
      try {
        const resp = await authFetch(`${apiOrigin}/api/v1/models`);
        if (resp.ok) setAvailableModels(await resp.json());
      } catch(e) {}
    }
    loadModels();
  }, [apiOrigin]);

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

  const handleDeleteRun = async (runId: string) => {
    if (!confirm("Are you sure you want to delete this evaluation run? This action cannot be undone.")) return;
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${runId}`, {
        method: "DELETE"
      });
      if (resp.ok) {
        setDrillDownData(null);
        setSelectedRunIds((prev) => prev.filter((id) => id !== runId));
        fetchRuns();
      } else {
        setNotification({ type: 'error', message: 'Failed to delete evaluation run.' });
      }
    } catch (e) {
      console.error("Failed to delete run:", e);
      setNotification({ type: 'error', message: 'Error deleting run.' });
    }
  };

  const toggleRunSelection = (id: string) => {
    const runToAdd = runs.find(r => r.id === id);
    if (!runToAdd) return;

    setSelectedRunIds((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      
      // Enforce dataset isolation: you can only compare runs from the exact same dataset
      if (prev.length > 0) {
        const firstRun = runs.find(r => r.id === prev[0]);
        if (firstRun && firstRun.dataset_id !== runToAdd.dataset_id) {
          // If mismatch, clear the selection and start a new one with the newly clicked run
          return [id];
        }
      }

      if (prev.length >= 3) return [...prev.slice(1), id];
      return [...prev, id];
    });
  };

  const openDeployModal = async (runId: string) => {
    setDeployingRunId(runId);
    setDeployModalOpen(true);
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/agents`);
      if (resp.ok) {
        const data = await resp.json();
        setAgentsList(data);
        if (data.length > 0) setDeployTargetId(data[0].id);
      }
    } catch (e) {
      console.error("Failed to fetch agents", e);
    }
  };

  const submitDeployConfig = async () => {
    if (!deployingRunId || !deployTargetId) return;
    setIsDeploying(true);
    try {
      // 1. Get the payload for the RAG params
      const deployResp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${deployingRunId}/deploy`, { method: "POST" });
      if (!deployResp.ok) throw new Error("Failed to compute params from evaluation run.");
      const deployData = await deployResp.json();

      // 2. Fetch target agent
      const agentResp = await authFetch(`${apiOrigin}/api/v1/agents/${deployTargetId}`);
      if (!agentResp.ok) throw new Error("Failed to fetch target agent.");
      const agentData = await agentResp.json();

      // 3. Merge
      const newConfig = {
        ...agentData,
        rag_config: {
          ...agentData.rag_config,
          pipeline: deployData.rag_params,
          rerank: deployData.rerank_config,
        }
      };

      // 4. Save back
      const updateResp = await authFetch(`${apiOrigin}/api/v1/agents/${deployTargetId}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(newConfig)
      });

      if (updateResp.ok) {
        setNotification({ type: 'success', message: 'Configuration deployed successfully to agent!' });
        setDeployModalOpen(false);
      } else {
        throw new Error("Failed to save agent config");
      }
    } catch (e) {
      console.error("Deploy failed:", e);
      setNotification({ type: 'error', message: 'Deployment failed: ' + String(e) });
    } finally {
      setIsDeploying(false);
    }
  };

  const handleSendChat = async () => {
    if (!chatInput.trim() || !tuningJobId) return;
    const msg = chatInput.trim();
    setChatInput("");
    setTuningChat(curr => [...curr, { role: "user", content: msg }, { role: "system", content: "typing" }]);
    setChatSending(true);

    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/auto-tune/${tuningJobId}/chat`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ 
          message: msg,
          tuner_provider: tunerProvider !== "default" ? tunerProvider : undefined,
          tuner_model: tunerModelId.trim() ? tunerModelId.trim() : undefined
        })
      });
      if (resp.ok) {
        const data = await resp.json();
        setTuningChat(curr => {
          const newChat = [...curr];
          newChat[newChat.length - 1] = { role: "assistant", content: data.reply };
          return newChat;
        });
      } else {
        setTuningChat(curr => {
          const newChat = [...curr];
          newChat[newChat.length - 1] = { role: "assistant", content: "Error communicating with Overseer." };
          return newChat;
        });
      }
    } catch(e) {
      setTuningChat(curr => {
        const newChat = [...curr];
        newChat[newChat.length - 1] = { role: "assistant", content: "Network error." };
        return newChat;
      });
    } finally {
      setChatSending(false);
    }
  };

  const triggerAutoTune = async () => {
    if (selectedRunsData.length !== 1) return;
    const baseRun = selectedRunsData[0];
    if (!drillDownData || drillDownData.run.id !== baseRun.id) {
      setNotification({ type: 'error', message: 'Please drill down into the selected run first so we can extract its dataset.' });
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
          target_metric: "ndcg",
          tuner_provider: tunerProvider !== "default" ? tunerProvider : undefined,
          tuner_model: tunerModelId.trim() ? tunerModelId.trim() : undefined,
          max_token_budget: tuneMaxTokenBudget !== "" ? Number(tuneMaxTokenBudget) : undefined,
          min_accuracy: tuneMinAccuracy !== "" ? Number(tuneMinAccuracy) / 100 : undefined,
          max_latency: tuneMaxLatency !== "" ? Number(tuneMaxLatency) : undefined,
          max_tokens_per_run: tuneMaxTokens !== "" ? Number(tuneMaxTokens) : undefined,
        })
      });
      if (resp.ok) {
        const json = await resp.json();
        setTuningJobId(json.job_id);
        setAutoTuneOpen(false);
      }
    } catch (e) { console.error(e); }
  };

  const handleCompareRuns = async () => {
    if (selectedRunIds.length !== 2) return;
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/compare?ids=${selectedRunIds[0]},${selectedRunIds[1]}`);
      if (resp.ok) {
        setDiffData(await resp.json());
        setDiffModalOpen(true);
      }
    } catch (e) {
      setNotification({ type: "error", message: "Failed to load comparison data." });
    }
  };

  const setBaseline = async (runId: string) => {
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${runId}/set-baseline`, { method: "POST" });
      if (resp.ok) {
        setNotification({ type: "success", message: "Baseline set successfully!" });
        fetchRuns();
      } else {
        setNotification({ type: "error", message: "Failed to set baseline." });
      }
    } catch (e) {
      setNotification({ type: "error", message: "Network error setting baseline." });
    }
  };

  const filteredRuns = useMemo(() => {
    return runs.filter(r => datasetFilter === 'all' || (r.dataset_id || "inline") === datasetFilter);
  }, [runs, datasetFilter]);

  const selectedRunsData = useMemo(() => {
    return selectedRunIds.map((id) => runs.find((r) => r.id === id)).filter(Boolean) as RagEvalRun[];
  }, [selectedRunIds, runs]);

  if (loading && !runs.length) {
    return (
      <div className="space-y-6">
        <Card>
          <CardHeader>
            <Skeleton className="h-6 w-48" />
            <Skeleton className="h-4 w-72 mt-2" />
          </CardHeader>
          <CardContent>
            <div className="flex gap-2 mb-6">
              {[1,2,3,4].map(i => <Skeleton key={i} className="h-6 w-24 rounded-full" />)}
            </div>
            <div className="border rounded-xl overflow-hidden">
              <div className="bg-muted/50 px-4 py-3 flex gap-16">
                <Skeleton className="h-4 w-16" />
                <Skeleton className="h-4 w-20" />
                <Skeleton className="h-4 w-20" />
              </div>
              {[1,2,3,4,5].map(i => (
                <div key={i} className="px-4 py-3 flex gap-16 border-t">
                  <Skeleton className="h-4 w-24" />
                  <Skeleton className="h-4 w-16" />
                  <Skeleton className="h-4 w-16" />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><Skeleton className="h-6 w-36" /></CardHeader>
          <CardContent>
            {[1,2,3].map(i => (
              <div key={i} className="flex items-center gap-3 py-3 border-b last:border-0">
                <Skeleton className="h-5 w-5 rounded-full" />
                <Skeleton className="h-4 w-48" />
                <Skeleton className="h-4 w-12 ml-auto" />
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    );
  }

  // ── Empty State ──
  if (!loading && runs.length === 0) {
    return (
      <div className="space-y-6">
        <Card className="border-dashed border-2">
          <CardContent className="flex flex-col items-center gap-4 p-12">
            <div className="p-4 rounded-full bg-gradient-to-br from-purple-500/10 to-indigo-500/10">
              <Target className="h-12 w-12 text-purple-500/70" />
            </div>
            <h3 className="text-lg font-semibold">No Evaluation Runs Yet</h3>
            <p className="text-sm text-muted-foreground text-center max-w-md">
              Start by generating an evaluation dataset from the RAG Playground,
              then run your first evaluation benchmark to see detailed accuracy metrics here.
            </p>
            <div className="flex gap-2">
              <Badge variant="outline" className="px-3 py-1">
                <TrendingUp className="w-3 h-3 mr-1" /> Hit Rate
              </Badge>
              <Badge variant="outline" className="px-3 py-1">
                <Target className="w-3 h-3 mr-1" /> NDCG
              </Badge>
              <Badge variant="outline" className="px-3 py-1">
                <Zap className="w-3 h-3 mr-1" /> MRR
              </Badge>
            </div>
          </CardContent>
        </Card>
      </div>
    );
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
      {/* Inline notification toast */}
      {notification && (
        <div className={cn(
          "flex items-center gap-2 px-4 py-3 rounded-lg text-sm font-medium transition-all animate-in fade-in slide-in-from-top-2",
          notification.type === 'success' ? "bg-green-500/10 text-green-600 border border-green-500/20" : "bg-red-500/10 text-red-600 border border-red-500/20"
        )}>
          {notification.type === 'success' ? <CheckCircle2 className="w-4 h-4 shrink-0" /> : <XCircle className="w-4 h-4 shrink-0" />}
          {notification.message}
          <button className="ml-auto text-xs opacity-60 hover:opacity-100" onClick={() => setNotification(null)}>✕</button>
        </div>
      )}
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
            <div className="flex gap-2">
              <Button 
                variant="outline"
                className="gap-2"
                disabled={selectedRunIds.length !== 2}
                onClick={handleCompareRuns}
                title="Select exactly 2 runs to compare differentials"
              >
                Diff A vs B
              </Button>
              <Button 
                variant="secondary"
                className="gap-2 bg-gradient-to-r from-purple-500/10 to-indigo-500/10 border-purple-500/20 text-purple-600 hover:bg-purple-500/20 disabled:opacity-50 disabled:cursor-not-allowed"
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
            </div>
          )}
        </CardHeader>
        <CardContent>
          {datasetFilter !== "all" && runs.some(r => r.dataset_id === datasetFilter) && (
            <div className="h-[150px] w-full mb-6 border-b pb-4">
              <ResponsiveContainer width="100%" height="100%">
                 <LineChart data={
                   [...runs.filter(r => r.dataset_id === datasetFilter)].sort((a,b) => new Date(a.started_at).getTime() - new Date(b.started_at).getTime())
                 }>
                   <CartesianGrid strokeDasharray="3 3" vertical={false} opacity={0.3} />
                   <XAxis dataKey="name" tick={{fontSize: 10}} tickFormatter={(val) => val ? val.substring(0, 10) : ''} />
                   <YAxis tick={{fontSize: 10}} domain={['auto', 'auto']} width={40} />
                   <RechartsTooltip />
                   <Line type="monotone" name="Hit Rate" dataKey={(d) => d.scores.hit_rate} stroke="#10b981" strokeWidth={2} />
                   <Line type="monotone" name="NDCG" dataKey={(d) => d.scores.ndcg} stroke="#6366f1" strokeWidth={2} />
                 </LineChart>
              </ResponsiveContainer>
            </div>
          )}
          <div className="flex items-center gap-4 mb-4">
            <Label className="text-sm shrink-0">Filter by Dataset:</Label>
            <select
              className="flex h-9 w-[300px] rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              value={datasetFilter}
              onChange={(e) => {
                setDatasetFilter(e.target.value);
                // We must clear selected runs when filter changes to prevent cross-dataset comparisons
                setSelectedRunIds([]);
              }}
            >
              <option value="all">All Datasets</option>
              <option value="inline">Custom/Inline Data</option>
              {Array.from(new Set(runs.map(r => r.dataset_id).filter(Boolean))).map(dsId => {
                const name = runs.find(r => r.dataset_id === dsId)?.dataset_name || dsId;
                return <option key={dsId} value={dsId!}>{name}</option>;
              })}
            </select>
          </div>

          <div className="flex gap-2 mb-6 overflow-x-auto pb-2 flex-wrap">
            {filteredRuns.map((r) => (
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
            {hasMore && (
              <Button
                variant="ghost"
                size="sm"
                className="text-xs text-muted-foreground hover:text-foreground"
                disabled={loadingMore}
                onClick={() => fetchRuns(currentPage + 1, true)}
              >
                {loadingMore ? <Loader2 className="h-3 w-3 animate-spin mr-1" /> : null}
                Load More...
              </Button>
            )}
          </div>

          {selectedRunsData.length > 0 && (
            <div className="overflow-x-auto overflow-y-auto max-h-[520px] border rounded-xl relative">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted/50 text-muted-foreground border-b uppercase text-xs sticky top-0 z-10 backdrop-blur-sm">
                  <tr>
                    <th className="px-4 py-3 font-medium">Metric</th>
                    {selectedRunsData.map((r) => (
                      <th key={r.id} className="px-4 py-3 font-medium">
                        <div className="flex flex-col">
                          <span className="truncate max-w-[150px] text-foreground flex items-center gap-1">
                            {r.is_baseline && <span className="text-amber-500" title="Baseline Dataset Model">⭐</span>}
                            {r.regression_detected && <span className="text-rose-500" title="Regression Detected">⚠️</span>}
                            {r.name || "Unnamed"}
                          </span>
                          <span className="text-[10px] font-normal opacity-70">
                            {new Date(r.started_at).toLocaleDateString()}
                          </span>
                          {!r.is_baseline && (
                            <Button size="sm" variant="ghost" className="h-6 mt-1 text-[10px] bg-amber-500/10 text-amber-600 hover:bg-amber-500/20" onClick={() => setBaseline(r.id)}>
                              Set as Baseline
                            </Button>
                          )}
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
                  <tr>
                    <td className="px-4 py-3 font-medium">Dataset</td>
                    {selectedRunsData.map((r) => <td key={r.id} className="px-4 py-3">{r.dataset_name || "Custom/Inline"}</td>)}
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

                  {/* Bootstrap CI row — shown when drill-down data has CI info */}
                  {drillDownData?.run?.bootstrap_ci && selectedRunsData.some(r => r.id === drillDownData?.run?.id) && (
                    <>
                      <tr className="bg-muted/10"><td colSpan={selectedRunsData.length + 1} className="px-4 py-2 font-semibold">📊 95% Confidence Intervals (Bootstrap)</td></tr>
                      {['hit_rate', 'mrr', 'ndcg'].map(metric => {
                        const ci = (drillDownData.run as any).bootstrap_ci?.[metric];
                        if (!ci) return null;
                        return (
                          <tr key={`ci-${metric}`}>
                            <td className="px-4 py-3 font-medium">{metric === 'hit_rate' ? 'Hit Rate CI' : metric === 'mrr' ? 'MRR CI' : 'NDCG CI'}</td>
                            {selectedRunsData.map(r => (
                              <td key={r.id} className="px-4 py-3">
                                {r.id === drillDownData.run.id ? (
                                  <span className="text-xs font-mono bg-muted/80 px-2 py-1 rounded">
                                    [{(ci.ci_lower * 100).toFixed(1)}%, {(ci.ci_upper * 100).toFixed(1)}%]
                                  </span>
                                ) : (
                                  <span className="text-muted-foreground text-xs">drill down to see</span>
                                )}
                              </td>
                            ))}
                          </tr>
                        );
                      })}
                    </>
                  )}

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
                          <Button size="sm" onClick={() => openDeployModal(r.id)}>
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
          <CardHeader className="bg-emerald-500/5 flex flex-row items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Target className="h-5 w-5 text-emerald-500" />
              Per-Query Analysis 
              <span className="text-muted-foreground font-normal text-sm ml-2">
                ({drillDownData.run.name || "Run"})
              </span>
            </CardTitle>
            <div className="flex items-center gap-4">
              <div className="flex bg-muted rounded-md p-1">
                <button className={`px-3 py-1 text-xs rounded-md ${drillDownFilter === 'all' ? 'bg-background shadow-sm' : ''}`} onClick={() => { setDrillDownFilter('all'); setExpandedQuery(null); }}>All</button>
                <button className={`px-3 py-1 text-xs rounded-md ${drillDownFilter === 'passed' ? 'bg-background shadow-sm text-emerald-600' : ''}`} onClick={() => { setDrillDownFilter('passed'); setExpandedQuery(null); }}>Passed</button>
                <button className={`px-3 py-1 text-xs rounded-md ${drillDownFilter === 'failed' ? 'bg-background shadow-sm text-rose-600' : ''}`} onClick={() => { setDrillDownFilter('failed'); setExpandedQuery(null); }}>Failed</button>
              </div>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={() => window.open(`${API_BASE_URL.replace("/api/v1", "")}/api/v1/rag-eval/runs/${drillDownData.run.id}/export?format=csv`)}>CSV</Button>
                <Button variant="outline" size="sm" onClick={() => window.open(`${API_BASE_URL.replace("/api/v1", "")}/api/v1/rag-eval/runs/${drillDownData.run.id}/export?format=json`)}>JSON</Button>
              </div>
              <Button
                variant="ghost"
                size="icon"
                className="text-rose-500 hover:text-rose-600 hover:bg-rose-50"
                onClick={() => handleDeleteRun(drillDownData.run.id)}
                disabled={drillDownLoading}
                title="Delete Evaluation Run"
              >
                <Trash2 className="w-5 h-5" />
              </Button>
            </div>
          </CardHeader>
          <CardContent className="p-0">
            <div className="divide-y">
              {filteredDrillDown.map((q, idx) => (
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
                      {/* Left: Retrieval Scores */}
                      <div className="space-y-4 border-r pr-4">
                        <div>
                          <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Retrieval Scores</h4>
                          <div className="grid grid-cols-2 gap-2 text-sm">
                            <div className="bg-muted/50 p-2 rounded">
                              <span className="text-muted-foreground">NDCG:</span> <span className="font-mono font-medium">{q.ndcg_score.toFixed(3)}</span>
                            </div>
                            <div className="bg-muted/50 p-2 rounded">
                              <span className="text-muted-foreground">MRR:</span> <span className="font-mono font-medium">{q.reciprocal_rank.toFixed(3)}</span>
                            </div>
                            <div className="bg-muted/50 p-2 rounded">
                              <span className="text-muted-foreground">Precision:</span> <span className="font-mono font-medium">{q.precision.toFixed(3)}</span>
                            </div>
                            <div className="bg-muted/50 p-2 rounded">
                              <span className="text-muted-foreground">Recall:</span> <span className="font-mono font-medium">{q.recall.toFixed(3)}</span>
                            </div>
                          </div>
                        </div>
                        <div>
                          <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Retrieved Context (Top 3)</h4>
                          <div className="space-y-2">
                            {q.top_results.slice(0, 3).map((tr, i) => (
                              <div key={i} className="text-sm bg-muted/50 p-2 rounded border flex items-center justify-between overflow-hidden">
                                <span className="truncate mr-2">{tr.title}</span>
                                <Badge variant="secondary" className="shrink-0 text-[10px]">{tr.score.toFixed(3)}</Badge>
                              </div>
                            ))}
                          </div>
                        </div>
                        <div className="text-xs text-muted-foreground">
                          Latency: <span className="font-mono">{q.total_latency_ms}ms</span>
                        </div>
                      </div>
                      
                      {/* Right: Generated & Context */}
                      <div className="space-y-4">
                        <div>
                          <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Generated Answer</h4>
                          <div className={`text-sm bg-background border p-3 rounded-md italic ${q.hit ? 'border-emerald-500/30' : 'border-rose-500/30'}`}>
                            {q.generated_answer || "N/A"}
                          </div>
                        </div>
                        {q.judge_reasoning && (
                          <div>
                            <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Judge Reasoning</h4>
                            <p className="text-sm text-muted-foreground">{q.judge_reasoning}</p>
                          </div>
                        )}
                        {(q.faithfulness !== null || q.answer_relevancy !== null || q.context_precision !== null) && (
                          <div>
                            <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Generation Quality</h4>
                            <div className="grid grid-cols-3 gap-2 text-sm">
                              {q.faithfulness !== null && (
                                <div className="bg-muted/50 p-2 rounded text-center">
                                  <div className="text-muted-foreground text-[10px]">Faithfulness</div>
                                  <div className={`font-mono font-bold ${q.faithfulness >= 8 ? 'text-emerald-500' : q.faithfulness >= 5 ? 'text-amber-500' : 'text-rose-500'}`}>{q.faithfulness}/10</div>
                                </div>
                              )}
                              {q.answer_relevancy !== null && (
                                <div className="bg-muted/50 p-2 rounded text-center">
                                  <div className="text-muted-foreground text-[10px]">Relevancy</div>
                                  <div className={`font-mono font-bold ${q.answer_relevancy >= 8 ? 'text-emerald-500' : q.answer_relevancy >= 5 ? 'text-amber-500' : 'text-rose-500'}`}>{q.answer_relevancy}/10</div>
                                </div>
                              )}
                              {q.context_precision !== null && (
                                <div className="bg-muted/50 p-2 rounded text-center">
                                  <div className="text-muted-foreground text-[10px]">Ctx Precision</div>
                                  <div className="font-mono font-bold">{q.context_precision.toFixed(2)}</div>
                                </div>
                              )}
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Auto Tuner Sheet */}
      <Sheet open={autoTuneOpen} onOpenChange={setAutoTuneOpen}>
        <SheetContent className="sm:max-w-xl flex flex-col h-full p-4 z-[100] gap-0">
          <SheetHeader className="pb-4">
            <SheetTitle className="flex items-center gap-2 text-purple-600">
              <Wand2 className="w-5 h-5" /> Sub-Agent Auto-Tuner
            </SheetTitle>
            <SheetDescription>
              Initialize an autonomous agent loop that analyzes failure modes and iteratively tunes parameters.
            </SheetDescription>
          </SheetHeader>
          <div className="flex-1 flex flex-col gap-4 overflow-y-auto pr-2 pb-4">
            
            {/* Tuning Active View (Graph) */}
            {(tuningJobId || tuningStatus) ? (
               <div className="space-y-4 border rounded-md p-4 bg-muted/20 flex-shrink-0">
                  <div className="flex items-center justify-between">
                    <div className="font-semibold text-sm flex items-center gap-2">
                      {tuningStatus?.status !== "completed" && <span className="animate-spin rounded-full h-3 w-3 border-b-2 border-purple-600"></span>}
                      {tuningStatus?.status === "completed" ? "✅ Tuning Complete!" : "Tuning in Progress..."}
                    </div>
                    <div className="text-xs text-muted-foreground border bg-background px-2 py-1 rounded">
                      Iteration: {tuningStatus?.current_iteration || 0} / {tuningStatus?.iterations || tuneIterations}
                    </div>
                  </div>

                  <div className="h-[180px] w-full bg-background rounded border p-2">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={runs
                        .filter(r => tuningJobId && r.name && r.name.includes(tuningJobId.substring(0, 8)))
                        .sort((a,b) => new Date(a.started_at).getTime() - new Date(b.started_at).getTime())
                        .map((r, i) => ({ iteration: i+1, ndcg: r.scores.ndcg }))
                      }>
                        <CartesianGrid strokeDasharray="3 3" vertical={false} opacity={0.5} />
                        <XAxis dataKey="iteration" tick={{fontSize: 12}} tickLine={false} />
                        <YAxis domain={['auto', 'auto']} tick={{fontSize: 12}} width={40} tickLine={false} />
                        <RechartsTooltip contentStyle={{fontSize: "12px", borderRadius: "8px"}} />
                        <Line type="monotone" dataKey="ndcg" stroke="#9333ea" strokeWidth={2} dot={{r: 4}} activeDot={{r: 6}} />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
               </div>
            ) : (
                <div className="space-y-4 border rounded-md p-4 mb-4 flex-shrink-0">
                  <h4 className="text-sm font-medium mb-2">Tuning Options</h4>
                  <div className="grid grid-cols-2 gap-4 flex-col overflow-y-auto max-h-[300px] p-1">
                    <div className="space-y-2">
                      <Label className="text-xs">Iterations</Label>
                      <Input type="number" min={1} max={10} value={tuneIterations} onChange={e => setTuneIterations(Number(e.target.value))} />
                      <p className="text-[10px] text-muted-foreground">Max evolution cycles.</p>
                    </div>
                    <div className="space-y-2">
                      <Label className="text-xs">Max Run Tokens</Label>
                      <Input type="number" placeholder="No limit" value={tuneMaxTokens} onChange={e => setTuneMaxTokens(e.target.value ? Number(e.target.value) : "")} />
                      <p className="text-[10px] text-muted-foreground">Tokens per eval.</p>
                    </div>
                    <div className="space-y-2">
                      <Label className="text-xs">Max Budget Limit (Tokens)</Label>
                      <Input type="number" placeholder="Total tuner budget" value={tuneMaxTokenBudget} onChange={e => setTuneMaxTokenBudget(e.target.value ? Number(e.target.value) : "")} />
                      <p className="text-[10px] text-muted-foreground">Hard cap across loop.</p>
                    </div>
                    <div className="space-y-2">
                      <Label className="text-xs">Min Target Accuracy (%)</Label>
                      <Input type="number" placeholder="80" value={tuneMinAccuracy} onChange={e => setTuneMinAccuracy(e.target.value ? Number(e.target.value) : "")} />
                    </div>
                    <div className="space-y-2">
                      <Label className="text-xs">Max Allowed Latency (ms)</Label>
                      <Input type="number" placeholder="1000" value={tuneMaxLatency} onChange={e => setTuneMaxLatency(e.target.value ? Number(e.target.value) : "")} />
                    </div>
                  </div>
                  <div className="p-3 bg-muted/30 rounded border border-purple-500/20 text-xs mt-4">
                    <strong>Cost Estimate: </strong> 
                    {drillDownData 
                      ? (() => {
                          const r = drillDownData.run;
                          const avgTokens = Math.round(((r as any).total_prompt_tokens + (r as any).total_completion_tokens) / (r.total_queries || 1));
                          const estimatedTokens = avgTokens * r.total_queries * tuneIterations;
                          return `~${estimatedTokens.toLocaleString()} tokens total evaluated (approx ${avgTokens} per query)`;
                        })()
                      : "Calculating..."}
                  </div>
                  
                  <div className="grid grid-cols-2 gap-3 pt-2 border-t">
                    <div className="space-y-1">
                      <Label className="text-xs">Overseer Provider</Label>
                      <Select value={tunerProvider} onValueChange={(val) => { setTunerProvider(val); setTunerModelId(""); }}>
                        <SelectTrigger className="text-xs h-8">
                          <SelectValue placeholder="Provider" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="default">Tenant Default</SelectItem>
                          {Array.from(new Set(availableModels.map(m => m.provider))).map(p => (
                             <SelectItem key={p} value={p}>{p.toUpperCase()}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    
                    <div className="space-y-1">
                      <Label className="text-xs">Model ID</Label>
                      {tunerProvider === "default" ? (
                        <Input 
                          placeholder="Default Model" 
                          value={tunerModelId}
                          onChange={(e) => setTunerModelId(e.target.value)}
                          className="text-xs h-8"
                        />
                      ) : (
                        <Select value={tunerModelId || "default"} onValueChange={(val) => setTunerModelId(val === "default" ? "" : val)}>
                          <SelectTrigger className="text-xs h-8">
                            <SelectValue placeholder="Model Check..." />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="default">Default for Provider</SelectItem>
                            {availableModels.filter(m => m.provider === tunerProvider).map(m => (
                              <SelectItem key={m.model_id} value={m.model_id}>{m.model_id.split('/').pop() || m.model_id}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    </div>
                  </div>
                </div>
            )}

            {/* Chat View */}
            <div className="flex-1 flex flex-col border rounded-md min-h-[300px]">
               <div className="flex-1 overflow-y-auto p-4 space-y-4 bg-muted/10">
                 {tuningChat.map((msg, i) => (
                   <div key={i} className={cn("text-sm max-w-[85%] rounded-md p-3", msg.role === "user" ? "ml-auto bg-purple-600 text-white" : msg.role === "system" && msg.content === "typing" ? "mr-auto bg-muted italic text-muted-foreground" : "mr-auto bg-background border")}>
                     {msg.role === "system" ? (msg.content === "Agent Overseer ready. Waiting for tuning job to begin." ? msg.content : "Agent is typing...") : msg.content}
                   </div>
                 ))}
               </div>
               <div className="p-3 bg-background border-t">
                 <form onSubmit={(e) => { e.preventDefault(); handleSendChat(); }} className="flex gap-2">
                   <Input 
                     className="flex-1" 
                     placeholder="Ask the Overseer agent..." 
                     value={chatInput} 
                     onChange={(e) => setChatInput(e.target.value)} 
                     disabled={chatSending || !tuningJobId} 
                   />
                   <Button size="icon" type="submit" disabled={chatSending || !chatInput.trim() || !tuningJobId} className="bg-purple-600 hover:bg-purple-700">
                     <Send className="w-4 h-4 text-white" />
                   </Button>
                 </form>
               </div>
            </div>

          </div>
          <div className="mt-auto border-t pt-4 flex gap-2 justify-end">
            <Button variant="outline" onClick={() => setAutoTuneOpen(false)}>Close</Button>
            {(!tuningJobId && !tuningStatus) && (
              <Button onClick={triggerAutoTune} className="bg-purple-600 hover:bg-purple-700 text-white">Start Tuning 🪄</Button>
            )}
          </div>
        </SheetContent>
      </Sheet>

      {/* Deploy Agent Dialog */}
      <Dialog open={deployModalOpen} onOpenChange={setDeployModalOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <ServerCog className="w-5 h-5 text-blue-600" /> Deploy Configuration
            </DialogTitle>
            <DialogDescription>
              Select an agent to patch with these RAG evaluation parameters. This will directly modify the active agent's configuration!
            </DialogDescription>
          </DialogHeader>
          <div className="py-4 space-y-4">
             <div className="space-y-2">
                <Label>Select Target Agent</Label>
                <select 
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
                  value={deployTargetId}
                  onChange={(e) => setDeployTargetId(e.target.value)}
                >
                  <option value="" disabled>-- Select an Agent --</option>
                  {agentsList.map(a => (
                    <option key={a.id} value={a.id}>{a.name} ({a.id.substring(0, 8)})</option>
                  ))}
                </select>
             </div>
             <p className="text-sm text-yellow-600 bg-yellow-50 p-2 rounded border border-yellow-200">
               ℹ️ Note: Deploying this will instantly overwrite the agent's RAG weights (Vector/Tree/Graph) and Search properties (Top K, Rerank strategies) with the evaluated optimal versions.
             </p>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeployModalOpen(false)}>Cancel</Button>
            <Button onClick={submitDeployConfig} disabled={!deployTargetId || isDeploying} className="bg-blue-600 hover:bg-blue-700 text-white">
              {isDeploying ? "Deploying..." : "Confirm & Deploy"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {/* Diff Comparison Modal */}
      <Dialog open={diffModalOpen} onOpenChange={setDiffModalOpen}>
         <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col p-6">
            <DialogHeader>
               <DialogTitle>Version Differential (A vs B)</DialogTitle>
               <DialogDescription>Comparing Hit Rate query changes between the two selected runs.</DialogDescription>
            </DialogHeader>
            <div className="overflow-y-auto flex-1 pr-2 space-y-6">
              {diffData ? (
                 <>
                   <div>
                     <h4 className="flex items-center gap-2 font-medium text-emerald-500 mb-2">
                       <CheckCircle2 className="w-4 h-4" /> Improvements ({diffData.improvements?.length || 0})
                     </h4>
                     {diffData.improvements?.length > 0 ? (
                       <div className="space-y-2">
                          {diffData.improvements.map((q: any, i: number) => (
                            <div key={i} className="text-sm p-3 bg-emerald-500/10 border border-emerald-500/20 rounded">
                              <p className="font-medium">"{q.query}"</p>
                              <p className="text-xs text-muted-foreground mt-1">NDCG: {q.previous_ndcg.toFixed(2)} → {q.new_ndcg.toFixed(2)}</p>
                            </div>
                          ))}
                       </div>
                     ) : <p className="text-sm text-muted-foreground italic">No unique improvements found.</p>}
                   </div>

                   <div>
                     <h4 className="flex items-center gap-2 font-medium text-rose-500 mb-2">
                       <XCircle className="w-4 h-4" /> Regressions ({diffData.regressions?.length || 0})
                     </h4>
                     {diffData.regressions?.length > 0 ? (
                       <div className="space-y-2">
                          {diffData.regressions.map((q: any, i: number) => (
                            <div key={i} className="text-sm p-3 bg-rose-500/10 border border-rose-500/20 rounded">
                              <p className="font-medium">"{q.query}"</p>
                              <p className="text-xs text-muted-foreground mt-1">NDCG: {q.previous_ndcg.toFixed(2)} → {q.new_ndcg.toFixed(2)}</p>
                            </div>
                          ))}
                       </div>
                     ) : <p className="text-sm text-muted-foreground italic">No queries regressed.</p>}
                   </div>
                 </>
              ) : (
                 <div className="flex justify-center p-8"><Loader2 className="w-8 h-8 animate-spin text-muted-foreground" /></div>
              )}
            </div>
         </DialogContent>
      </Dialog>
    </div>
  );
}
