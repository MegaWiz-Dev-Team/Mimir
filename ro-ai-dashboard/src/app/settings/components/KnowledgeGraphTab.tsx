"use client";

import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { SettingsTabProps } from "./types";
import { fetchHealth } from "@/lib/api";

export function KnowledgeGraphTab({ isLoading }: SettingsTabProps) {
    const [healthStatus, setHealthStatus] = useState<string>("Checking...");

    useEffect(() => {
        fetchHealth()
            .then(data => {
                if (data && data.status === "ok") {
                    setHealthStatus(`Connected (Backend v${data.version || "unknown"})`);
                } else {
                    setHealthStatus("Disconnected");
                }
            })
            .catch(() => {
                setHealthStatus("Disconnected (API Unreachable)");
            });
    }, []);

    if (isLoading) {
        return <div className="py-4 text-center text-muted-foreground">Loading...</div>;
    }

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
                        <h4 className="font-medium">Backend Health Status</h4>
                        <p className="text-sm text-muted-foreground mt-1">{healthStatus}</p>
                    </div>
                </div>
            </CardContent>
        </Card>
    );
}
