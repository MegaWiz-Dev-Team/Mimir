"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Cookies from "js-cookie";
import { fetchSources } from "@/lib/api";
import { ArrowRight, FolderInput, Layers, Fingerprint, MessageSquare, Cpu } from "lucide-react";
import Link from "next/link";

export function PipelineStatusBar() {
    const pathname = usePathname();
    const token = Cookies.get("access_token");

    const [mounted, setMounted] = useState(false);
    const [counts, setCounts] = useState({
        sources: 0,
        chunks: 0,
        dedup: 0,
        qa: 0,
        vector: 0,
    });

    useEffect(() => {
        setMounted(true);
    }, []);

    useEffect(() => {
        if (!mounted || pathname === "/login" || !token) return;

        const loadCounts = async () => {
            try {
                const sources = await fetchSources().catch(() => []);

                // Calculate real counts from sources data
                const totalSources = sources.length;
                const sourcesWithChunks = sources.filter((s: any) => (s.total_chunks || 0) > 0).length;
                const totalChunks = sources.reduce((sum: number, s: any) => sum + (s.total_chunks || 0), 0);
                const completedSources = sources.filter((s: any) => s.last_sync_status === "COMPLETED").length;

                setCounts({
                    sources: totalSources,
                    chunks: totalChunks,
                    dedup: completedSources, // sources that completed dedup
                    qa: 0,     // QA pipeline — future sprint
                    vector: 0, // Vector — future sprint
                });
            } catch (error) {
                console.warn("[PipelineStatusBar] Failed to load pipeline stats:", error);
            }
        };

        loadCounts();
        const interval = setInterval(loadCounts, 10000);
        return () => clearInterval(interval);
    }, [pathname, token, mounted]);

    if (!mounted || pathname === "/login" || !token) return null;

    const steps = [
        { label: "Sources", count: counts.sources, unit: "sources", icon: FolderInput, href: "/sources", color: "text-blue-500" },
        { label: "Chunks", count: counts.chunks, unit: "chunks", icon: Layers, href: "/knowledge", color: "text-amber-500" },
        { label: "Dedup", count: counts.dedup, unit: "done", icon: Fingerprint, href: "/knowledge", color: "text-emerald-500" },
        { label: "QA", count: counts.qa, unit: "pairs", icon: MessageSquare, href: "/quality_control", color: "text-purple-500", future: true },
        { label: "Vector", count: counts.vector, unit: "embedded", icon: Cpu, href: "/knowledge?tab=vectors", color: "text-rose-500", future: true },
    ];

    return (
        <div className="w-full bg-card border-b">
            <div className="container mx-auto px-8 py-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mr-2">Pipeline</div>
                    <div className="flex-1 flex items-center justify-around md:justify-start md:gap-6">
                        {steps.map((step, index) => {
                            const Icon = step.icon;
                            const isFuture = (step as any).future;
                            return (
                                <div key={step.label} className="flex items-center gap-3">
                                    <Link href={step.href}>
                                        <div className={`flex flex-col sm:flex-row items-center gap-1.5 px-3 py-1.5 rounded-lg transition-colors hover:bg-muted cursor-pointer border border-transparent hover:border-border ${isFuture ? 'opacity-50' : ''}`}>
                                            <Icon className={`w-4 h-4 ${step.color}`} />
                                            <div className="text-center sm:text-left">
                                                <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">{step.label}</div>
                                                <div className={`text-sm font-bold ${step.count > 0 ? step.color : 'text-muted-foreground'}`}>
                                                    {isFuture ? "—" : step.count} <span className="text-[10px] font-normal text-muted-foreground">{isFuture ? "" : step.unit}</span>
                                                </div>
                                            </div>
                                        </div>
                                    </Link>
                                    {index < steps.length - 1 && (
                                        <ArrowRight className="w-3 h-3 text-muted-foreground/30 hidden md:block" />
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
