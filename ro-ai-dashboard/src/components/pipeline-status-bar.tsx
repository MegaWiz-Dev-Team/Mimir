"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Cookies from "js-cookie";
import { fetchSources, fetchRuns, fetchQcClusters, fetchVectorStats } from "@/lib/api";
import { ArrowRight, Database, PlayCircle, ShieldCheck, CheckCircle2 } from "lucide-react";
import Link from "next/link";

export function PipelineStatusBar() {
    const pathname = usePathname();
    const token = Cookies.get("access_token");

    // Hide on login page or when not authenticated
    if (pathname === "/login" || !token) return null;

    const [counts, setCounts] = useState({
        sources: 0,
        running: 0,
        pendingQc: 0,
        vectorized: 0,
    });

    useEffect(() => {
        const loadCounts = async () => {
            try {
                const [sources, runs, qcData, vectorStats] = await Promise.all([
                    fetchSources().catch(() => []),
                    fetchRuns().catch(() => []),
                    fetchQcClusters().catch(() => ({ clusters: [] })),
                    fetchVectorStats().catch(() => ({ database: { indexed_qa: 0 } })),
                ]);

                // Count active/pending items 
                const activeSources = sources.filter((s: any) => s.last_sync_status !== "COMPLETED").length || sources.length;
                const activeRuns = runs.filter((r: any) => r.status === "RUNNING").length;
                const pendingQc = qcData.clusters?.filter((c: any) => c.status === "PENDING").length || 0;
                const vectorized = vectorStats.database?.indexed_qa || 0;

                setCounts({
                    sources: activeSources,
                    running: activeRuns,
                    pendingQc,
                    vectorized,
                });
            } catch (error) {
                console.warn("[PipelineStatusBar] Failed to load pipeline stats:", error);
            }
        };

        loadCounts();
        const interval = setInterval(loadCounts, 10000); // 10s refresh
        return () => clearInterval(interval);
    }, []);

    const steps = [
        { label: "Sources", count: counts.sources, icon: Database, href: "/sources", color: "text-blue-500", bg: "bg-blue-50 dark:bg-blue-950/30 text-blue-700" },
        { label: "Generating", count: counts.running, icon: PlayCircle, href: "/", color: "text-amber-500", bg: "bg-amber-50 dark:bg-amber-950/30 text-amber-700" },
        { label: "Pending QC", count: counts.pendingQc, icon: ShieldCheck, href: "/quality_control", color: "text-red-500", bg: "bg-red-50 dark:bg-red-950/30 text-red-700" },
        { label: "Vectorized", count: counts.vectorized, icon: CheckCircle2, href: "/vector", color: "text-green-500", bg: "bg-green-50 dark:bg-green-950/30 text-green-700" },
    ];

    return (
        <div className="w-full bg-card border-b">
            <div className="container mx-auto px-8 py-4">
                <div className="flex flex-wrap items-center justify-between gap-4">
                    <div className="text-sm font-semibold text-muted-foreground mr-4">Global Pipeline Status:</div>
                    <div className="flex-1 flex items-center justify-around md:justify-start md:gap-8">
                        {steps.map((step, index) => {
                            const Icon = step.icon;
                            return (
                                <div key={step.label} className="flex items-center gap-4">
                                    <Link href={step.href}>
                                        <div className={`flex flex-col sm:flex-row items-center gap-2 p-2 sm:px-4 sm:py-2 rounded-lg transition-colors hover:bg-muted cursor-pointer border border-transparent hover:border-border`}>
                                            <Icon className={`w-5 h-5 sm:w-6 sm:h-6 ${step.color}`} />
                                            <div className="text-center sm:text-left">
                                                <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{step.label}</div>
                                                <div className={`text-base sm:text-lg font-bold ${step.count > 0 ? step.color : ''}`}>
                                                    {step.count} <span className="text-xs font-normal text-muted-foreground">items</span>
                                                </div>
                                            </div>
                                        </div>
                                    </Link>
                                    {index < steps.length - 1 && (
                                        <ArrowRight className="w-4 h-4 text-muted-foreground/30 hidden md:block" />
                                    )}
                                </div>
                            );
                        })}
                    </div>
                </div>
            </div>
        </div>
    );
}
