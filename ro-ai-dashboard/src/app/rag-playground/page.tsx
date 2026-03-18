"use client";

import { useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SourceBadge, SourceBadgeGroup, SourceLegend } from "@/components/ui/source-badge";
import { WeightSlider } from "@/components/ui/weight-slider";
import { GraphStatus } from "@/components/ui/graph-status";
import { Send, Loader2, Search, Sparkles, Database, TreePine, Share2, BarChart3 } from "lucide-react";

interface RetrievalResult {
  content: string;
  title: string;
  score: number;
  source_type: "vector" | "tree" | "graph";
  metadata?: Record<string, any>;
}

interface SearchResponse {
  answer: string;
  sources: { document_title: string; relevant_sections: string[]; source_type: string }[];
  mode_used: string;
  results?: RetrievalResult[];
  distribution?: { vector: number; tree: number; graph: number; total: number };
}

export default function RAGPlaygroundPage() {
  const [question, setQuestion] = useState("");
  const [mode, setMode] = useState<string>("hybrid");
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [weights, setWeights] = useState({ vector: 0.5, tree: 0.3, graph: 0.2 });
  const [searchHistory, setSearchHistory] = useState<{ question: string; mode: string; resultCount: number }[]>([]);

  const handleSearch = useCallback(async () => {
    if (!question.trim() || loading) return;

    setLoading(true);
    setError(null);

    try {
      const resp = await fetch("http://localhost:8080/api/v1/query", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Tenant-Id": "default_tenant",
        },
        body: JSON.stringify({
          question: question.trim(),
          mode,
          weights,
          limit: 10,
        }),
      });

      if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);

      const data: SearchResponse = await resp.json();
      setResults(data);
      setSearchHistory((prev) => [
        { question: question.trim(), mode: data.mode_used || mode, resultCount: data.sources?.length || 0 },
        ...prev.slice(0, 9),
      ]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to search");
    } finally {
      setLoading(false);
    }
  }, [question, mode, weights, loading]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSearch();
    }
  };

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
        <SourceLegend />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* Left Panel — Controls */}
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

          {/* Graph Status */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Data Sources</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Vector DB Status */}
              <div className="flex items-center justify-between">
                <span className="text-sm flex items-center gap-1.5">
                  🔷 Vector DB
                </span>
                <Badge variant="outline" className="text-[10px] px-1.5 h-5 bg-green-500/10 text-green-500 border-green-500/20">
                  Online
                </Badge>
              </div>

              {/* Tree Index Status */}
              <div className="flex items-center justify-between">
                <span className="text-sm flex items-center gap-1.5">
                  🌿 PageIndex
                </span>
                <Badge variant="outline" className="text-[10px] px-1.5 h-5 bg-green-500/10 text-green-500 border-green-500/20">
                  Online
                </Badge>
              </div>

              {/* Graph Status Component */}
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

        {/* Main Panel — Search + Results */}
        <div className="lg:col-span-3 space-y-6">
          {/* Search Bar */}
          <Card>
            <CardContent className="p-4">
              <div className="flex gap-3">
                <div className="relative flex-1">
                  <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                  <Input
                    value={question}
                    onChange={(e) => setQuestion(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Ask a question about your knowledge base..."
                    disabled={loading}
                    className="pl-10 h-12 text-base"
                  />
                </div>
                <Button
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
              {/* Answer Card */}
              {results.answer && (
                <Card className="border-primary/20">
                  <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-sm flex items-center gap-2">
                        <Sparkles className="h-4 w-4 text-primary" />
                        Answer
                      </CardTitle>
                      <Badge variant="outline" className="text-xs">
                        Mode: {results.mode_used}
                      </Badge>
                    </div>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm leading-relaxed">{results.answer}</p>
                  </CardContent>
                </Card>
              )}

              {/* Source Distribution */}
              {results.distribution && (
                <Card>
                  <CardHeader className="pb-3">
                    <CardTitle className="text-sm flex items-center gap-2">
                      <BarChart3 className="h-4 w-4" />
                      Source Distribution
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
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
              )}

              {/* Source Cards */}
              {results.sources && results.sources.length > 0 && (
                <div className="space-y-3">
                  <h3 className="text-sm font-medium text-muted-foreground">
                    Sources ({results.sources.length})
                  </h3>
                  {results.sources.map((source, i) => (
                    <Card key={i} className="hover:border-primary/30 transition-colors">
                      <CardContent className="p-4">
                        <div className="flex items-start justify-between mb-2">
                          <h4 className="font-medium text-sm">{source.document_title}</h4>
                          <SourceBadge sourceType={source.source_type} />
                        </div>
                        {source.relevant_sections && source.relevant_sections.length > 0 && (
                          <div className="space-y-1.5 mt-2">
                            {source.relevant_sections.slice(0, 3).map((section, j) => (
                              <p key={j} className="text-xs text-muted-foreground bg-muted/50 rounded px-2 py-1.5">
                                {section}
                              </p>
                            ))}
                            {source.relevant_sections.length > 3 && (
                              <p className="text-xs text-muted-foreground italic">
                                +{source.relevant_sections.length - 3} more sections
                              </p>
                            )}
                          </div>
                        )}
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
                <h3 className="text-lg font-semibold mb-2">
                  Search Your Knowledge Base
                </h3>
                <p className="text-muted-foreground max-w-md text-sm">
                  Ask a question to test hybrid retrieval across Vector (semantic search),
                  Tree (document structure), and Graph (entity relationships).
                </p>
                <div className="flex gap-2 mt-6">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setQuestion("What are the common side effects of Aspirin?")}
                  >
                    Try: Side effects
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setQuestion("How does the API authentication work?")}
                  >
                    Try: API auth
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
