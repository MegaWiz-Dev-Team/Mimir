"use client";

import { useState, useCallback, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SourceBadge, SourceLegend } from "@/components/ui/source-badge";
import { WeightSlider } from "@/components/ui/weight-slider";
import { GraphStatus } from "@/components/ui/graph-status";
import {
  Send, Loader2, Search, Sparkles, Database, TreePine, Share2,
  BarChart3, Wand2, Zap, Target, TrendingUp, CheckCircle2, XCircle,
  Clock, FlaskConical, ChevronDown, ChevronUp, FileJson, Play
} from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";

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

interface BenchmarkResponse {
  benchmark_id: string;
  total_queries: number;
  hit_rate: number;
  mrr: number;
  avg_latency_ms: number;
  weights_used: { vector: number; tree: number; graph: number };
  per_query: QueryBenchmarkResult[];
  label: string | null;
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
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [weights, setWeights] = useState({ vector: 0.5, tree: 0.3, graph: 0.2 });
  const [searchHistory, setSearchHistory] = useState<{ question: string; mode: string; resultCount: number }[]>([]);

  // Optimizer state
  const [optimizing, setOptimizing] = useState(false);
  const [suggestions, setSuggestions] = useState<QuerySuggestion[]>([]);
  const [optimizeModel, setOptimizeModel] = useState("");

  // Benchmark state
  const [activeTab, setActiveTab] = useState<"search" | "benchmark">("search");
  const [benchmarkItems, setBenchmarkItems] = useState<string>("");
  const [benchmarkLoading, setBenchmarkLoading] = useState(false);
  const [benchmarkResults, setBenchmarkResults] = useState<BenchmarkResponse | null>(null);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  const [showBenchmarkDetails, setShowBenchmarkDetails] = useState(false);
  const [benchmarkLabel, setBenchmarkLabel] = useState("");

  const apiOrigin = API_BASE_URL.replace(/\/api\/v1$/, "");

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
      };
      // Single-source mode: only enable that source
      if (mode !== "hybrid") {
        body.sources = [mode];
      }

      const resp = await authFetch(`${apiOrigin}/api/search`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
      const data: SearchResponse = await resp.json();
      setResults(data);
      setSearchHistory((prev) => [
        { question: question.trim(), mode: data.mode_used || mode, resultCount: data.results?.length || 0 },
        ...prev.slice(0, 9),
      ]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setLoading(false);
    }
  }, [question, mode, weights, loading, apiOrigin]);

  // ── Optimize Handler ────────────────────────────

  const handleOptimize = useCallback(async () => {
    if (!question.trim() || optimizing) return;
    setOptimizing(true);

    try {
      const resp = await authFetch(`${apiOrigin}/api/search/optimize`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: question.trim(), count: 5 }),
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

  const handleBenchmark = useCallback(async () => {
    setBenchmarkLoading(true);
    setBenchmarkError(null);
    setBenchmarkResults(null);

    try {
      let items: BenchmarkItem[];
      try {
        items = JSON.parse(benchmarkItems);
        if (!Array.isArray(items)) throw new Error("Must be an array");
      } catch (e) {
        throw new Error("Invalid JSON. Expected: [{\"query\": \"...\", \"expected_titles\": [\"...\"]}]");
      }

      const resp = await authFetch(`${apiOrigin}/api/search/benchmark`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          items,
          weights,
          limit: 5,
          label: benchmarkLabel || undefined,
        }),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
      const data: BenchmarkResponse = await resp.json();
      setBenchmarkResults(data);
    } catch (err) {
      setBenchmarkError(err instanceof Error ? err.message : "Benchmark failed");
    } finally {
      setBenchmarkLoading(false);
    }
  }, [benchmarkItems, weights, benchmarkLabel, apiOrigin]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSearch();
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
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* ── Left Panel — Controls ──────────────── */}
        <div className="space-y-6">
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
              <CardContent>
                <WeightSlider weights={weights} onChange={setWeights} disabled={loading} />
              </CardContent>
            </Card>
          )}

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

              {/* Results */}
              {results && (
                <>
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
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <Clock className="h-5 w-5 text-muted-foreground mb-1" />
                        <span className="text-2xl font-bold">{results.latency_ms}</span>
                        <span className="text-[10px] text-muted-foreground">ms latency</span>
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
                  {results.results.length > 0 && (
                    <div className="space-y-3">
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
          ) : (
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
                      <Label className="text-xs mb-1.5 block">Run Label (optional)</Label>
                      <Input
                        id="benchmark-label"
                        value={benchmarkLabel}
                        onChange={(e) => setBenchmarkLabel(e.target.value)}
                        placeholder="e.g. benchmark-round-1"
                        className="h-9"
                      />
                    </div>
                  </div>

                  {/* Eval Set Input */}
                  <div>
                    <Label className="text-xs mb-1.5 block">
                      Evaluation Set (JSON)
                      <button
                        className="ml-2 text-primary hover:underline"
                        onClick={() => setBenchmarkItems(JSON.stringify([
                          { query: "What are the side effects of Aspirin?", expected_titles: ["Aspirin"] },
                          { query: "How does ibuprofen work?", expected_titles: ["Ibuprofen", "NSAID"] },
                          { query: "Drug interactions with Warfarin", expected_titles: ["Warfarin", "Drug Interactions"] },
                        ], null, 2))}
                      >
                        Load Example
                      </button>
                    </Label>
                    <textarea
                      id="benchmark-input"
                      value={benchmarkItems}
                      onChange={(e) => setBenchmarkItems(e.target.value)}
                      placeholder={`[\n  {"query": "What is Aspirin?", "expected_titles": ["Aspirin Guide"]},\n  {"query": "Drug interactions", "expected_titles": ["Drug Info"]}\n]`}
                      className="w-full h-40 rounded-md border bg-muted/30 px-3 py-2 text-sm font-mono resize-y focus:outline-none focus:ring-2 focus:ring-primary/30"
                    />
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
                          MRR
                        </span>
                      </CardContent>
                    </Card>
                    <Card>
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <Clock className="h-5 w-5 text-muted-foreground mb-1" />
                        <span className="text-3xl font-bold">
                          {benchmarkResults.avg_latency_ms.toFixed(0)}
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          Avg ms
                        </span>
                      </CardContent>
                    </Card>
                    <Card>
                      <CardContent className="p-4 flex flex-col items-center justify-center">
                        <FlaskConical className="h-5 w-5 text-muted-foreground mb-1" />
                        <span className="text-3xl font-bold">
                          {benchmarkResults.total_queries}
                        </span>
                        <span className="text-[10px] text-muted-foreground font-medium">
                          Queries tested
                        </span>
                      </CardContent>
                    </Card>
                  </div>

                  {/* Per-Query Breakdown */}
                  <Card>
                    <CardHeader className="pb-2">
                      <button
                        onClick={() => setShowBenchmarkDetails(!showBenchmarkDetails)}
                        className="flex items-center justify-between w-full"
                      >
                        <CardTitle className="text-sm flex items-center gap-2">
                          <BarChart3 className="h-4 w-4" />
                          Per-Query Results
                        </CardTitle>
                        {showBenchmarkDetails
                          ? <ChevronUp className="h-4 w-4" />
                          : <ChevronDown className="h-4 w-4" />
                        }
                      </button>
                    </CardHeader>
                    {showBenchmarkDetails && (
                      <CardContent>
                        <div className="space-y-2">
                          {benchmarkResults.per_query.map((qr, i) => (
                            <div
                              key={i}
                              className={`flex items-center gap-3 p-3 rounded-lg text-sm
                                ${qr.hit ? "bg-green-500/5 border border-green-500/20" : "bg-red-500/5 border border-red-500/20"}`}
                            >
                              {qr.hit ? (
                                <CheckCircle2 className="h-4 w-4 text-green-500 shrink-0" />
                              ) : (
                                <XCircle className="h-4 w-4 text-red-500 shrink-0" />
                              )}
                              <div className="flex-1 min-w-0">
                                <p className="font-medium truncate">{qr.query}</p>
                                <p className="text-xs text-muted-foreground mt-0.5">
                                  Top results: {qr.top_results.slice(0, 3).join(", ") || "none"}
                                </p>
                              </div>
                              <div className="text-right shrink-0">
                                {qr.matched_at_rank ? (
                                  <Badge variant="outline" className="text-[10px] bg-green-500/10 text-green-400 border-green-500/20">
                                    Rank #{qr.matched_at_rank}
                                  </Badge>
                                ) : (
                                  <Badge variant="outline" className="text-[10px] bg-red-500/10 text-red-400 border-red-500/20">
                                    Miss
                                  </Badge>
                                )}
                                <span className="text-[10px] text-muted-foreground ml-2">{qr.latency_ms}ms</span>
                              </div>
                            </div>
                          ))}
                        </div>
                      </CardContent>
                    )}
                  </Card>

                  {/* Weights Used */}
                  <Card>
                    <CardContent className="p-4">
                      <div className="flex items-center justify-between text-sm">
                        <span className="text-muted-foreground">Benchmark ID:</span>
                        <code className="text-xs bg-muted px-2 py-0.5 rounded">{benchmarkResults.benchmark_id}</code>
                      </div>
                      {benchmarkResults.label && (
                        <div className="flex items-center justify-between text-sm mt-2">
                          <span className="text-muted-foreground">Label:</span>
                          <Badge variant="outline">{benchmarkResults.label}</Badge>
                        </div>
                      )}
                      <div className="flex items-center justify-between text-sm mt-2">
                        <span className="text-muted-foreground">Weights:</span>
                        <span className="text-xs font-mono">
                          V:{(benchmarkResults.weights_used.vector * 100).toFixed(0)}%
                          T:{(benchmarkResults.weights_used.tree * 100).toFixed(0)}%
                          G:{(benchmarkResults.weights_used.graph * 100).toFixed(0)}%
                        </span>
                      </div>
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
          )}
        </div>
      </div>
    </div>
  );
}
