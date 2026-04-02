"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Workflow, Save, Check } from "lucide-react";
import { SettingsTabProps } from "./types";

export function PipelineTab(props: SettingsTabProps) {
    const { config, setConfig, isSaving, currentTenantId, updateTenantConfigFn,
        chunkStrategy, setChunkStrategy, chunkSize, setChunkSize, chunkOverlap, setChunkOverlap, dedupThreshold, setDedupThreshold } = props;
    const [saved, setSaved] = useState(false);

    return (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Workflow className="w-5 h-5 text-primary" />
                    Pipeline Settings
                </CardTitle>
                <CardDescription>
                    Configure chunking strategy, extraction settings, crawl limits, and deduplication threshold.
                </CardDescription>
            </CardHeader>
            <CardContent>
                <div className="space-y-6">
                    <div className="space-y-2">
                        <label className="text-sm font-medium">Max Crawl Pages</label>
                        <Input
                            type="number"
                            value={config?.max_crawl_pages ?? 100}
                            onChange={e => {
                                if (config) setConfig({ ...config, max_crawl_pages: Math.max(10, Math.min(500, parseInt(e.target.value) || 100)) });
                            }}
                            min={10} max={500}
                        />
                        <p className="text-xs text-muted-foreground">
                            จำนวนหน้าสูงสุดที่ Web Hierarchy Loader จะ crawl (10–500, default: 100)
                        </p>
                    </div>

                    <div className="space-y-2">
                        <label className="text-sm font-medium">Chunking Strategy</label>
                        <select value={chunkStrategy} onChange={e => setChunkStrategy(e.target.value)}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2">
                            <option value="auto">Auto (recommended)</option>
                            <option value="fixed">Fixed Size</option>
                            <option value="recursive">Recursive (Markdown-aware)</option>
                        </select>
                        <p className="text-xs text-muted-foreground">Auto mode selects the best strategy based on content type.</p>
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Chunk Size (chars)</label>
                            <Input type="number" value={chunkSize} onChange={e => setChunkSize(parseInt(e.target.value) || 512)} min={100} max={4000} />
                        </div>
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Chunk Overlap (chars)</label>
                            <Input type="number" value={chunkOverlap} onChange={e => setChunkOverlap(parseInt(e.target.value) || 0)} min={0} max={500} />
                        </div>
                    </div>

                    <div className="space-y-2">
                        <label className="text-sm font-medium">Dedup Strategy</label>
                        <select value={dedupThreshold} onChange={e => setDedupThreshold(parseInt(e.target.value))}
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2">
                            <option value={0}>Exact Match (SHA-256)</option>
                        </select>
                        <p className="text-xs text-muted-foreground">Duplicate chunks with identical content hash are automatically skipped during sync.</p>
                    </div>

                    <div className="pt-4 flex justify-end">
                        <Button
                            disabled={isSaving || !currentTenantId || saved}
                            className={saved ? "bg-green-600 hover:bg-green-700 text-white" : ""}
                            onClick={async () => {
                                if (!currentTenantId || !config) return;
                                try { 
                                    await updateTenantConfigFn(currentTenantId, { 
                                        max_crawl_pages: config.max_crawl_pages,
                                        pipeline_settings: {
                                            ...(config.pipeline_settings || {}),
                                            chunk_strategy: chunkStrategy,
                                            chunk_size: chunkSize,
                                            chunk_overlap: chunkOverlap,
                                            dedup_threshold: dedupThreshold,
                                        },
                                    }); 
                                    setSaved(true);
                                    setTimeout(() => setSaved(false), 2000);
                                }
                                catch { alert("Failed to save pipeline settings."); }
                            }}
                        >
                            {saved ? <Check className="w-4 h-4 mr-2" /> : <Save className="w-4 h-4 mr-2" />}
                            {saved ? "Saved Successfully" : isSaving ? "Saving..." : "Save Pipeline Settings"}
                        </Button>
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
