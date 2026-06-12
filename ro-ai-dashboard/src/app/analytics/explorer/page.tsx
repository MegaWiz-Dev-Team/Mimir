"use client";

// Asgard Analytics — SQL Explorer (ADR-024 P3).
// Read-only SQL over registered datasets via analytics-api → table + chart.

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Loader2, Play, AlertCircle, Table as TableIcon, BarChart3 } from "lucide-react";
import {
    ResponsiveContainer, BarChart, Bar, XAxis, YAxis, Tooltip, CartesianGrid,
} from "recharts";

const TENANT = "asgard_analytics";

interface Column { name: string; type: string; nullable: boolean }
interface QueryResult {
    columns: Column[];
    rows: (string | null)[][];
    truncated: boolean;
    row_count: number;
}

// Pick the first column whose values all parse as finite numbers → chartable y-axis.
function numericColumnIndex(res: QueryResult): number {
    for (let c = 0; c < res.columns.length; c++) {
        if (res.rows.length === 0) break;
        const allNum = res.rows.every((r) => {
            const v = r[c];
            return v !== null && v !== "" && Number.isFinite(Number(v));
        });
        if (allNum) return c;
    }
    return -1;
}

export default function AnalyticsExplorerPage() {
    const [sql, setSql] = useState(
        "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY n DESC"
    );
    const [result, setResult] = useState<QueryResult | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);

    const run = async () => {
        setLoading(true);
        setError(null);
        setResult(null);
        try {
            const resp = await fetch("/api/analytics/query", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ tenant_id: TENANT, sql }),
            });
            const data = await resp.json();
            if (!resp.ok) {
                setError(data?.error || `query failed (HTTP ${resp.status})`);
            } else {
                setResult(data as QueryResult);
            }
        } catch (e) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    };

    // Build chart data: x = first column, y = first all-numeric column.
    const yIdx = result ? numericColumnIndex(result) : -1;
    const xIdx = result && yIdx >= 0 ? (yIdx === 0 ? 1 : 0) : -1;
    const chartData =
        result && yIdx >= 0 && xIdx >= 0
            ? result.rows.map((r) => ({
                  x: r[xIdx] ?? "",
                  y: Number(r[yIdx]),
              }))
            : [];

    return (
        <div className="p-6 space-y-6 max-w-5xl mx-auto">
            <div>
                <h1 className="text-2xl font-semibold flex items-center gap-2">
                    <BarChart3 className="h-6 w-6" /> Analytics Explorer
                </h1>
                <p className="text-sm text-muted-foreground">
                    Read-only SQL (DuckDB) over registered datasets · tenant {TENANT}
                </p>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle className="text-base">Query</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                    <textarea
                        className="w-full min-h-[120px] rounded-md border border-input bg-background p-3 font-mono text-sm"
                        value={sql}
                        onChange={(e) => setSql(e.target.value)}
                        spellCheck={false}
                    />
                    <div className="flex items-center gap-3">
                        <Button onClick={run} disabled={loading || !sql.trim()}>
                            {loading ? (
                                <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                                <Play className="h-4 w-4" />
                            )}
                            Run
                        </Button>
                        <span className="text-xs text-muted-foreground">
                            Only SELECT / WITH / DESCRIBE — capped &amp; time-limited &amp; audited
                        </span>
                    </div>
                    {error && (
                        <div className="flex items-start gap-2 text-sm text-red-600">
                            <AlertCircle className="h-4 w-4 mt-0.5" />
                            <span className="font-mono">{error}</span>
                        </div>
                    )}
                </CardContent>
            </Card>

            {result && (
                <>
                    {chartData.length > 0 && (
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base flex items-center gap-2">
                                    <BarChart3 className="h-4 w-4" /> Chart
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <ResponsiveContainer width="100%" height={280}>
                                    <BarChart data={chartData}>
                                        <CartesianGrid strokeDasharray="3 3" />
                                        <XAxis dataKey="x" />
                                        <YAxis />
                                        <Tooltip />
                                        <Bar dataKey="y" fill="#6366f1" />
                                    </BarChart>
                                </ResponsiveContainer>
                            </CardContent>
                        </Card>
                    )}

                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base flex items-center gap-2">
                                <TableIcon className="h-4 w-4" /> Results
                                <span className="text-xs font-normal text-muted-foreground">
                                    {result.row_count} rows{result.truncated ? " (truncated)" : ""}
                                </span>
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="overflow-x-auto">
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        {result.columns.map((c) => (
                                            <TableHead key={c.name}>
                                                {c.name}
                                                <span className="ml-1 text-[10px] text-muted-foreground">
                                                    {c.type}
                                                </span>
                                            </TableHead>
                                        ))}
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {result.rows.map((r, i) => (
                                        <TableRow key={i}>
                                            {r.map((v, j) => (
                                                <TableCell key={j} className="font-mono text-sm">
                                                    {v ?? <span className="text-muted-foreground">NULL</span>}
                                                </TableCell>
                                            ))}
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                </>
            )}
        </div>
    );
}
