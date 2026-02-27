"use client";

import { Database, Layers, MessageSquare, BarChart3 } from "lucide-react";
import { StatsResponse } from "@/lib/api";

interface DashboardStatsProps {
    stats: StatsResponse | null;
    loading: boolean;
}

const cards = [
    {
        key: "total_sources",
        label: "Total Sources",
        icon: Database,
        color: "text-blue-600 dark:text-blue-400",
        bg: "bg-blue-50 dark:bg-blue-900/30",
        getValue: (s: StatsResponse) => s.total_sources,
    },
    {
        key: "total_chunks",
        label: "Total Chunks",
        icon: Layers,
        color: "text-emerald-600 dark:text-emerald-400",
        bg: "bg-emerald-50 dark:bg-emerald-900/30",
        getValue: (s: StatsResponse) => s.total_chunks,
    },
    {
        key: "qa_pairs",
        label: "QA Pairs",
        icon: MessageSquare,
        color: "text-violet-600 dark:text-violet-400",
        bg: "bg-violet-50 dark:bg-violet-900/30",
        getValue: (s: StatsResponse) => s.qa_pairs,
    },
    {
        key: "vector_coverage",
        label: "Vector Coverage",
        icon: BarChart3,
        color: "text-amber-600 dark:text-amber-400",
        bg: "bg-amber-50 dark:bg-amber-900/30",
        getValue: (s: StatsResponse) => `${Math.round(s.vector_coverage)}%`,
    },
];

export function DashboardStats({ stats, loading }: DashboardStatsProps) {
    return (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {cards.map(({ key, label, icon: Icon, color, bg, getValue }) => (
                <div
                    key={key}
                    className="flex items-center gap-4 rounded-xl border bg-card p-5 shadow-sm transition-shadow hover:shadow-md"
                >
                    <div className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-lg ${bg}`}>
                        <Icon className={`h-6 w-6 ${color}`} />
                    </div>
                    <div>
                        <p className="text-sm font-medium text-muted-foreground">{label}</p>
                        <p className="text-2xl font-bold tracking-tight">
                            {loading ? (
                                <span className="inline-block h-7 w-12 animate-pulse rounded bg-muted" />
                            ) : stats ? (
                                getValue(stats)
                            ) : (
                                "—"
                            )}
                        </p>
                    </div>
                </div>
            ))}
        </div>
    );
}
