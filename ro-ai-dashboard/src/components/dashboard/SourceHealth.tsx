"use client";

import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import { SourceHealth as SourceHealthType } from "@/lib/api";

interface SourceHealthProps {
    health: SourceHealthType | null;
    loading: boolean;
}

const COLORS: Record<string, string> = {
    healthy: "#10b981",
    failed: "#ef4444",
    pending: "#f59e0b",
    running: "#3b82f6",
};

const LABELS: Record<string, string> = {
    healthy: "Healthy",
    failed: "Failed",
    pending: "Pending",
    running: "Running",
};

export function SourceHealth({ health, loading }: SourceHealthProps) {
    const data = health
        ? Object.entries(health)
            .filter(([, v]) => v > 0)
            .map(([key, value]) => ({
                name: LABELS[key] || key,
                value,
                color: COLORS[key] || "#94a3b8",
            }))
        : [];

    const total = data.reduce((a, b) => a + b.value, 0);

    return (
        <div className="rounded-xl border bg-card shadow-sm">
            <div className="border-b px-5 py-4">
                <h3 className="text-base font-semibold">Source Health</h3>
            </div>
            <div className="p-5">
                {loading ? (
                    <div className="flex items-center justify-center h-[180px] text-muted-foreground">
                        Loading...
                    </div>
                ) : total === 0 ? (
                    <div className="flex items-center justify-center h-[180px] text-muted-foreground text-sm">
                        No sources yet
                    </div>
                ) : (
                    <>
                        <div className="h-[160px]">
                            <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
                                <PieChart>
                                    <Pie
                                        data={data}
                                        cx="50%"
                                        cy="50%"
                                        innerRadius={45}
                                        outerRadius={70}
                                        paddingAngle={3}
                                        dataKey="value"
                                        strokeWidth={0}
                                    >
                                        {data.map((entry, index) => (
                                            <Cell key={index} fill={entry.color} />
                                        ))}
                                    </Pie>
                                    <Tooltip
                                        contentStyle={{
                                            background: "hsl(var(--card))",
                                            border: "1px solid hsl(var(--border))",
                                            borderRadius: "8px",
                                            fontSize: "13px",
                                        }}
                                    />
                                </PieChart>
                            </ResponsiveContainer>
                        </div>
                        <div className="flex justify-center gap-4 mt-2">
                            {data.map((d) => (
                                <div key={d.name} className="flex items-center gap-1.5 text-sm">
                                    <span
                                        className="inline-block h-3 w-3 rounded-full"
                                        style={{ background: d.color }}
                                    />
                                    <span className="font-medium">{d.value}</span>
                                    <span className="text-muted-foreground text-xs lowercase">{d.name}</span>
                                </div>
                            ))}
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
