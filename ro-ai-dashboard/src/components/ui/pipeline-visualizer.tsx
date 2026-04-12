"use client";

import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Database, TreePine, Share2, Target, Layers,
  CheckCircle2, XCircle, Clock, AlertTriangle, SkipForward
} from "lucide-react";

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";

// ── Types ──────────────────────────────────────────

export interface TraceEvent {
  step: string;
  status: string; // "success" | "timeout" | "error" | "skipped"
  duration_ms: number;
  parameters: Record<string, any>;
  input_summary: string;
  output_summary: string;
  items_in: number;
  items_out: number;
}

export interface PipelineVisualizerProps {
  traceLog: TraceEvent[];
  totalLatencyMs: number;
  isLoading?: boolean;
  playgroundState?: {
    mode: string;
    weights: { vector: number; tree: number; graph: number };
    searchProvider: string;
    searchModelId: string;
    evalProvider: string;
    evalModelId: string;
    generateLLM: boolean;
    rerankStrategy: string;
    hopLimit: number;
    alpha: number;
    threshold: number;
    availableModels: any[];
  };
  onConfigChange?: (key: string, value: any) => void;
  onReRun?: () => void;
}

// ── Step Styling ───────────────────────────────────

const STEP_CONFIG: Record<string, { icon: React.ReactNode; color: string; bgColor: string }> = {
  "Vector Search": {
    icon: <Database className="h-4 w-4" />,
    color: "text-blue-400",
    bgColor: "border-blue-500/30 bg-blue-500/5",
  },
  "Graph Search": {
    icon: <Share2 className="h-4 w-4" />,
    color: "text-purple-400",
    bgColor: "border-purple-500/30 bg-purple-500/5",
  },
  "Unified Candidate Pool": {
    icon: <Layers className="h-4 w-4" />,
    color: "text-emerald-400",
    bgColor: "border-emerald-500/30 bg-emerald-500/5",
  },
  "Tree Search": {
    icon: <TreePine className="h-4 w-4" />,
    color: "text-green-400",
    bgColor: "border-green-500/30 bg-green-500/5",
  },
  "Reranking": {
    icon: <Target className="h-4 w-4" />,
    color: "text-pink-400",
    bgColor: "border-pink-500/30 bg-pink-500/5",
  },
};

const STATUS_CONFIG: Record<string, { icon: React.ReactNode; color: string; label: string }> = {
  success: { icon: <CheckCircle2 className="h-3.5 w-3.5" />, color: "text-green-500", label: "Success" },
  timeout: { icon: <AlertTriangle className="h-3.5 w-3.5" />, color: "text-amber-500", label: "Timeout" },
  error:   { icon: <XCircle className="h-3.5 w-3.5" />, color: "text-red-500", label: "Error" },
  skipped: { icon: <SkipForward className="h-3.5 w-3.5" />, color: "text-muted-foreground", label: "Skipped" },
  running: { icon: <span className="h-2.5 w-2.5 rounded-full bg-primary animate-pulse" />, color: "text-primary", label: "Running" },
};

const DEFAULT_STEP = {
  icon: <Layers className="h-4 w-4" />,
  color: "text-muted-foreground",
  bgColor: "border-border bg-muted/5",
};

// ── Trace Node Component ──────────────────────────

interface TraceNodeProps {
  event: TraceEvent;
  isParallel?: boolean;
  idx?: number;
  isLoading?: boolean;
  isSelected?: boolean;
  onClick?: () => void;
}

function TraceNode({ event, isParallel, idx = 0, isLoading, isSelected, onClick }: TraceNodeProps) {
  const stepConfig = STEP_CONFIG[event.step] || DEFAULT_STEP;
  const statusConfig = STATUS_CONFIG[event.status] || STATUS_CONFIG.success;
  
  // Create a staggered pulse effect based on node index when loading
  const pulseClass = isLoading ? "animate-pulse" : "";
  const delayStyle = isLoading ? { animationDelay: `${idx * 150}ms` } : {};

  return (
    <div className={`${isParallel ? "flex-1 min-w-0" : "w-full max-w-sm mx-auto"} ${pulseClass}`} style={delayStyle}>
      <Card
        className={`${stepConfig.bgColor} cursor-pointer transition-all duration-200 hover:scale-[1.02] active:scale-[0.98] ${
          isLoading ? "border-primary/50 shadow-[0_0_15px_rgba(var(--primary),0.1)]" : ""
        } ${isSelected && !isLoading ? "ring-2 ring-primary ring-offset-2 ring-offset-background" : ""}`}
        onClick={() => !isLoading && onClick && onClick()}
      >
        <CardContent className="p-3">
          {/* Header */}
          <div className="flex items-center justify-between mb-1.5">
            <div className={`flex items-center gap-1.5 font-medium text-sm ${stepConfig.color}`}>
              {stepConfig.icon}
              <span>{event.step}</span>
            </div>
            <div className={`flex items-center gap-1 ${statusConfig.color}`}>
              {isLoading ? STATUS_CONFIG.running.icon : statusConfig.icon}
            </div>
          </div>

          {/* Quick Stats */}
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            {isLoading ? (
              <span className="flex items-center gap-1 text-primary animate-pulse">Running...</span>
            ) : (
              <>
                <span className="flex items-center gap-1">
                  <Clock className="h-3 w-3" />
                  {event.duration_ms.toLocaleString()}ms
                </span>
                {event.items_out > 0 && (
                  <span>{event.items_in} → {event.items_out} items</span>
                )}
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

// ── Event Detail Panel ────────────────────────────

function EventDetailPanel({ 
  event, 
  playgroundState, 
  onConfigChange, 
  onReRun 
}: { 
  event: TraceEvent | null; 
  playgroundState?: PipelineVisualizerProps['playgroundState'];
  onConfigChange?: PipelineVisualizerProps['onConfigChange'];
  onReRun?: PipelineVisualizerProps['onReRun'];
}) {
  if (!event) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-muted-foreground bg-muted/5 border border-dashed rounded-lg p-6 min-h-[400px]">
        <Layers className="h-8 w-8 mb-3 opacity-20" />
        <p className="text-sm">Select a pipeline step</p>
        <p className="text-xs opacity-70">to view parameters and I/O</p>
      </div>
    );
  }

  const stepConfig = STEP_CONFIG[event.step] || DEFAULT_STEP;

  return (
    <div className="h-full bg-card border rounded-lg overflow-hidden flex flex-col shadow-sm min-h-[400px]">
      <div className={`${stepConfig.bgColor} border-b px-4 py-3 flex items-center gap-2`}>
        <div className={stepConfig.color}>{stepConfig.icon}</div>
        <h3 className="font-semibold text-sm">{event.step}</h3>
      </div>
      
      <div className="flex-1 overflow-y-auto">
        {/* SECTION 1: Last Execution (Read-Only) */}
        <div className="p-4 border-b">
          <div className="text-xs font-semibold text-muted-foreground mb-3 flex items-center gap-1.5">
            <span>▶</span> Last Execution
          </div>
          
          <div className="space-y-4 text-sm">
            {Object.keys(event.parameters).length > 0 && (
              <div className="bg-muted/30 border rounded-md p-3 font-mono text-[11px] space-y-1.5">
                {Object.entries(event.parameters).map(([k, v]) => (
                  <div key={k} className="flex flex-col sm:flex-row sm:gap-2 border-b border-border/40 last:border-0 pb-1.5 last:pb-0">
                    <span className="text-muted-foreground font-semibold min-w-24">{k}:</span>
                    <span className="text-foreground/90 break-words">{typeof v === "object" ? JSON.stringify(v) : String(v)}</span>
                  </div>
                ))}
              </div>
            )}
            
            {event.input_summary && (
              <div>
                <div className="text-xs text-muted-foreground mb-1.5">Input Data:</div>
                <div className="bg-muted/30 border rounded-md p-3 font-mono text-[11px] text-foreground/80 break-words whitespace-pre-wrap">
                  {event.input_summary.split('\n').map((line, i) => (
                    <div key={i} className={line.trim().startsWith('•') ? 'pl-2 text-primary/80' : ''}>
                      {line}
                    </div>
                  ))}
                </div>
              </div>
            )}
            
            {event.output_summary && (
              <div>
                <div className="text-xs text-muted-foreground mb-1.5">Output Data:</div>
                <div className="bg-muted/30 border rounded-md p-3 font-mono text-[11px] text-foreground/80 break-words whitespace-pre-wrap">
                  {event.output_summary.split('\n').map((line, i) => (
                    <div key={i} className={line.trim().startsWith('•') ? 'pl-2 text-primary/80' : ''}>
                      {line}
                    </div>
                  ))}
                </div>
              </div>
            )}
            
            {Object.keys(event.parameters).length === 0 && !event.output_summary && (
              <div className="text-xs text-muted-foreground italic">No telemetry recorded for this step.</div>
            )}
          </div>
        </div>

        {/* SECTION 2: Configure Next Run (Editable) */}
        {playgroundState && onConfigChange && (
          <div className="p-4 bg-muted/5 pb-6">
            <div className="text-xs font-semibold text-primary mb-3 flex items-center gap-1.5">
              <span>⚙️</span> Configure Next Run
            </div>
            
            <div className="space-y-4">
              {event.step === "Vector Search" && (
                <>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-[11px]">Weight ({Math.round(playgroundState.weights.vector * 100)}%)</Label>
                    </div>
                    <input type="range" min={0} max={100} step={5} value={playgroundState.weights.vector * 100} onChange={(e) => onConfigChange('weights', { ...playgroundState.weights, vector: parseInt(e.target.value) / 100 })} className="w-full h-1.5 rounded-lg appearance-none cursor-pointer accent-blue-500 bg-muted" />
                  </div>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-[11px]">Alpha (Dense/Sparse Blend) - {playgroundState.alpha.toFixed(2)}</Label>
                    </div>
                    <input type="range" min={0} max={100} step={5} value={playgroundState.alpha * 100} onChange={(e) => onConfigChange('alpha', parseInt(e.target.value) / 100)} className="w-full h-1.5 rounded-lg appearance-none cursor-pointer accent-primary bg-muted" />
                  </div>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-[11px]">Threshold (Min Score) - {playgroundState.threshold.toFixed(2)}</Label>
                    </div>
                    <input type="range" min={0} max={100} step={5} value={playgroundState.threshold * 100} onChange={(e) => onConfigChange('threshold', parseInt(e.target.value) / 100)} className="w-full h-1.5 rounded-lg appearance-none cursor-pointer accent-primary bg-muted" />
                  </div>
                </>
              )}

              {event.step === "Graph Search" && (
                <>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-[11px]">Weight ({Math.round(playgroundState.weights.graph * 100)}%)</Label>
                    </div>
                    <input type="range" min={0} max={100} step={5} value={playgroundState.weights.graph * 100} onChange={(e) => onConfigChange('weights', { ...playgroundState.weights, graph: parseInt(e.target.value) / 100 })} className="w-full h-1.5 rounded-lg appearance-none cursor-pointer accent-purple-500 bg-muted" />
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-[11px]">Expansion Hops</Label>
                    <Select value={String(playgroundState.hopLimit)} onValueChange={(v) => onConfigChange('hopLimit', parseInt(v))}>
                      <SelectTrigger className="h-7 text-xs"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="1">1 hop</SelectItem>
                        <SelectItem value="2">2 hops</SelectItem>
                        <SelectItem value="3">3 hops</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </>
              )}

              {event.step === "Tree Search" && (
                <>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label className="text-[11px]">Weight ({Math.round(playgroundState.weights.tree * 100)}%)</Label>
                    </div>
                    <input type="range" min={0} max={100} step={5} value={playgroundState.weights.tree * 100} onChange={(e) => onConfigChange('weights', { ...playgroundState.weights, tree: parseInt(e.target.value) / 100 })} className="w-full h-1.5 rounded-lg appearance-none cursor-pointer accent-green-500 bg-muted" />
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-[11px]">Provider</Label>
                    <Select value={playgroundState.searchProvider} onValueChange={(v) => onConfigChange('searchProvider', v)}>
                      <SelectTrigger className="h-7 text-xs"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="default">Tenant Default</SelectItem>
                        <SelectItem value="google">Google</SelectItem>
                        <SelectItem value="anthropic">Anthropic</SelectItem>
                        <SelectItem value="openai">OpenAI</SelectItem>
                        <SelectItem value="ollama">Ollama</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-[11px]">Model (Optional)</Label>
                    <Select value={playgroundState.searchModelId || "default"} onValueChange={(v) => onConfigChange('searchModelId', v === "default" ? "" : v)}>
                      <SelectTrigger className="h-7 text-xs"><SelectValue placeholder="Tenant Default" /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="default">Tenant Default</SelectItem>
                        {playgroundState.availableModels
                          .filter(m => m.provider === playgroundState.searchProvider)
                          .map(m => (
                          <SelectItem key={m.id} value={m.id}>{m.name}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </>
              )}

              {event.step === "Reranking" && (
                <div className="space-y-1.5">
                  <Label className="text-[11px]">Rerank Strategy</Label>
                  <Select value={playgroundState.rerankStrategy} onValueChange={(v) => onConfigChange('rerankStrategy', v)}>
                    <SelectTrigger className="h-7 text-xs"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="none">None (Raw Unified Score)</SelectItem>
                      <SelectItem value="weighted">Weighted Combine</SelectItem>
                      <SelectItem value="rrf">Reciprocal Rank Fusion (RRF)</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              )}

              {event.step === "Unified Candidate Pool" && (
                <div className="text-xs text-muted-foreground italic bg-muted/30 p-3 rounded text-center">
                  This is a merge step. Configure individual sources above.
                </div>
              )}

              {/* Re-Run Button */}
              {onReRun && (
                <div className="pt-2">
                  <Button onClick={onReRun} size="sm" className="w-full text-xs h-8">
                    🔁 Re-Run Search
                  </Button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Connector Line ────────────────────────────────

function Connector() {
  return (
    <div className="flex justify-center py-1">
      <div className="w-px h-6 bg-border/60" />
    </div>
  );
}

// ── Main Pipeline Visualizer ──────────────────────

export function PipelineVisualizer({ traceLog, totalLatencyMs, isLoading, playgroundState, onConfigChange, onReRun }: PipelineVisualizerProps) {
  const [selectedEventIndex, setSelectedEventIndex] = useState<number | null>(null);

  // Auto-select the last step when trace data arrives (so the right panel isn't empty)
  useEffect(() => {
    if (traceLog && traceLog.length > 0 && !isLoading) {
      setSelectedEventIndex(traceLog.length - 1);
    } else {
      setSelectedEventIndex(null);
    }
  }, [traceLog, isLoading]);

  // If no traceLog is provided yet, show a skeleton layout of the pipeline
  const isPending = !traceLog || traceLog.length === 0;

  const displayLog = isPending
    ? [
        { step: "Vector Search", status: "skipped", duration_ms: 0, parameters: {}, input_summary: "", output_summary: "", items_in: 0, items_out: 0 },
        { step: "Graph Search", status: "skipped", duration_ms: 0, parameters: {}, input_summary: "", output_summary: "", items_in: 0, items_out: 0 },
        { step: "Unified Candidate Pool", status: "skipped", duration_ms: 0, parameters: {}, input_summary: "", output_summary: "", items_in: 0, items_out: 0 },
        { step: "Tree Search", status: "skipped", duration_ms: 0, parameters: {}, input_summary: "", output_summary: "", items_in: 0, items_out: 0 },
        { step: "Reranking", status: "skipped", duration_ms: 0, parameters: {}, input_summary: "", output_summary: "", items_in: 0, items_out: 0 },
      ]
    : traceLog;

  // Separate parallel Stage 1 events from sequential events
  const parallelSteps = displayLog.filter(e => 
    e.step === "Vector Search" || e.step === "Graph Search"
  );
  const sequentialSteps = displayLog.filter(e => 
    e.step !== "Vector Search" && e.step !== "Graph Search"
  );

  const handleNodeClick = (actualIndex: number) => {
    setSelectedEventIndex(selectedEventIndex === actualIndex ? null : actualIndex);
  };

  const selectedEvent = selectedEventIndex !== null && !isPending ? displayLog[selectedEventIndex] : null;

  return (
    <Card className="border-dashed border-primary/20">
      <CardContent className="p-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <span>📊</span>
            Pipeline Trace {isPending && <span className="text-muted-foreground font-normal ml-2">(Pending Execution)</span>}
          </div>
          <div className="flex items-center gap-2">
            {onReRun && !isLoading && (
              <Button size="sm" variant="outline" onClick={onReRun} className="h-7 text-xs">
                🔁 Re-Run
              </Button>
            )}
            {!isPending && (
              <Badge variant="outline" className="text-[10px] font-mono">
                {totalLatencyMs.toLocaleString()}ms total
              </Badge>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
          {/* Left: Pipeline Diagram */}
          <div className="lg:col-span-3 bg-muted/10 border border-dashed rounded-xl p-4 md:p-6">
            {/* Stage 1: Parallel nodes side-by-side */}
            {parallelSteps.length > 0 && (
              <>
                <div className="text-[10px] text-muted-foreground text-center mb-2 font-medium tracking-wider uppercase">
                  ⚡ Stage 1 — Parallel Retrieval
                </div>
                <div className="flex gap-3">
                  {parallelSteps.map((event, i) => {
                    const actualIndex = displayLog.findIndex(e => e.step === event.step);
                    return (
                      <TraceNode 
                        key={i} 
                        event={event as TraceEvent} 
                        isParallel 
                        idx={i} 
                        isLoading={isLoading} 
                        isSelected={selectedEventIndex === actualIndex}
                        onClick={() => handleNodeClick(actualIndex)}
                      />
                    );
                  })}
                </div>
              </>
            )}

            {/* Sequential stages */}
            {sequentialSteps.map((event, i) => {
              const actualIndex = displayLog.findIndex(e => e.step === event.step);
              return (
                <div key={i}>
                  <Connector />
                  <TraceNode 
                    event={event as TraceEvent} 
                    idx={i + parallelSteps.length} 
                    isLoading={isLoading}
                    isSelected={selectedEventIndex === actualIndex}
                    onClick={() => handleNodeClick(actualIndex)}
                  />
                </div>
              );
            })}
          </div>

          {/* Right: Detailed Parameters & Input/Output */}
            <div className="lg:col-span-2 relative">
             <div className="sticky top-6">
               <EventDetailPanel 
                 event={selectedEvent as TraceEvent | null} 
                 playgroundState={playgroundState}
                 onConfigChange={onConfigChange}
                 onReRun={onReRun}
               />
             </div>
          </div>
        </div>

        {/* Duration Breakdown Bar */}
        <div className="mt-4 pt-3 border-t border-border/30">
          <div className="text-[10px] text-muted-foreground mb-2 font-medium">Latency Breakdown</div>
          <div className="space-y-1.5">
            {(traceLog || []).filter(e => e.duration_ms > 0).map((event, i) => {
              const pct = (event.duration_ms / totalLatencyMs) * 100;
              const stepConfig = STEP_CONFIG[event.step] || DEFAULT_STEP;
              return (
                <div key={i} className={`flex items-center gap-2 text-[11px] ${stepConfig.color}`}>
                  <span className="w-28 truncate">{event.step}</span>
                  <div className="flex-1 h-2 rounded-full bg-muted/30 overflow-hidden">
                    <div
                      className="h-full rounded-full bg-current opacity-60 transition-all duration-500"
                      style={{ width: `${Math.max(pct, 2)}%` }}
                    />
                  </div>
                  <span className="w-16 text-right text-muted-foreground font-mono">
                    {event.duration_ms.toLocaleString()}ms
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
