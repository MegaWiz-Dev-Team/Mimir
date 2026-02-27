"use client";

import { DataSource } from "@/lib/api";
import { StatusBadge } from "@/components/ui/status-badge";

interface RecentActivityProps {
    sources: DataSource[];
    loading: boolean;
}

function timeAgo(dateStr: string): string {
    const now = new Date();
    const date = new Date(dateStr);
    const diff = Math.floor((now.getTime() - date.getTime()) / 1000);

    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
}

export function RecentActivity({ sources, loading }: RecentActivityProps) {
    const recent = [...sources]
        .sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
        .slice(0, 10);

    return (
        <div className="rounded-xl border bg-card shadow-sm">
            <div className="border-b px-5 py-4">
                <h3 className="text-base font-semibold">Recent Activity</h3>
            </div>
            <div className="divide-y">
                {loading ? (
                    <div className="flex items-center justify-center p-8 text-muted-foreground">
                        Loading...
                    </div>
                ) : recent.length === 0 ? (
                    <div className="flex items-center justify-center p-8 text-muted-foreground">
                        No activity yet
                    </div>
                ) : (
                    recent.map((source) => (
                        <div key={source.id} className="flex items-start gap-3 px-5 py-3">
                            <div
                                className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${source.last_sync_status === "COMPLETED"
                                    ? "bg-emerald-500"
                                    : source.last_sync_status === "FAILED"
                                        ? "bg-red-500"
                                        : source.last_sync_status === "RUNNING"
                                            ? "bg-blue-500 animate-pulse"
                                            : "bg-amber-500"
                                    }`}
                            />
                            <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-2 flex-wrap">
                                    <span className="font-medium text-sm truncate">
                                        {source.name}
                                    </span>
                                    <span className="text-xs">—</span>
                                    <StatusBadge status={source.last_sync_status || "PENDING"} />
                                    {source.total_chunks != null && source.total_chunks > 0 && (
                                        <span className="text-xs text-muted-foreground">
                                            — {source.total_chunks} chunks
                                        </span>
                                    )}
                                </div>
                                <p className="text-xs text-muted-foreground mt-0.5" suppressHydrationWarning>
                                    Updated {timeAgo(source.updated_at)}
                                </p>
                            </div>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
