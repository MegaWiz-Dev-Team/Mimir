"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { SettingsTabProps } from "./types";
import { fetchHealth, triggerPrimeKGEmbed, fetchPrimeKGEmbedStatus, PrimeKGEmbedStatus } from "@/lib/api";

export function KnowledgeGraphTab({ isLoading }: SettingsTabProps) {
    const [healthStatus, setHealthStatus] = useState<string>("Checking...");
    const [embedStatus, setEmbedStatus] = useState<PrimeKGEmbedStatus | null>(null);
    const [embedLoading, setEmbedLoading] = useState(false);

    useEffect(() => {
        fetchHealth()
            .then(data => {
                if (data && data.status === "ok") {
                    setHealthStatus(`Connected (Backend v${data.version || "unknown"})`);
                } else {
                    setHealthStatus("Disconnected");
                }
            })
            .catch(() => setHealthStatus("Disconnected (API Unreachable)"));
    }, []);

    const refreshEmbedStatus = useCallback(() => {
        fetchPrimeKGEmbedStatus()
            .then(setEmbedStatus)
            .catch(() => {});
    }, []);

    useEffect(() => {
        refreshEmbedStatus();
    }, [refreshEmbedStatus]);

    // Poll while running
    useEffect(() => {
        if (embedStatus?.status !== "running") return;
        const id = setInterval(refreshEmbedStatus, 3000);
        return () => clearInterval(id);
    }, [embedStatus?.status, refreshEmbedStatus]);

    const handleEmbed = async (dryRun: boolean) => {
        setEmbedLoading(true);
        try {
            const result = await triggerPrimeKGEmbed({ batch_size: 500, dry_run: dryRun });
            setEmbedStatus(result);
        } catch (e) {
            setEmbedStatus({ status: "failed", embedded: 0, total: 0, errors: 1, message: String(e) });
        } finally {
            setEmbedLoading(false);
        }
    };

    if (isLoading) {
        return <div className="py-4 text-center text-muted-foreground">Loading...</div>;
    }

    const embedPct = embedStatus && embedStatus.total > 0
        ? Math.round((embedStatus.embedded / embedStatus.total) * 100)
        : 0;

    return (
        <Card>
            <CardContent className="space-y-4 pt-6">
                <div className="p-4 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
                    <h4 className="font-medium text-green-800 dark:text-green-300">✓ Knowledge Graph Active</h4>
                </div>

                <div className="grid grid-cols-2 gap-4">
                    <a href="/graph" className="p-4 rounded-lg border border-slate-200 dark:border-zinc-700 hover:border-purple-400 dark:hover:border-purple-500 transition-colors group">
                        <h4 className="font-medium group-hover:text-purple-600 dark:group-hover:text-purple-400">Open Graph Explorer</h4>
                        <p className="text-sm text-muted-foreground mt-1">Visualize entities and relationships</p>
                    </a>
                    <div className="p-4 rounded-lg border border-slate-200 dark:border-zinc-700">
                        <h4 className="font-medium">Backend Health</h4>
                        <p className="text-sm text-muted-foreground mt-1">{healthStatus}</p>
                    </div>
                </div>

                {/* PrimeKG Embedding Section */}
                <div className="p-4 rounded-lg border border-slate-200 dark:border-zinc-700 space-y-3">
                    <div className="flex items-center justify-between">
                        <div>
                            <h4 className="font-medium">PrimeKG Global Knowledge</h4>
                            <p className="text-sm text-muted-foreground mt-0.5">
                                129,375 medical entities · semantic search via BGE-M3
                            </p>
                        </div>
                        {embedStatus?.status === "completed" && (
                            <span className="text-xs font-medium px-2 py-1 rounded bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300">
                                ✓ Embedded
                            </span>
                        )}
                        {embedStatus?.status === "running" && (
                            <span className="text-xs font-medium px-2 py-1 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300 animate-pulse">
                                Running…
                            </span>
                        )}
                    </div>

                    {embedStatus && embedStatus.total > 0 && (
                        <div className="space-y-1">
                            <div className="h-2 rounded-full bg-slate-100 dark:bg-zinc-800 overflow-hidden">
                                <div
                                    className="h-full rounded-full bg-blue-500 transition-all duration-500"
                                    style={{ width: `${embedPct}%` }}
                                />
                            </div>
                            <div className="flex justify-between text-xs text-muted-foreground">
                                <span>{embedStatus.embedded.toLocaleString()} / {embedStatus.total.toLocaleString()} nodes</span>
                                <span>{embedPct}%{embedStatus.errors > 0 ? ` · ${embedStatus.errors} errors` : ""}</span>
                            </div>
                        </div>
                    )}

                    {embedStatus?.message && (
                        <p className="text-xs text-muted-foreground">{embedStatus.message}</p>
                    )}

                    <div className="flex gap-2">
                        <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleEmbed(true)}
                            disabled={embedLoading || embedStatus?.status === "running"}
                        >
                            Dry Run (500 nodes)
                        </Button>
                        <Button
                            size="sm"
                            onClick={() => handleEmbed(false)}
                            disabled={embedLoading || embedStatus?.status === "running"}
                        >
                            {embedStatus?.status === "completed" ? "Re-embed All" : "Embed All (129K)"}
                        </Button>
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
