"use client";

import React, { useEffect, useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { StatusBadge } from "@/components/ui/status-badge";
import { fetchPipelineOverview, runBatchPipeline, fetchModels, ModelConfig } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { RefreshCw, Play, Clock, Cpu, FileText, ChevronRight, Activity, AlertCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger, DialogFooter } from "@/components/ui/dialog";

export function PipelineDashboard() {
    const [overview, setOverview] = useState<any>(null);
    const [loading, setLoading] = useState(true);
    const [triggering, setTriggering] = useState(false);
    const [selected, setSelected] = useState<number[]>([]);
    
    const [providerOverride, setProviderOverride] = useState("");
    const [modelOverride, setModelOverride] = useState("");
    const [embeddingProviderOverride, setEmbeddingProviderOverride] = useState("");
    const [embeddingModelOverride, setEmbeddingModelOverride] = useState("");
    const [enableEmbedding, setEnableEmbedding] = useState(true);
    const [enableKg, setEnableKg] = useState(true);
    const [enableQa, setEnableQa] = useState(true);
    const [enablePageIndex, setEnablePageIndex] = useState(false);
    const [availableModels, setAvailableModels] = useState<ModelConfig[]>([]);
    const [showConfigModal, setShowConfigModal] = useState(false);

    const load = async () => {
        try {
            const data = await fetchPipelineOverview();
            setOverview(data);
        } catch (err) {
            console.error("Pipeline overview error:", err);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        load();
        fetchModels().then(setAvailableModels).catch(console.error);
        const interval = setInterval(load, 5000); // 5 sec auto refresh
        return () => clearInterval(interval);
    }, []);

    const handleTrigger = async () => {
        setTriggering(true);
        try {
            const p = providerOverride.trim() || undefined;
            const m = modelOverride.trim() || undefined;
            const ep = embeddingProviderOverride.trim() || undefined;
            const em = embeddingModelOverride.trim() || undefined;
            
            // If no specific sources are selected, we assume the user wants to process ALL pending sources.
            const processAll = selected.length === 0;
            
            await runBatchPipeline(selected.length > 0 ? selected : undefined, processAll, p, m, ep, em, enableEmbedding, enableKg, enableQa, enablePageIndex);
            setSelected([]);
            setProviderOverride("");
            setModelOverride("");
            setEmbeddingProviderOverride("");
            setEmbeddingModelOverride("");
            setShowConfigModal(false);
            await load();
        } catch (err) {
            console.error("Failed to trigger batch pipeline", err);
        } finally {
            setTriggering(false);
        }
    };

    if (!overview && loading) {
        return <div className="p-8 text-center"><RefreshCw className="animate-spin w-8 h-8 mx-auto text-muted-foreground" /></div>;
    }

    if (!overview) return <div className="p-8 text-center text-red-500">Failed to load pipeline stats</div>;

    return (
        <div className="grid gap-6">
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                <Card>
                    <CardContent className="p-6 flex flex-col justify-between h-full">
                        <div className="text-sm font-medium text-muted-foreground mb-2">Pending Sources</div>
                        <div className="text-3xl font-bold">{overview.pending_sources}</div>
                        <div className="text-xs text-muted-foreground mt-2">out of {overview.total_sources} total</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardContent className="p-6 flex flex-col justify-between h-full">
                        <div className="text-sm font-medium text-muted-foreground mb-2">Total ETA</div>
                        <div className="text-3xl font-bold">{overview.total_estimate_human || "0m"}</div>
                        <div className="text-xs text-muted-foreground mt-2">remaining processing time</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardContent className="p-6 flex flex-col justify-between h-full">
                        <div className="text-sm font-medium text-muted-foreground mb-2">Speed (Avg)</div>
                        <div className="text-3xl font-bold">{(overview.avg_ms_per_chunk / 1000).toFixed(1)}s</div>
                        <div className="text-xs text-muted-foreground mt-2">per structural chunk</div>
                    </CardContent>
                </Card>
                <Card className="bg-primary/5 border-primary/20">
                        <div className="flex flex-col h-full justify-center">
                            <Dialog open={showConfigModal} onOpenChange={setShowConfigModal}>
                                <DialogTrigger asChild>
                                    <Button 
                                        className="w-full h-12 text-sm font-semibold shadow-md border-primary/20" 
                                        disabled={(overview.pending_sources === 0 && selected.length === 0) || triggering}
                                    >
                                        <Play className="w-5 h-5 mr-2" />
                                        {selected.length > 0 ? `Configure & Start Selected (${selected.length})` : "Configure & Start Batch Process"}
                                    </Button>
                                </DialogTrigger>
                                <DialogContent className="sm:max-w-[425px]">
                                    <DialogHeader>
                                        <DialogTitle className="flex items-center gap-2"><Cpu className="w-5 h-5 text-primary"/> Batch Engine Setup</DialogTitle>
                                        <DialogDescription>
                                            Override default models for this batch execution.
                                        </DialogDescription>
                                    </DialogHeader>
                                    
                                    <div className="grid gap-4 py-4">
                                        <div className="grid gap-4">
                                            <div className="grid gap-2">
                                                <label className="text-sm font-medium">Provider</label>
                                                <select 
                                                    className="h-10 px-3 rounded-md border bg-background text-sm"
                                                    value={providerOverride} 
                                                    onChange={e => { setProviderOverride(e.target.value); setModelOverride(""); }}
                                                >
                                                    <option value="">Default ({overview?.default_provider || "Provider"})</option>
                                                    {Array.from(new Set(availableModels.map(m => m.provider))).map(p => (
                                                        <option key={p} value={p}>{p}</option>
                                                    ))}
                                                </select>
                                            </div>
                                            <div className="grid gap-2">
                                                <label className="text-sm font-medium">Model ID</label>
                                                <select 
                                                    className="h-10 px-3 rounded-md border bg-background text-sm"
                                                    value={modelOverride}
                                                    onChange={e => setModelOverride(e.target.value)}
                                                >
                                                    <option value="">Default ({overview?.default_model || "Model"})</option>
                                                    {availableModels.filter(m => m.provider === (providerOverride || overview?.default_provider)).map(m => (
                                                        <option key={m.model_id} value={m.model_id}>{m.model_id}</option>
                                                    ))}
                                                </select>
                                            </div>
                                        </div>
                                        <p className="text-xs text-muted-foreground -mt-2">This AI Model will be used for both Knowledge Graph and QA Extractor steps.</p>

                                        <div className="text-sm font-medium mt-4 mb-2">Select pipeline steps to execute:</div>

                                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                                            <label className="flex items-start space-x-3 cursor-pointer">
                                                <input 
                                                    type="checkbox" 
                                                    className="form-checkbox mt-1 text-primary focus:ring-primary h-4 w-4" 
                                                    checked={enableEmbedding} 
                                                    onChange={(e) => setEnableEmbedding(e.target.checked)} 
                                                />
                                                <div>
                                                    <span className="font-medium text-sm">Text Chunking & Embedding</span>
                                                    <p className="text-xs text-muted-foreground mt-0.5">Parse, chunk, and encode document text into the vector database.</p>
                                                </div>
                                            </label>
                                        </div>

                                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                                            <label className="flex items-start space-x-3 cursor-pointer">
                                                <input 
                                                    type="checkbox" 
                                                    className="form-checkbox mt-1 text-primary focus:ring-primary h-4 w-4" 
                                                    checked={enableKg} 
                                                    onChange={(e) => setEnableKg(e.target.checked)} 
                                                />
                                                <div>
                                                    <span className="font-medium text-sm">Knowledge Graph Extraction</span>
                                                    <p className="text-xs text-muted-foreground mt-0.5">Extract entities & relations using LLM to build a knowledge graph.</p>
                                                </div>
                                            </label>
                                        </div>

                                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                                            <label className="flex items-start space-x-3 cursor-pointer">
                                                <input 
                                                    type="checkbox" 
                                                    className="form-checkbox mt-1 text-primary focus:ring-primary h-4 w-4" 
                                                    checked={enableQa} 
                                                    onChange={(e) => setEnableQa(e.target.checked)} 
                                                />
                                                <div>
                                                    <span className="font-medium text-sm">Synthetic Q&A Generation</span>
                                                    <p className="text-xs text-muted-foreground mt-0.5">Generate high-quality question-answer pairs for better semantic retrieval.</p>
                                                </div>
                                            </label>
                                        </div>

                                        <div className="border rounded-md p-3 bg-secondary/30 mt-2">
                                            <label className="flex items-start space-x-3 cursor-pointer">
                                                <input 
                                                    type="checkbox" 
                                                    className="form-checkbox mt-1 text-primary focus:ring-primary h-4 w-4" 
                                                    checked={enablePageIndex} 
                                                    onChange={(e) => setEnablePageIndex(e.target.checked)} 
                                                />
                                                <div>
                                                    <span className="font-medium text-sm">Hierarchical PageIndex Tree <span className="text-orange-500">(Advanced)</span></span>
                                                    <p className="text-xs text-muted-foreground mt-0.5">Generate a semantic hierarchy tree of the entire document. Consumes significant LLM tokens.</p>
                                                </div>
                                            </label>
                                        </div>
                                    </div>
                                    
                                    <DialogFooter>
                                        <Button variant="outline" onClick={() => setShowConfigModal(false)}>Cancel</Button>
                                        <Button onClick={handleTrigger} disabled={triggering}>
                                            {triggering ? <RefreshCw className="w-4 h-4 mr-2 animate-spin" /> : <Play className="w-4 h-4 mr-2" />}
                                            Start Batch
                                        </Button>
                                    </DialogFooter>
                                </DialogContent>
                            </Dialog>
                        </div>
                </Card>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle className="text-lg flex items-center gap-2">
                        <Activity className="w-5 h-5 text-blue-500" /> Source Progress Queue
                    </CardTitle>
                </CardHeader>
                <CardContent className="p-0">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead className="w-12 text-center">
                                    <input 
                                        type="checkbox" 
                                        className="w-4 h-4 rounded border-gray-300 accent-primary" 
                                        checked={selected.length === overview.sources.length && overview.sources.length > 0}
                                        onChange={(e) => {
                                            if (e.target.checked) {
                                                setSelected(overview.sources.map((s: any) => s.source_id));
                                            } else {
                                                setSelected([]);
                                            }
                                        }}
                                    />
                                </TableHead>
                                <TableHead>Source</TableHead>
                                <TableHead>Status</TableHead>
                                <TableHead>Progress Detail</TableHead>
                                <TableHead>Total Time / ETA</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {overview.sources.length === 0 && (
                                <TableRow>
                                    <TableCell colSpan={5} className="text-center py-8 text-muted-foreground">
                                        No sources available for pipeline processing.
                                    </TableCell>
                                </TableRow>
                            )}
                            {overview.sources.map((s: any) => (
                                <React.Fragment key={s.source_id}>
                                    <TableRow className={s.pipeline?.status === "running" ? "bg-blue-50/50 dark:bg-blue-900/10" : selected.includes(s.source_id) ? "bg-muted/50" : ""}>
                                        <TableCell className="text-center">
                                            <input 
                                                type="checkbox" 
                                                className="w-4 h-4 rounded border-gray-300 accent-primary"
                                                checked={selected.includes(s.source_id)}
                                                onChange={(e) => {
                                                    if (e.target.checked) setSelected([...selected, s.source_id]);
                                                    else setSelected(selected.filter(id => id !== s.source_id));
                                                }}
                                            />
                                        </TableCell>
                                        <TableCell>
                                            <div className="font-medium">{s.name}</div>
                                            <div className="text-xs text-muted-foreground mt-1 flex items-center gap-2">
                                                <Badge variant="outline" className="text-[10px] font-mono">
                                                    Chunks: {s.chunks} | KG: {s.kg?.entities}e / {s.kg?.relations}r
                                                </Badge>
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            <StatusBadge status={s.pipeline?.status || "never_run"} />
                                        </TableCell>
                                        <TableCell>
                                            {s.pipeline?.status === "never_run" ? (
                                                <span className="text-sm text-muted-foreground">Waiting for batch...</span>
                                            ) : (
                                                <div className="space-y-1">
                                                    {s.steps?.map((st: any) => (
                                                        <div key={st.step} className="flex items-center gap-2 text-xs">
                                                            <div className="w-28 font-medium text-slate-600 dark:text-slate-400 truncate">{st.name}</div>
                                                            <div className="w-20"><StatusBadge status={st.status} /></div>
                                                            
                                                            {/* STEP LEVEL MODEL/PROVIDER */}
                                                            {st.status !== "skipped" && (st.model || st.provider) && (
                                                                <div className="text-slate-400 font-mono text-[10px] flex items-center bg-slate-100 dark:bg-slate-800 px-2 py-0.5 rounded">
                                                                    <Cpu className="w-3 h-3 mr-1" />
                                                                    {st.provider && <span className="text-blue-500 mr-1">{st.provider}:</span>}
                                                                    <span className="truncate max-w-[150px]">{st.model || "unknown"}</span>
                                                                </div>
                                                            )}
                                                            {st.error && (
                                                                <div className="text-red-500 truncate max-w-[150px]" title={st.error}>
                                                                    <AlertCircle className="w-3 h-3 inline mr-1" />Error
                                                                </div>
                                                            )}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <div className="flex flex-col gap-1">
                                                {s.pipeline?.status === "running" && s.estimate_human && (
                                                    <span className="text-sm font-medium text-blue-600 flex items-center">
                                                        <Clock className="w-3 h-3 mr-1 animate-pulse" /> ETA: {s.estimate_human}
                                                    </span>
                                                )}
                                                {s.actual_duration_ms && (
                                                    <span className="text-sm text-muted-foreground">
                                                        Took: {Math.round(s.actual_duration_ms / 1000)}s
                                                    </span>
                                                )}
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                </React.Fragment>
                            ))}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>
        </div>
    );
}
