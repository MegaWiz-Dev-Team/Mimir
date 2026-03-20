"use client";

import { DataSource } from "@/lib/api";
import { CheckCircle2, Circle, Globe, Upload, Database, FileText, Lock } from "lucide-react";
import Link from "next/link";

interface PipelineStatusTableProps {
    sources: DataSource[];
    loading: boolean;
    qaCount?: number;
    qdrantPoints?: number;
}

function getTypeBadge(type: string) {
    const config: Record<string, { icon: React.ReactNode; label: string; bg: string }> = {
        web: {
            icon: <Globe className="w-3.5 h-3.5" />,
            label: "Web",
            bg: "bg-blue-50 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300",
        },
        file: {
            icon: <Upload className="w-3.5 h-3.5" />,
            label: "File",
            bg: "bg-violet-50 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300",
        },
        document: {
            icon: <Upload className="w-3.5 h-3.5" />,
            label: "File",
            bg: "bg-violet-50 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300",
        },
        tabular: {
            icon: <FileText className="w-3.5 h-3.5" />,
            label: "Tabular",
            bg: "bg-emerald-50 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300",
        },
        mcp: {
            icon: <Database className="w-3.5 h-3.5" />,
            label: "MCP",
            bg: "bg-purple-50 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300",
        },
    };
    const c = config[type] ?? { icon: <Circle className="w-3.5 h-3.5" />, label: type, bg: "bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400" };
    return (
        <span className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full ${c.bg}`}>
            {c.icon}
            {c.label}
        </span>
    );
}

/** Done = green check, Not done = gray empty circle, Locked = lock icon with tooltip */
function StatusIcon({ done, locked, tooltip }: { done: boolean; locked?: boolean; tooltip?: string }) {
    if (locked) {
        return (
            <div className="flex justify-center" title={tooltip}>
                <Lock className="w-4 h-4 text-muted-foreground/30" />
            </div>
        );
    }
    return (
        <div className="flex justify-center">
            {done ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
            ) : (
                <Circle className="w-4 h-4 text-muted-foreground/25" />
            )}
        </div>
    );
}

export function PipelineStatusTable({ sources, loading, qaCount = 0, qdrantPoints = 0 }: PipelineStatusTableProps) {
    const completedCount = sources.filter((s) => s.last_sync_status === "COMPLETED").length;
    const hasQA = qaCount > 0;
    const hasVector = qdrantPoints > 0;

    return (
        <div className="rounded-xl border bg-card shadow-sm">
            <div className="border-b px-5 py-4 flex items-center justify-between">
                <h3 className="text-base font-semibold">Pipeline Status (per source)</h3>
                {sources.length > 0 && (
                    <span className="text-xs text-muted-foreground">
                        {completedCount}/{sources.length} fully processed
                    </span>
                )}
            </div>
            <div className="overflow-x-auto">
                <table className="w-full text-sm">
                    <thead>
                        <tr className="border-b text-muted-foreground">
                            <th className="px-5 py-3 text-left font-medium">Source Name</th>
                            <th className="px-3 py-3 text-left font-medium">Type</th>
                            <th className="px-3 py-3 text-center font-medium w-20">Ingest</th>
                            <th className="px-3 py-3 text-center font-medium w-20">Chunks</th>
                            <th className="px-3 py-3 text-center font-medium w-20">Dedup</th>
                            <th className="px-3 py-3 text-center font-medium w-20">
                                <span title="QA pair generation">QA</span>
                            </th>
                            <th className="px-3 py-3 text-center font-medium w-20">
                                <span title="Vector embedding to Qdrant">Vector</span>
                            </th>
                        </tr>
                    </thead>
                    <tbody className="divide-y">
                        {loading ? (
                            <tr>
                                <td colSpan={7} className="px-5 py-8 text-center text-muted-foreground">
                                    <div className="flex items-center justify-center gap-2">
                                        <div className="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                                        Loading...
                                    </div>
                                </td>
                            </tr>
                        ) : sources.length === 0 ? (
                            <tr>
                                <td colSpan={7} className="px-5 py-8 text-center text-muted-foreground">
                                    No sources yet — add one to get started.
                                </td>
                            </tr>
                        ) : (
                            sources.map((s) => (
                                <tr key={s.id} className="hover:bg-muted/50 transition-colors">
                                    <td className="px-5 py-3 font-medium">
                                        <Link
                                            href="/sources"
                                            className="text-blue-600 dark:text-blue-400 hover:underline"
                                        >
                                            {s.name}
                                        </Link>
                                    </td>
                                    <td className="px-3 py-3">
                                        {getTypeBadge(s.source_type)}
                                    </td>
                                    <td className="px-3 py-3">
                                        <StatusIcon done={s.last_sync_status === "COMPLETED" || s.last_sync_status === "RUNNING"} />
                                    </td>
                                    <td className="px-3 py-3">
                                        <StatusIcon done={(s.total_chunks ?? 0) > 0} />
                                    </td>
                                    <td className="px-3 py-3">
                                        <StatusIcon done={s.last_sync_status === "COMPLETED"} />
                                    </td>
                                    <td className="px-3 py-3">
                                        <StatusIcon done={hasQA} tooltip={hasQA ? `${qaCount} QA pairs generated` : "No QA pairs yet"} />
                                    </td>
                                    <td className="px-3 py-3">
                                        <StatusIcon done={hasVector} tooltip={hasVector ? `${qdrantPoints} vectors indexed` : "Not indexed yet"} />
                                    </td>
                                </tr>
                            ))
                        )}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
