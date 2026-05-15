"use client";

import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SourceBadge, SourceLegend } from "@/components/ui/source-badge";
import { WeightSlider } from "@/components/ui/weight-slider";
import { GraphStatus } from "@/components/ui/graph-status";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import {
  Send, Loader2, Search, Sparkles, Database, TreePine, Share2,
  BarChart3, Wand2, Zap, Target, TrendingUp, CheckCircle2, XCircle,
  Clock, FlaskConical, ChevronDown, ChevronUp, FileJson, Play,
  Activity, Save
} from "lucide-react";
import Cookies from "js-cookie";
import { useRouter } from "next/navigation";
import { authFetch, API_BASE_URL, fetchTenantConfig, updateTenantConfig } from "@/lib/api";
import { RagEvalDashboard } from "@/components/evaluations/rag-eval-dashboard";
import { PipelineVisualizer } from "@/components/ui/pipeline-visualizer";

// ── Types ──────────────────────────────────────────

interface RetrievalResult {
  content: string;
  title: string;
  score: number;
  source_type: "vector" | "tree" | "graph";
  metadata?: Record<string, any>;
}

interface SearchResponse {
  results: RetrievalResult[];
  distribution: { vector: number; tree: number; graph: number; total: number };
  weights_used: { vector: number; tree: number; graph: number };
  mode_used: string;
  latency_ms: number;
  query: string;
  synthesis?: string;
  trace_log?: any[];
}

interface QuerySuggestion {
  query: string;
  strategy: string;
  explanation: string;
  confidence: number;
}

interface OptimizeResponse {
  original_query: string;
  suggestions: QuerySuggestion[];
  latency_ms: number;
  model_used: string;
}

interface BenchmarkItem {
  query: string;
  expected_titles: string[];
  expected_content?: string;
}

interface QueryBenchmarkResult {
  query: string;
  hit: boolean;
  reciprocal_rank: number;
  latency_ms: number;
  top_results: string[];
  matched_at_rank: number | null;
}

interface EvalRunResponse {
  run_id: string;
  hit_rate: number;
  mrr: number;
  ndcg: number;
  map: number;
}

// ── Strategy Badge Colors ──────────────────────────

const STRATEGY_COLORS: Record<string, string> = {
  keyword_expansion: "bg-blue-500/15 text-blue-400 border-blue-500/25",
  synonym: "bg-amber-500/15 text-amber-400 border-amber-500/25",
  decomposition: "bg-emerald-500/15 text-emerald-400 border-emerald-500/25",
  semantic_rephrase: "bg-purple-500/15 text-purple-400 border-purple-500/25",
  original: "bg-zinc-500/15 text-zinc-400 border-zinc-500/25",
};

const STRATEGY_ICONS: Record<string, React.ReactNode> = {
  keyword_expansion: <Zap className="h-3 w-3" />,
  synonym: <Wand2 className="h-3 w-3" />,
  decomposition: <Target className="h-3 w-3" />,
  semantic_rephrase: <Sparkles className="h-3 w-3" />,
  original: <Search className="h-3 w-3" />,
};

// ── Main Page ──────────────────────────────────────

export default function RAGPlaygroundPage() {
  // Search state
  const [question, setQuestion] = useState("");
  const [mode, setMode] = useState<string>("hybrid");
  const [loading, setLoading] = useState(false);
  const sessionIdRef = useRef<string | null>(null);
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [weights, setWeights] = useState({ vector: 0.5, tree: 0.3, graph: 0.2 });
  const [rerankStrategy, setRerankStrategy] = useState<string>("none");
  const [generateLLM, setGenerateLLM] = useState<boolean>(false);
  const [enableTrace, setEnableTrace] = useState<boolean>(true);
  const [searchHistory, setSearchHistory] = useState<{ question: string; mode: string; resultCount: number }[]>([]);
  const [hopLimit, setHopLimit] = useState(2);
  const [alpha, setAlpha] = useState(0.7);
  const [threshold, setThreshold] = useState(0.0);

  // Optimizer state
  const [optimizing, setOptimizing] = useState(false);
  const [suggestions, setSuggestions] = useState<QuerySuggestion[]>([]);
  const [optimizeModel, setOptimizeModel] = useState("");

  // Context
  const router = useRouter();

  // Search Configuration State
  const [isSavingConfig, setIsSavingConfig] = useState(false);

  // Benchmark state
  const [activeTab, setActiveTab] = useState<"search" | "benchmark" | "evaluation">("search");
  const [benchmarkItems, setBenchmarkItems] = useState<string>("");
  const [benchmarkLoading, setBenchmarkLoading] = useState(false);
  const [benchmarkResults, setBenchmarkResults] = useState<EvalRunResponse | null>(null);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  const [benchmarkLabel, setBenchmarkLabel] = useState("");
  const [evaluateGeneration, setEvaluateGeneration] = useState(false);

  // Wizard state
  const [wizardOpen, setWizardOpen] = useState(false);
  
  // Generate Set Modal state
  const [genModalOpen, setGenModalOpen] = useState(false);
  const [genPrompt, setGenPrompt] = useState("");
  const [genCount, setGenCount] = useState(5);
  const [genMultiTurn, setGenMultiTurn] = useState(false);
  const [genProvider, setGenProvider] = useState<string>("default");
  const [genModelId, setGenModelId] = useState<string>("");
  const [isGenerating, setIsGenerating] = useState(false);
  const [availableModels, setAvailableModels] = useState<any[]>([]);

  // Analytics / Datasets
  const [datasets, setDatasets] = useState<any[]>([]);
  const [activeDatasetId, setActiveDatasetId] = useState<string>("none");
  const [saveModalOpen, setSaveModalOpen] = useState(false);
  const [saveDatasetName, setSaveDatasetName] = useState("");
  const [saveDatasetDifficulty, setSaveDatasetDifficulty] = useState<string>("mixed");
  const [saveDatasetQuestionType, setSaveDatasetQuestionType] = useState<string>("mixed");

  // LLM Overrides
  const [searchProvider, setSearchProvider] = useState<string>("default");
  const [searchModelId, setSearchModelId] = useState<string>("");
  const [evalProvider, setEvalProvider] = useState<string>("default");
  const [evalModelId, setEvalModelId] = useState<string>("");

  // UI Feedback State
  const [applyMessage, setApplyMessage] = useState<{type: "success" | "error", text: string} | null>(null);

  const apiOrigin = API_BASE_URL.replace(/\/api\/v1$/, "");

  const fetchDatasets = useCallback(async () => {
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/datasets`);
      if (resp.ok) {
        const data = await resp.json();
        setDatasets(data.datasets || []);
      }
    } catch(err) {
      console.error(err);
    }
  }, [apiOrigin]);

  useEffect(() => {
    fetchDatasets();
  }, [fetchDatasets]);

  useEffect(() => {
    authFetch(`${apiOrigin}/api/v1/models`)
      .then(r => r.ok ? r.json() : [])
      .then(data => setAvailableModels(Array.isArray(data) ? data : []))
      .catch(console.error);
  }, [apiOrigin]);

  const handleConfigChange = useCallback((key: string, value: any) => {
    switch (key) {
      case 'weights': setWeights(value); break;
      case 'mode': setMode(value); break;
      case 'searchProvider': setSearchProvider(value); setSearchModelId(''); break;
      case 'searchModelId': setSearchModelId(value); break;
      case 'rerankStrategy': setRerankStrategy(value); break;
      case 'hopLimit': setHopLimit(value); break;
      case 'alpha': setAlpha(value); break;
      case 'threshold': setThreshold(value); break;
      case 'generateLLM': setGenerateLLM(value); break;
    }
  }, []);

  // ── Search Handler ──────────────────────────────

  const handleSearch = useCallback(async () => {
    if (!question.trim() || loading) return;
    setLoading(true);
    setError(null);
    setSuggestions([]);

    try {
      const body: Record<string, any> = {
        query: question.trim(),
        weights: mode === "hybrid" ? weights : undefined,
        limit: 10,
        synthesize: generateLLM,
        provider: searchProvider !== "default" ? searchProvider.toLowerCase() : undefined,
        model: searchModelId || undefined,
        trace: enableTrace,
        alpha,
        threshold,
        hop_limit: hopLimit,
      };
      // Single-source mode: only enable that source
      if (mode !== "hybrid") {
        body.sources = [mode];
      }

      if (rerankStrategy !== "none") {
        body.rerank = {
          enabled: true,
          strategy: rerankStrategy,
          final_top_k: 10,
        };
      }

      let endpoint = `${apiOrigin}/api/v1/search`;
      let requestBody: any = body;

      if (mode === "swarm") {
        if (!sessionIdRef.current) {
          sessionIdRef.current = crypto.randomUUID();
        }
        endpoint = `${apiOrigin}/api/v1/tenants/default_tenant/swarm`;
        requestBody = { query: question.trim(), session_id: sessionIdRef.current };
      }

      const resp = await authFetch(endpoint, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(requestBody),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
      
      if (mode === "swarm") {
        const data = await resp.json();
        setResults({
          results: [{ content: data.answer, title: "Autonomous Agent Response", score: 1.0, source_type: "tree" }],
          distribution: { vector: 0, tree: 0, graph: 0, total: 0 },
          weights_used: { vector: 0, tree: 0, graph: 0 },
          mode_used: "swarm",
          latency_ms: 0,
          query: question.trim()
        });
        setSearchHistory((prev) => [
          { question: question.trim(), mode: "swarm", resultCount: 1 },
          ...prev.slice(0, 9),
        ]);
      } else {
        const data: SearchResponse = await resp.json();
        setResults(data);
        setSearchHistory((prev) => [
          { question: question.trim(), mode: data.mode_used || mode, resultCount: data.results?.length || 0 },
          ...prev.slice(0, 9),
        ]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setLoading(false);
    }
  }, [question, mode, weights, loading, apiOrigin, generateLLM, searchProvider, searchModelId, enableTrace, rerankStrategy]);

  // ── Optimize Handler ────────────────────────────

  const handleOptimize = useCallback(async () => {
    if (!question.trim() || optimizing) return;
    setOptimizing(true);

    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/search/optimize`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ 
          query: question.trim(), 
          count: 5,
          provider: searchProvider !== "default" ? searchProvider : undefined,
          model_id: searchModelId.trim() ? searchModelId.trim() : undefined 
        }),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: OptimizeResponse = await resp.json();
      setSuggestions(data.suggestions);
      setOptimizeModel(data.model_used);
    } catch (err) {
      console.error("Optimize failed:", err);
    } finally {
      setOptimizing(false);
    }
  }, [question, optimizing, apiOrigin]);

  // ── Benchmark Handler ───────────────────────────

  const autoRunLabel = useMemo(() => {
    const today = new Date().toISOString().split("T")[0];
    let stratStr = "Vector";
    if (weights.tree > 0 || weights.graph > 0) stratStr = "Hybrid";
    if (weights.vector === 0 && weights.tree > 0) stratStr = "Tree";
    if (weights.vector === 0 && weights.graph > 0) stratStr = "Graph";

    let modelStr = evalModelId.trim() || evalProvider;
    if (modelStr === "default" || !modelStr) modelStr = "DefaultModel";
    else modelStr = modelStr.split("/").pop() || modelStr; // Shorten model names like ollama/llama3

    const rankStr = rerankStrategy !== "none" ? `-${rerankStrategy}` : "";
    const genStr = evaluateGeneration ? "-Judge" : "";
    
    return `${today}-${modelStr}-${stratStr}${rankStr}${genStr}`;
  }, [weights, evalProvider, evalModelId, rerankStrategy, evaluateGeneration]);

  const handleBenchmark = useCallback(async () => {
    setBenchmarkLoading(true);
    setBenchmarkError(null);
    setBenchmarkResults(null);

    try {
      let items: BenchmarkItem[];
      try {
        const parsed = JSON.parse(benchmarkItems);
        // Auto-extract if user pastes object wrapper
        items = Array.isArray(parsed) ? parsed : (parsed.eval_set || [parsed]);
        if (!Array.isArray(items) || items.length === 0) throw new Error("Must be a non-empty array");
        // Lightweight verification
        if (!items[0]?.query || !items[0]?.expected_titles) {
           throw new Error("Missing query or expected_titles in items");
        }
      } catch (e) {
        throw new Error("Invalid format. Expected: [{\"query\": \"...\", \"expected_titles\": [\"...\"]}]");
      }

      const payload: Record<string, any> = {
        name: benchmarkLabel || autoRunLabel,
        eval_set: items,
        params: {
          weights,
          top_k: 5,
          vector_alpha: 0.5,
          vector_threshold: 0.0,
          graph_hops: 1,
          rerank: rerankStrategy !== "none" ? {
            enabled: true,
            strategy: rerankStrategy,
            final_top_k: 5
          } : null,
          search_provider: searchProvider !== "default" ? searchProvider : undefined,
          search_model: searchModelId.trim() ? searchModelId.trim() : undefined,
          generation_provider: evalProvider !== "default" ? evalProvider : undefined,
          generation_model: evalModelId.trim() ? evalModelId.trim() : undefined,
          generation_temperature: 0.1,
          generation_max_tokens: 1024,
        },
        judge_provider: evalProvider !== "default" ? evalProvider : undefined,
        judge_model: evalModelId.trim() ? evalModelId.trim() : undefined,
        evaluate_generation: evaluateGeneration
      };

      if (activeDatasetId !== "none") {
        payload.dataset_id = activeDatasetId;
        payload.dataset_name = datasets.find((d) => d.id === activeDatasetId)?.name;
      }

      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
      const initData = await resp.json();
      
      if (initData.status === "completed") {
         setBenchmarkResults(initData);
      } else {
         let currentRunId = initData.run_id;
         while (true) {
            await new Promise(r => setTimeout(r, 5000));
            const pollResp = await authFetch(`${apiOrigin}/api/v1/rag-eval/runs/${currentRunId}`);
            if (!pollResp.ok) continue;
            const pollData = await pollResp.json();
            if (pollData.status === "completed") {
              setBenchmarkResults(pollData);
              break;
            } else if (pollData.status === "error") {
              throw new Error("Benchmark evaluation failed in background.");
            }
         }
      }
    } catch (err) {
      setBenchmarkError(err instanceof Error ? err.message : "Benchmark failed");
    } finally {
      setBenchmarkLoading(false);
    }
  }, [benchmarkItems, weights, benchmarkLabel, autoRunLabel, evaluateGeneration, rerankStrategy, evalProvider, evalModelId, apiOrigin, activeDatasetId, datasets]);

  const handleSaveDataset = async () => {
    if (!benchmarkItems.trim() || !saveDatasetName.trim()) return;
    try {
      const parsed = JSON.parse(benchmarkItems);

      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/datasets`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ 
          name: saveDatasetName, 
          description: "", 
          difficulty: saveDatasetDifficulty,
          question_type: saveDatasetQuestionType,
          eval_set: parsed 
        }),
      });

      if (resp.ok) {
        await fetchDatasets();
        const data = await resp.json();
        setActiveDatasetId(data.id);
        setSaveModalOpen(false);
        setSaveDatasetName("");
        setSaveDatasetDifficulty("mixed");
        setSaveDatasetQuestionType("mixed");
      } else {
        alert("Failed to save dataset");
      }
    } catch (e) {
      alert("Invalid JSON data");
    }
  };



  // ── Generate Set Handler ────────────────────────

  const handleGenerateSet = useCallback(async () => {
    if (!genPrompt.trim() || isGenerating) return;
    setIsGenerating(true);
    try {
      const resp = await authFetch(`${apiOrigin}/api/v1/rag-eval/generate-set`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          prompt: genPrompt.trim(),
          count: genCount,
          multi_turn: genMultiTurn,
          provider: genProvider !== "default" ? genProvider : undefined,
          model_id: genModelId.trim() ? genModelId.trim() : undefined
        }),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
      const data = await resp.json();
      const evalArray = Array.isArray(data) ? data : (data.eval_set || data);
      setBenchmarkItems(JSON.stringify(evalArray, null, 2));
      setGenModalOpen(false);
    } catch (err) {
      console.error("Generate failed:", err);
      alert("Failed to generate set: " + (err instanceof Error ? err.message : String(err)));
    } finally {
      setIsGenerating(false);
    }
  }, [genPrompt, genCount, genMultiTurn, genProvider, genModelId, isGenerating, apiOrigin]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSearch();
    }
  };

  const handleApplyConfig = async (e: React.MouseEvent) => {
    e.preventDefault();
    setIsSavingConfig(true);
    setApplyMessage(null);
    try {
      const draftConfig = {
        provider: searchProvider !== "default" ? searchProvider : undefined,
        model_id: searchModelId || undefined,
        use_rag: mode !== "graph" && mode !== "tree",
        use_knowledge_graph: mode === "hybrid" || mode === "graph",
        use_pageindex: mode === "hybrid" || mode === "tree",
        rag_params: {
          weights: mode === "hybrid" ? weights : undefined,
          advanced: {
            top_k_per_source: 10,
            vector_alpha: alpha,
            vector_threshold: threshold,
            graph_hops: hopLimit
          }
        },
        rerank_config: rerankStrategy !== "none" ? {
          enabled: true,
          strategy: rerankStrategy,
          final_top_k: 10
        } : undefined
      };
      
      sessionStorage.setItem("draftRagConfig", JSON.stringify(draftConfig));
      setApplyMessage({ type: "success", text: "Redirecting to Agent Studio..." });
      
      // Short delay to let user read
      setTimeout(() => {
        router.push("/agents?action=create_from_rag");
      }, 500);
      
    } catch (e: any) {
      setApplyMessage({ type: "error", text: "Failed: " + e.message });
      setTimeout(() => setApplyMessage(null), 5000);
    } finally {
      // Don't set isSavingConfig to false immediately if redirecting
      setTimeout(() => setIsSavingConfig(false), 2000);
    }
  };

  // ── Render ──────────────────────────────────────

  return (
    <div className="container mx-auto p-8 max-w-7xl">
      {/* Header */}
      <div className="flex justify-between items-start mb-8">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Sparkles className="h-8 w-8 text-primary" />
            RAG Ensemble Playground
          </h1>
          <p className="text-muted-foreground mt-1">
            Test hybrid retrieval across Vector, Tree, and Graph sources with configurable weights
          </p>
        </div>
        <div className="flex items-center gap-3">
          <Button onClick={() => setWizardOpen(true)} className="bg-primary/10 text-primary hover:bg-primary/20" variant="secondary">
            <Zap className="h-4 w-4 mr-2" />
            Quick Start
          </Button>
          <SourceLegend />
          {/* Tab Toggle */}
          <div className="flex border rounded-lg overflow-hidden bg-muted/30">
            <button
              onClick={() => setActiveTab("search")}
              className={`px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === "search"
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              <Search className="h-4 w-4 inline mr-1.5" />
              Search
            </button>
            <button
              onClick={() => setActiveTab("benchmark")}
              className={`px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === "benchmark"
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              <FlaskConical className="h-4 w-4 inline mr-1.5" />
              Benchmark
            </button>
            <button
              onClick={() => setActiveTab("evaluation")}
              className={`px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === "evaluation"
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              <Activity className="h-4 w-4 inline mr-1.5" />
              Evaluation Matrix
            </button>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* ── Left Panel — Controls ──────────────── */}
        <div className="space-y-6">
          {/* LLM Configuration - Search */}
          {activeTab === "search" && (
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">Generation Model</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="space-y-1">
                  <Label className="text-xs">Provider</Label>
                  <Select value={searchProvider} onValueChange={(val) => { setSearchProvider(val); setSearchModelId(""); }}>
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue placeholder="Select Provider" />
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
                  {searchProvider === "default" ? (
                    <Input 
                      placeholder="Default for slot" 
                      value={searchModelId}
                      onChange={(e) => setSearchModelId(e.target.value)}
                      className="h-8 text-xs"
                    />
                  ) : (
                    <Select value={searchModelId || "default"} onValueChange={(val) => setSearchModelId(val === "default" ? "" : val)}>
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue placeholder="Model Check..." />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="default">Default for Provider</SelectItem>
                        {availableModels.filter(m => m.provider === searchProvider).map(m => (
                          <SelectItem key={m.model_id} value={m.model_id}>{m.model_id.split('/').pop() || m.model_id}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                </div>
                <div className="flex items-center space-x-2 pt-3 border-t">
                  <input
                    type="checkbox"
                    id="gen-llm"
                    checked={generateLLM}
                    onChange={(e) => setGenerateLLM(e.target.checked)}
                    className="h-3.5 w-3.5 cursor-pointer accent-primary"
                  />
                  <Label htmlFor="gen-llm" className="text-xs cursor-pointer font-medium text-primary">
                    Generate Answer using LLM
                  </Label>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Search Mode */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Search Mode</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <Select value={mode} onValueChange={setMode}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="hybrid">
                    <div className="flex items-center gap-2">
                      <Sparkles className="h-4 w-4 text-primary" />
                      Hybrid (All Sources)
                    </div>
                  </SelectItem>
                  <SelectItem value="vector">
                    <div className="flex items-center gap-2">
                      <Database className="h-4 w-4 text-blue-500" />
                      Vector Only
                    </div>
                  </SelectItem>
                  <SelectItem value="tree">
                    <div className="flex items-center gap-2">
                      <TreePine className="h-4 w-4 text-green-500" />
                      Tree Only
                    </div>
                  </SelectItem>
                  <SelectItem value="graph">
                    <div className="flex items-center gap-2">
                      <Share2 className="h-4 w-4 text-purple-500" />
                      Graph Only
                    </div>
                  </SelectItem>
                  <SelectItem value="swarm">
                    <div className="flex items-center gap-2">
                      <Wand2 className="h-4 w-4 text-orange-500" />
                      Autonomous Agent (Swarm)
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>
            </CardContent>
          </Card>

          {/* Weight Slider */}
          {mode === "hybrid" && (
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">Ensemble Weights</CardTitle>
                <CardDescription className="text-xs">
                  Adjust how much each source contributes
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex gap-2">
                  <Badge variant="outline" className="cursor-pointer hover:bg-muted" onClick={() => {setWeights({vector: 0.5, tree: 0.3, graph: 0.2}); setRerankStrategy("none")}}>Balanced</Badge>
                  <Badge variant="outline" className="cursor-pointer hover:bg-muted" onClick={() => {setWeights({vector: 0.8, tree: 0.2, graph: 0.0}); setRerankStrategy("none")}}>Speed</Badge>
                  <Badge variant="outline" className="cursor-pointer hover:bg-muted font-medium bg-purple-500/10 text-purple-600 border-purple-500/20" title="Cross-Encoder re-ranking with deep graph traversal" onClick={() => {setWeights({vector: 0.4, tree: 0.3, graph: 0.3}); setRerankStrategy("cross-encoder")}}>Max Accuracy</Badge>
                </div>
                <WeightSlider weights={weights} onChange={setWeights} disabled={loading} />
              </CardContent>
            </Card>
          )}

          {/* Re-ranking Options */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Re-ranking Strategy</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <Select value={rerankStrategy} onValueChange={setRerankStrategy}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Fast (No Re-ranking)</SelectItem>
                  <SelectItem value="rrf">RRF (Reciprocal Rank Fusion)</SelectItem>
                  <SelectItem value="cross-encoder">Cross-Encoder (Accurate / Slower) 🚀</SelectItem>
                </SelectContent>
              </Select>
            </CardContent>
          </Card>

          {/* Apply to Agent Button */}
          <div className="pt-2 pb-1 space-y-2">
            <Button 
                type="button"
                onClick={handleApplyConfig}
                disabled={isSavingConfig}
                className="w-full h-12 bg-gradient-to-r from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white shadow-lg shadow-purple-200 dark:shadow-none"
            >
                {isSavingConfig ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Wand2 className="w-5 h-5 mr-2" />}
                Create Agent with these Settings
            </Button>
            {applyMessage && (
              <div className={`text-xs p-2 rounded text-center font-medium ${applyMessage.type === "success" ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" : "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400"}`}>
                {applyMessage.text}
              </div>
            )}
            <p className="text-[11px] text-center text-muted-foreground mt-2">
                Sends these RAG settings to the Agent Studio so you can build a specialized AI Agent.
            </p>
          </div>

          {/* Data Sources */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Data Sources</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm flex items-center gap-1.5">🔷 Vector DB</span>
                <Badge variant="outline" className="text-[10px] px-1.5 h-5 bg-green-500/10 text-green-500 border-green-500/20">Online</Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm flex items-center gap-1.5">🌿 PageIndex</span>
                <Badge variant="outline" className="text-[10px] px-1.5 h-5 bg-green-500/10 text-green-500 border-green-500/20">Online</Badge>
              </div>
              <div className="border-t pt-3">
                <GraphStatus />
              </div>
            </CardContent>
          </Card>

          {/* Search History */}
          {searchHistory.length > 0 && (
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">Recent Searches</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                {searchHistory.map((h, i) => (
                  <button
                    key={i}
                    onClick={() => setQuestion(h.question)}
                    className="w-full text-left text-xs p-2 rounded-md hover:bg-muted/80 bg-muted/40 transition-colors"
                  >
                    <div className="flex items-center justify-between">
                      <span className="truncate flex-1">{h.question}</span>
                      <Badge variant="outline" className="text-[9px] ml-2 shrink-0">
                        {h.mode} · {h.resultCount}
                      </Badge>
                    </div>
                  </button>
                ))}
              </CardContent>
            </Card>
          )}
        </div>

        {/* ── Main Panel ────────────────────────── */}
        <div className="lg:col-span-3 space-y-6">

          {activeTab === "search" ? (
            <>
              {/* Search Bar + Optimize Button */}
              <Card>
                <CardContent className="p-4">
                  <div className="flex gap-3">
                    <div className="relative flex-1">
                      <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                      <Input
                        id="search-input"
                        value={question}
                        onChange={(e) => setQuestion(e.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder="Ask a question about your knowledge base..."
                        disabled={loading}
                        className="pl-10 h-12 text-base"
                      />
                    </div>
                    <Button
                      id="optimize-btn"
                      variant="outline"
                      onClick={handleOptimize}
                      disabled={optimizing || !question.trim()}
                      className="h-12 px-4 border-primary/30 hover:bg-primary/10"
                      title="AI-optimize this query"
                    >
                      {optimizing ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <>
                          <Wand2 className="mr-1.5 h-4 w-4 text-primary" />
                          Optimize
                        </>
                      )}
                    </Button>
                    <Button
                      id="search-btn"
                      onClick={handleSearch}
                      disabled={loading || !question.trim()}
                      className="h-12 px-6"
                    >
                      {loading ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <>
                          <Send className="mr-2 h-4 w-4" />
                          Search
                        </>
                      )}
                    </Button>
                  </div>
                  
                  {/* Trace Toggle */}
                  <div className="flex justify-end mt-3">
                    <div className="flex items-center space-x-2 bg-muted/20 px-2.5 py-1.5 rounded-md border border-border/50 transition-colors hover:bg-muted/30">
                      <input
                        type="checkbox"
                        id="trace-toggle"
                        checked={enableTrace}
                        onChange={(e) => setEnableTrace(e.target.checked)}
                        className="h-3.5 w-3.5 cursor-pointer accent-primary"
                      />
                      <Label htmlFor="trace-toggle" className="text-[11px] uppercase tracking-wider cursor-pointer font-semibold text-muted-foreground transition-colors">
                        🔬 Trace Pipeline
                      </Label>
                    </div>
                  </div>

                  {/* AI Optimizer Suggestions */}
                  {suggestions.length > 0 && (
                    <div className="mt-4 space-y-2">
                      <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <Sparkles className="h-3 w-3 text-primary" />
                        <span>AI-optimized queries</span>
                        {optimizeModel && (
                          <Badge variant="outline" className="text-[9px] px-1.5 h-4">
                            via {optimizeModel}
                          </Badge>
                        )}
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {suggestions.map((s, i) => (
                          <button
                            key={i}
                            onClick={() => {
                              setQuestion(s.query);
                              setSuggestions([]);
                            }}
                            className={`group relative inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full 
                              text-xs border transition-all hover:scale-[1.02] active:scale-[0.98]
                              ${STRATEGY_COLORS[s.strategy] || STRATEGY_COLORS.original}`}
                            title={s.explanation}
                          >
                            {STRATEGY_ICONS[s.strategy] || STRATEGY_ICONS.original}
                            <span className="max-w-[200px] truncate">{s.query}</span>
                            <span className="opacity-60 text-[10px]">
                              {(s.confidence * 100).toFixed(0)}%
                            </span>
                          </button>
                        ))}
                      </div>
                    </div>
                  )}
                </CardContent>
              </Card>

              {/* Error */}
              {error && (
                <Card className="border-red-500/30 bg-red-500/5">
                  <CardContent className="p-4 text-red-400 text-sm">
                    ❌ {error}
                  </CardContent>
                </Card>
              )}

              {/* Pipeline Trace Visualizer (Always visible to show architecture/trace) */}
              <div className="mb-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                <PipelineVisualizer 
                  traceLog={results?.trace_log || []} 
                  totalLatencyMs={results?.latency_ms || 0}
                  isLoading={loading}
                  playgroundState={{
                    mode, weights, searchProvider, searchModelId,
                    evalProvider, evalModelId, generateLLM, rerankStrategy,
                    hopLimit, alpha, threshold, availableModels,
                  }}
                  onConfigChange={handleConfigChange}
                  onReRun={handleSearch}
                />
              </div>

              {/* Results */}
              {results && (
                <>
                  {/* Synthesis Block */}
                  {results.synthesis && (
                    <Card className="border-primary/50 bg-primary/5 mb-4 shadow-sm">
                      <CardContent className="p-5">
                        <div className="flex items-center gap-2 mb-3 text-sm font-semibold text-primary">
                          <Wand2 className="h-4 w-4" />
                          AI Synthesized Answer
                        </div>
                        <div className="text-sm leading-relaxed whitespace-pre-wrap text-foreground/90">
                          {results.synthesis}
                        </div>
                      </CardContent>
                    </Card>
                  )}

                  {/* Source Distribution + Metrics */}
                  <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                    <Card className="col-span-2">
                      <CardContent className="p-4">
                        <div className="flex items-center gap-2 mb-3 text-sm font-medium">
                          <BarChart3 className="h-4 w-4" />
                          Source Distribution
                        </div>
                        <div className="flex h-3 rounded-full overflow-hidden bg-muted mb-3">
                          {results.distribution.vector > 0 && (
                            <div
                              className="bg-blue-500 transition-all"
                              style={{ width: `${(results.distribution.vector / results.distribution.total) * 100}%` }}
                            />
                          )}
                          {results.distribution.tree > 0 && (
                            <div
                              className="bg-green-500 transition-all"
                              style={{ width: `${(results.distribution.tree / results.distribution.total) * 100}%` }}
                            />
                          )}
                          {results.distribution.graph > 0 && (
                            <div
                              className="bg-purple-500 transition-all"
                              style={{ width: `${(results.distribution.graph / results.distribution.total) * 100}%` }}
                            />
                          )}
                        </div>
                        <div className="flex gap-4 text-xs text-muted-foreground">
                          <span>🔷 Vector: {results.distribution.vector}</span>
                          <span>🌿 Tree: {results.distribution.tree}</span>
                          <span>🔮 Graph: {results.distribution.graph}</span>
                        </div>
                      </CardContent>
                    </Card>
                    <Card>
                      <CardContent className="p-4 flex flex-col items-center justify-center relative">
                        <Clock className="h-5 w-5 text-muted-foreground mb-1" />
                        <span className="text-2xl font-bold">{results.latency_ms}</span>
                        <span className="text-[10px] text-muted-foreground">ms latency</span>
                        {results.trace_log && (
                          <div className="absolute top-2 right-2 flex items-center justify-center">
                            <span className="h-2 w-2 rounded-full bg-primary animate-pulse relative"></span>
                            <span className="absolute h-2 w-2 rounded-full bg-primary animate-ping opacity-75"></span>
                          </div>
                        )}
                      </CardContent>
                    </Card>
                    <Card>
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <TrendingUp className="h-5 w-5 text-muted-foreground mb-1" />
                        <span className="text-2xl font-bold">{results.results.length}</span>
                        <span className="text-[10px] text-muted-foreground">{results.mode_used} results</span>
                      </CardContent>
                    </Card>
                  </div>

                  {/* Source Cards */}
                  {results.results && results.results.length > 0 && (
                    <div className="space-y-3 mt-8">
                      <h3 className="text-sm font-medium text-muted-foreground">
                        Results ({results.results.length})
                      </h3>
                      {results.results.map((result, i) => (
                        <Card key={i} className="hover:border-primary/30 transition-colors">
                          <CardContent className="p-4">
                            <div className="flex items-start justify-between mb-2">
                              <div className="flex items-center gap-2">
                                <span className="text-xs font-mono text-muted-foreground w-5">#{i + 1}</span>
                                <h4 className="font-medium text-sm">{result.title}</h4>
                              </div>
                              <div className="flex items-center gap-2 shrink-0">
                                <Badge variant="outline" className="text-[10px] font-mono">
                                  {result.score.toFixed(3)}
                                </Badge>
                                <SourceBadge sourceType={result.source_type} />
                              </div>
                            </div>
                            <p className="text-xs text-muted-foreground bg-muted/50 rounded px-3 py-2 line-clamp-3">
                              {result.content}
                            </p>
                          </CardContent>
                        </Card>
                      ))}
                    </div>
                  )}

                </>
              )}

              {/* Empty State */}
              {!results && !loading && !error && (
                <Card className="border-dashed">
                  <CardContent className="flex flex-col items-center justify-center py-20 text-center">
                    <Search className="h-12 w-12 text-muted-foreground/30 mb-4" />
                    <h3 className="text-lg font-semibold mb-2">Search Your Knowledge Base</h3>
                    <p className="text-muted-foreground max-w-md text-sm">
                      Ask a question to test hybrid retrieval across Vector (semantic search),
                      Tree (document structure), and Graph (entity relationships).
                    </p>
                    <div className="flex gap-2 mt-6">
                      <Button variant="outline" size="sm" onClick={() => setQuestion("What are the common side effects of Aspirin?")}>
                        Try: Side effects
                      </Button>
                      <Button variant="outline" size="sm" onClick={() => setQuestion("How does the API authentication work?")}>
                        Try: API auth
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              )}
            </>
          ) : activeTab === "benchmark" ? (
            /* ── Benchmark Tab ────────────────────── */
            <>
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2 text-lg">
                    <FlaskConical className="h-5 w-5 text-primary" />
                    Batch Benchmark
                  </CardTitle>
                  <CardDescription>
                    Run evaluation queries against ground-truth data to measure Hit Rate and MRR
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  {/* Label */}
                  <div className="flex gap-3">
                    <div className="flex-1">
                      <Label className="text-xs mb-1.5 block">Run Label (Leave blank to use auto-generated name)</Label>
                      <Input
                        id="benchmark-label"
                        value={benchmarkLabel}
                        onChange={(e) => setBenchmarkLabel(e.target.value)}
                        placeholder={`Auto: ${autoRunLabel}`}
                        className="h-9 placeholder:text-purple-500/50"
                      />
                    </div>
                    <div className="flex items-end mb-1">
                      <div className="flex items-center space-x-2 bg-muted/30 p-2 border rounded-md">
                        <input
                          type="checkbox"
                          id="eval-generation"
                          checked={evaluateGeneration}
                          onChange={(e) => setEvaluateGeneration(e.target.checked)}
                          className="h-4 w-4 rounded border-gray-300 text-primary"
                        />
                        <Label htmlFor="eval-generation" className="text-xs cursor-pointer m-0 leading-none">
                          Evaluate Generation (Judge LLM)
                        </Label>
                      </div>
                    </div>
                  </div>
                  
                  {evaluateGeneration && (
                    <div className="flex gap-3 pt-2">
                       <div className="flex-1 space-y-1">
                         <Label className="text-xs">Judge Provider</Label>
                         <Select value={evalProvider} onValueChange={(val) => { setEvalProvider(val); setEvalModelId(""); }}>
                           <SelectTrigger className="h-8 text-xs">
                             <SelectValue placeholder="Select Provider" />
                           </SelectTrigger>
                           <SelectContent>
                             <SelectItem value="default">Tenant Default</SelectItem>
                             {Array.from(new Set(availableModels.map(m => m.provider))).map(p => (
                                <SelectItem key={p} value={p}>{p.toUpperCase()}</SelectItem>
                             ))}
                           </SelectContent>
                         </Select>
                       </div>
                       
                       <div className="flex-1 space-y-1">
                         <Label className="text-xs">Judge Model ID</Label>
                         {evalProvider === "default" ? (
                           <Input 
                             placeholder="Default for slot" 
                             value={evalModelId}
                             onChange={(e) => setEvalModelId(e.target.value)}
                             className="h-8 text-xs"
                           />
                         ) : (
                           <Select value={evalModelId || "default"} onValueChange={(val) => setEvalModelId(val === "default" ? "" : val)}>
                             <SelectTrigger className="h-8 text-xs">
                               <SelectValue placeholder="Model Check..." />
                             </SelectTrigger>
                             <SelectContent>
                               <SelectItem value="default">Default for Provider</SelectItem>
                               {availableModels.filter(m => m.provider === evalProvider).map(m => (
                                 <SelectItem key={m.model_id} value={m.model_id}>{m.model_id.split('/').pop() || m.model_id}</SelectItem>
                               ))}
                             </SelectContent>
                           </Select>
                         )}
                       </div>
                    </div>
                  )}

                  {/* Eval Set Input */}
                  <div>
                    <Label className="text-xs mb-1.5 block">
                      Evaluation Set (JSON)
                      <div className="mt-2 mb-2 flex items-center justify-between">
                        <div className="flex gap-2">
                          <Select 
                            value={activeDatasetId} 
                            onValueChange={(val) => {
                              setActiveDatasetId(val);
                              if (val !== "none") {
                                const ds = datasets.find(d => d.id === val);
                                if (ds) setBenchmarkItems(JSON.stringify(ds.eval_set, null, 2));
                              }
                            }}
                          >
                            <SelectTrigger className="w-[200px] h-8 text-xs">
                              <SelectValue placeholder="Load Dataset..." />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="none">Custom / Inline Data</SelectItem>
                              {datasets.map((d) => (
                                <SelectItem key={d.id} value={d.id}>
                                  {d.name} v{d.version || 1} — {d.items_count} items
                                  {d.difficulty && ` [${d.difficulty}]`}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                          
                          <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => setSaveModalOpen(true)} disabled={!benchmarkItems.trim() || activeDatasetId !== "none"}>
                            <Database className="w-3 h-3 mr-1" /> Save New
                          </Button>
                          {activeDatasetId !== "none" && (
                            <Button variant="outline" size="sm" className="h-8 text-xs" onClick={() => {
                              const ds = datasets.find(d => d.id === activeDatasetId);
                              if (ds) {
                                setSaveDatasetName(ds.name);
                                setSaveDatasetDifficulty(ds.difficulty || "mixed");
                                setSaveDatasetQuestionType(ds.question_type || "mixed");
                              }
                              setSaveModalOpen(true);
                            }}>
                              <Database className="w-3 h-3 mr-1" /> Save as New Version
                            </Button>
                          )}
                        </div>
                        <div className="flex gap-4">
                          <button
                            className="text-primary hover:underline font-medium"
                            onClick={() => {
                              setActiveDatasetId("none");
                              setBenchmarkItems(JSON.stringify([
                                { query: "What are the side effects of Aspirin?", expected_titles: ["Aspirin"] },
                                { query: "How does ibuprofen work?", expected_titles: ["Ibuprofen", "NSAID"] },
                                { query: "Drug interactions with Warfarin", expected_titles: ["Warfarin", "Drug Interactions"] },
                              ], null, 2));
                            }}
                          >
                            Load Example
                          </button>
                          <button
                            className="text-purple-500 hover:text-purple-400 hover:underline inline-flex items-center gap-1 font-medium"
                            onClick={() => setGenModalOpen(true)}
                          >
                            ✨ Generate with AI
                          </button>
                        </div>
                      </div>
                    </Label>
                    <textarea
                      id="benchmark-input"
                      value={benchmarkItems}
                      readOnly={activeDatasetId !== "none"}
                      onChange={(e) => {
                        setBenchmarkItems(e.target.value);
                        setActiveDatasetId("none"); // switch to custom if they type
                      }}
                      placeholder={`[\n  {"query": "What is Aspirin?", "expected_titles": ["Aspirin Guide"]},\n  {"query": "Drug interactions", "expected_titles": ["Drug Info"]}\n]`}
                      className={`w-full h-40 rounded-md border bg-muted/30 px-3 py-2 text-sm font-mono resize-y focus:outline-none focus:ring-2 focus:ring-primary/30 ${activeDatasetId !== "none" ? "opacity-60 cursor-not-allowed" : ""}`}
                    />
                    {activeDatasetId !== "none" && (
                      <p className="text-[10px] text-amber-500 mt-1">📌 Read-only — loaded from saved dataset. Click &quot;Custom / Inline Data&quot; to edit.</p>
                    )}
                  </div>

                  <Button
                    id="run-benchmark-btn"
                    onClick={handleBenchmark}
                    disabled={benchmarkLoading || !benchmarkItems.trim()}
                    className="w-full"
                  >
                    {benchmarkLoading ? (
                      <Loader2 className="h-4 w-4 animate-spin mr-2" />
                    ) : (
                      <Play className="h-4 w-4 mr-2" />
                    )}
                    Run Benchmark
                  </Button>
                </CardContent>
              </Card>

              {/* Benchmark Error */}
              {benchmarkError && (
                <Card className="border-red-500/30 bg-red-500/5">
                  <CardContent className="p-4 text-red-400 text-sm">
                    ❌ {benchmarkError}
                  </CardContent>
                </Card>
              )}

              {/* Benchmark Results */}
              {benchmarkResults && (
                <>
                  {/* Metrics Cards */}
                  <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                    <Card className={`${benchmarkResults.hit_rate >= 0.7 ? "border-green-500/30" : benchmarkResults.hit_rate >= 0.4 ? "border-amber-500/30" : "border-red-500/30"}`}>
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <Target className="h-5 w-5 text-primary mb-1" />
                        <span className="text-3xl font-bold">
                          {(benchmarkResults.hit_rate * 100).toFixed(1)}%
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          Hit Rate @5
                        </span>
                      </CardContent>
                    </Card>
                    <Card className={`${benchmarkResults.mrr >= 0.7 ? "border-green-500/30" : benchmarkResults.mrr >= 0.4 ? "border-amber-500/30" : "border-red-500/30"}`}>
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <TrendingUp className="h-5 w-5 text-primary mb-1" />
                        <span className="text-3xl font-bold">
                          {benchmarkResults.mrr.toFixed(3)}
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          MRR @5
                        </span>
                      </CardContent>
                    </Card>
                    <Card className="border-blue-500/20">
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <BarChart3 className="h-5 w-5 text-blue-500 mb-1" />
                        <span className="text-3xl font-bold">
                          {benchmarkResults.ndcg.toFixed(3)}
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          NDCG
                        </span>
                      </CardContent>
                    </Card>
                    <Card className="border-purple-500/20">
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <FlaskConical className="h-5 w-5 text-purple-500 mb-1" />
                        <span className="text-3xl font-bold">
                          {benchmarkResults.map.toFixed(3)}
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          MAP
                        </span>
                      </CardContent>
                    </Card>
                  </div>

                  {/* Run Info */}
                  <Card className="bg-primary/5 border-primary/20">
                    <CardContent className="p-4 flex flex-col items-center justify-center space-y-2">
                       <p className="text-sm">Evaluation run created successfully.</p>
                       <div className="flex items-center gap-2">
                         <span className="text-xs text-muted-foreground">Run ID:</span>
                         <code className="text-xs bg-background px-2 py-1 rounded border">{benchmarkResults.run_id}</code>
                       </div>
                       <Button 
                         variant="outline" 
                         size="sm" 
                         className="mt-2"
                         onClick={() => setActiveTab("evaluation")}
                       >
                         View Details in Evaluation Matrix ➜
                       </Button>
                    </CardContent>
                  </Card>
                </>
              )}

              {/* Empty Benchmark State */}
              {!benchmarkResults && !benchmarkLoading && !benchmarkError && (
                <Card className="border-dashed">
                  <CardContent className="flex flex-col items-center justify-center py-16 text-center">
                    <FlaskConical className="h-12 w-12 text-muted-foreground/30 mb-4" />
                    <h3 className="text-lg font-semibold mb-2">Batch Benchmark</h3>
                    <p className="text-muted-foreground max-w-md text-sm">
                      Define a set of queries with expected document titles.
                      Run them all to calculate <strong>Hit Rate</strong> (% of queries with hits in top-5)
                      and <strong>MRR</strong> (Mean Reciprocal Rank).
                    </p>
                  </CardContent>
                </Card>
              )}
            </>
          ) : activeTab === "evaluation" ? (
            <RagEvalDashboard />
          ) : null}
        </div>
      </div>

      {/* ── Save Dataset Modal ────────────────────── */}
      <Dialog open={saveModalOpen} onOpenChange={setSaveModalOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Database className="w-5 h-5 text-primary" />
              Save Evaluation Dataset
            </DialogTitle>
            <DialogDescription>
              Assign a recognizable name to quickly load this benchmark set later.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="dataset-name">Dataset Name</Label>
              <Input
                id="dataset-name"
                placeholder="e.g. Clinical Protocol Eval V1"
                value={saveDatasetName}
                onChange={(e) => setSaveDatasetName(e.target.value)}
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleSaveDataset();
                  }
                }}
              />
            </div>
            
            <div className="flex gap-4">
              <div className="space-y-2 flex-1">
                <Label>Difficulty</Label>
                <Select value={saveDatasetDifficulty} onValueChange={setSaveDatasetDifficulty}>
                  <SelectTrigger className="h-9">
                    <SelectValue placeholder="Difficulty" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="beginner">Beginner</SelectItem>
                    <SelectItem value="intermediate">Intermediate</SelectItem>
                    <SelectItem value="advanced">Advanced</SelectItem>
                    <SelectItem value="mixed">Mixed</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2 flex-1">
                <Label>Question Type</Label>
                <Select value={saveDatasetQuestionType} onValueChange={setSaveDatasetQuestionType}>
                  <SelectTrigger className="h-9">
                    <SelectValue placeholder="Question Type" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="factual">Factual QA</SelectItem>
                    <SelectItem value="reasoning">Reasoning</SelectItem>
                    <SelectItem value="extraction">Extraction</SelectItem>
                    <SelectItem value="mixed">Mixed</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <p className="text-xs text-muted-foreground">
              A snapshot of your JSON will be saved under this name.
            </p>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSaveModalOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleSaveDataset} disabled={!saveDatasetName.trim()}>
              Save Dataset
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Generate Set Modal ────────────────────── */}
      <Dialog open={genModalOpen} onOpenChange={setGenModalOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Wand2 className="h-5 w-5 text-purple-500" />
              AI Prompt Generator
            </DialogTitle>
            <DialogDescription>
              Using the underlying LLM to generate an evaluation set from real knowledge base documents.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label>Generation Prompt / Topic</Label>
              <Input 
                placeholder="e.g. Extract common questions about antibiotics side effects..." 
                value={genPrompt}
                onChange={(e) => setGenPrompt(e.target.value)}
              />
            </div>
            <div className="flex gap-4">
              <div className="space-y-2 flex-1">
                <Label>Provider</Label>
                <Select value={genProvider} onValueChange={(val) => { setGenProvider(val); setGenModelId(""); }}>
                  <SelectTrigger><SelectValue placeholder="Select Provider" /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="default">Tenant Default</SelectItem>
                    {Array.from(new Set(availableModels.map(m => m.provider))).map(p => (
                       <SelectItem key={p} value={p}>{p.toUpperCase()}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2 flex-1">
                <Label>Model ID (optional)</Label>
                {genProvider === "default" ? (
                  <Input 
                    placeholder="Leave blank for slot default" 
                    value={genModelId}
                    onChange={(e) => setGenModelId(e.target.value)}
                  />
                ) : (
                  <Select value={genModelId || "default"} onValueChange={(val) => setGenModelId(val === "default" ? "" : val)}>
                    <SelectTrigger><SelectValue placeholder="Model Name" /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="default">Default for Provider</SelectItem>
                      {availableModels.filter(m => m.provider === genProvider).map(m => (
                        <SelectItem key={m.model_id} value={m.model_id}>{m.model_id.split('/').pop() || m.model_id}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </div>
            </div>
            <div className="flex gap-4">
              <div className="space-y-2 flex-1">
                <Label>Count</Label>
                <Input 
                  type="number" 
                  min={1} max={20}
                  value={genCount}
                  onChange={(e) => setGenCount(Number(e.target.value))}
                />
              </div>
              <div className="space-y-2 flex-[2] flex flex-col justify-end pb-2">
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="gen-multi"
                    checked={genMultiTurn}
                    onChange={(e) => setGenMultiTurn(e.target.checked)}
                    className="h-4 w-4 rounded border-gray-300 text-purple-600 focus:ring-purple-600"
                  />
                  <Label htmlFor="gen-multi" className="text-sm cursor-pointer mb-0">Multi-turn Conversation</Label>
                </div>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setGenModalOpen(false)}>Cancel</Button>
            <Button 
              className="bg-purple-600 hover:bg-purple-700 text-white" 
              onClick={handleGenerateSet}
              disabled={isGenerating || !genPrompt.trim()}
            >
              {isGenerating ? <Loader2 className="h-4 w-4 animate-spin mr-2" /> : <Sparkles className="h-4 w-4 mr-2" />}
              Generate Tasks
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Quick-Start Wizard Modal */}
      <Dialog open={wizardOpen} onOpenChange={setWizardOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Mimir Quick-Start Wizard</DialogTitle>
            <DialogDescription>
              Choose a preset configuration to begin your evaluation quickly.
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-1 gap-4 py-4">
            <button
              onClick={() => {
                setWeights({vector: 0.5, tree: 0.3, graph: 0.2});
                setRerankStrategy("none");
                setMode("hybrid");
                setActiveTab("benchmark");
                setWizardOpen(false);
              }}
              className="flex flex-col text-left p-4 border rounded-xl hover:border-emerald-500 hover:bg-emerald-500/5 transition-colors group"
            >
              <div className="flex items-center justify-between mb-1">
                <span className="font-semibold group-hover:text-emerald-600">Balanced Evaluation</span>
                <Badge variant="outline" className="bg-emerald-500/10 text-emerald-600 border-emerald-500/20">Recommended</Badge>
              </div>
              <span className="text-xs text-muted-foreground">Hybrid vector + tree + graph. Best ratio of speed to accuracy for general QA datasets.</span>
            </button>

            <button
              onClick={() => {
                setWeights({vector: 0.8, tree: 0.2, graph: 0.0});
                setRerankStrategy("none");
                setMode("hybrid");
                setActiveTab("benchmark");
                setWizardOpen(false);
              }}
              className="flex flex-col text-left p-4 border rounded-xl hover:border-blue-500 hover:bg-blue-500/5 transition-colors group"
            >
              <div className="flex items-center justify-between mb-1">
                <span className="font-semibold group-hover:text-blue-600">High-Speed Retrieval</span>
                <Zap className="h-4 w-4 text-blue-500" />
              </div>
              <span className="text-xs text-muted-foreground">Disables graph hops and emphasizes vector search. Use for large datasets to minimize latency.</span>
            </button>

            <button
              onClick={() => {
                setWeights({vector: 0.4, tree: 0.3, graph: 0.3});
                setRerankStrategy("cross-encoder");
                setMode("hybrid");
                setActiveTab("benchmark");
                setWizardOpen(false);
              }}
              className="flex flex-col text-left p-4 border rounded-xl hover:border-purple-500 hover:bg-purple-500/5 transition-colors group"
            >
              <div className="flex items-center justify-between mb-1">
                <span className="font-semibold group-hover:text-purple-600">Max Accuracy</span>
                <Sparkles className="h-4 w-4 text-purple-500" />
              </div>
              <span className="text-xs text-muted-foreground">Enables dense Cross-Encoder re-ranking and deep graph traversal. Slower, but highest recall.</span>
            </button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
