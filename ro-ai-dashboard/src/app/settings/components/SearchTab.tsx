"use client";

import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Search, Save } from "lucide-react";
import { SettingsTabProps } from "./types";

export function SearchTab(props: SettingsTabProps) {
    const { isSaving, currentTenantId, updateTenantConfigFn,
        embeddingModel, setEmbeddingModel, topK, setTopK,
        similarityThreshold, setSimilarityThreshold, searchMode, setSearchMode } = props;

    return (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2"><Search className="w-5 h-5" /> Search & Retrieval Settings</CardTitle>
                <CardDescription>Configure embedding model, retrieval parameters, and search modes</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
                <div className="grid gap-2">
                    <label className="text-sm font-medium">Embedding Model</label>
                    <select value={embeddingModel} onChange={(e) => setEmbeddingModel(e.target.value)}
                        className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                        <option value="nomic-embed-text">nomic-embed-text (Ollama — local)</option>
                        <option value="text-embedding-3-small">text-embedding-3-small (OpenAI)</option>
                        <option value="text-embedding-3-large">text-embedding-3-large (OpenAI)</option>
                        <option value="text-embedding-004">text-embedding-004 (Google)</option>
                        <option value="bge-m3">bge-m3 (Ollama — multilingual)</option>
                    </select>
                    <p className="text-xs text-muted-foreground">Changing the model requires re-embedding all existing chunks</p>
                </div>
                <div className="grid grid-cols-2 gap-6">
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Top-K Results</label>
                        <Input type="number" min={1} max={50} value={topK} onChange={(e) => setTopK(parseInt(e.target.value) || 5)} />
                        <p className="text-xs text-muted-foreground">Number of similar chunks to retrieve (1-50)</p>
                    </div>
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Similarity Threshold</label>
                        <div className="flex items-center gap-3">
                            <input type="range" min={0} max={100} value={similarityThreshold * 100}
                                onChange={(e) => setSimilarityThreshold(parseInt(e.target.value) / 100)}
                                className="flex-1 h-2 bg-gray-200 dark:bg-zinc-700 rounded-lg appearance-none cursor-pointer" />
                            <span className="text-sm font-mono w-12 text-right">{similarityThreshold.toFixed(2)}</span>
                        </div>
                        <p className="text-xs text-muted-foreground">Minimum similarity score for results (0.0-1.0)</p>
                    </div>
                </div>
                <div className="grid gap-2">
                    <label className="text-sm font-medium">Search Mode</label>
                    <div className="grid grid-cols-3 gap-3">
                        {["semantic", "hybrid", "keyword"].map((mode) => (
                            <button key={mode} onClick={() => setSearchMode(mode)}
                                className={`p-3 rounded-lg border text-sm font-medium capitalize transition-colors ${searchMode === mode
                                    ? "border-blue-500 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400"
                                    : "border-border hover:bg-muted"}`}>
                                {mode === "semantic" && "🧠 "}{mode === "hybrid" && "🔀 "}{mode === "keyword" && "🔤 "}{mode}
                            </button>
                        ))}
                    </div>
                    <p className="text-xs text-muted-foreground">
                        {searchMode === "semantic" && "Pure vector similarity search using embeddings"}
                        {searchMode === "hybrid" && "Combines vector search, graph search, and SQL — best coverage"}
                        {searchMode === "keyword" && "Full-text keyword matching — fastest but least flexible"}
                    </p>
                </div>
                <div className="pt-4 border-t">
                    <Button onClick={async () => {
                        if (!currentTenantId) return;
                        try {
                            await updateTenantConfigFn(currentTenantId, {
                                search_settings: { embedding_model: embeddingModel, top_k: topK, similarity_threshold: similarityThreshold, search_mode: searchMode },
                            } as any);
                            alert("Search settings saved successfully.");
                        } catch { alert("Failed to save search settings."); }
                    }} disabled={isSaving || !currentTenantId}>
                        <Save className="w-4 h-4 mr-2" />
                        {isSaving ? "Saving..." : "Save Settings"}
                    </Button>
                </div>
            </CardContent>
        </Card>
    );
}
