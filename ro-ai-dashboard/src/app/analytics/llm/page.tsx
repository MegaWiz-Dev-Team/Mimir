"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Activity, Zap, Clock, DollarSign, Loader2, TrendingUp, AlertCircle, Shield, Bell, BarChart3 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
    fetchLlmUsage, fetchLlmUsageSummary, LlmUsageLog, LlmUsageSummary,
    getBudgetConfig, saveBudgetConfig, getAlerts, getBenchmark,
    BudgetConfig, UsageAlert, BenchmarkEntry,
} from "@/lib/api";

type DateRange = "today" | "7d" | "30d" | "all";

function getDateFrom(range: DateRange): string | undefined {
    if (range === "all") return undefined;
    const now = new Date();
    if (range === "today") {
        return now.toISOString().split("T")[0];
    }
    const days = range === "7d" ? 7 : 30;
    const d = new Date(now.getTime() - days * 86400000);
    return d.toISOString().split("T")[0];
}

function formatNumber(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return n.toString();
}

export default function LlmAnalyticsPage() {
    const [summary, setSummary] = useState<LlmUsageSummary | null>(null);
    const [logs, setLogs] = useState<LlmUsageLog[]>([]);
    const [loading, setLoading] = useState(true);
    const [dateRange, setDateRange] = useState<DateRange>("7d");
    const [currentPage, setCurrentPage] = useState(1);
    const [totalLogs, setTotalLogs] = useState(0);
    const [analyticsTab, setAnalyticsTab] = useState<"usage" | "budget" | "benchmark">("usage");

    // Budget
    const [budgets, setBudgets] = useState<BudgetConfig[]>([]);
    const [alerts, setAlerts] = useState<UsageAlert[]>([]);
    const [benchmark, setBenchmark] = useState<BenchmarkEntry[]>([]);
    const [savingBudget, setSavingBudget] = useState(false);
    const [newBudgetModel, setNewBudgetModel] = useState("");
    const [newBudgetLimit, setNewBudgetLimit] = useState(100000);
    const [newBudgetThreshold, setNewBudgetThreshold] = useState(80);

    const loadData = useCallback(async () => {
        setLoading(true);
        try {
            const dateFrom = getDateFrom(dateRange);
            const [summaryData, logsData] = await Promise.all([
                fetchLlmUsageSummary({ date_from: dateFrom }),
                fetchLlmUsage({ page: currentPage, per_page: 20, date_from: dateFrom }),
            ]);
            setSummary(summaryData);
            setLogs(logsData.logs);
            setTotalLogs(logsData.total);
        } catch (error) {
            console.warn("[Analytics] Failed to load:", error);
        } finally {
            setLoading(false);
        }
    }, [dateRange, currentPage]);

    useEffect(() => {
        loadData();
    }, [loadData]);

    const handleRangeChange = (range: DateRange) => {
        setDateRange(range);
        setCurrentPage(1);
    };

    useEffect(() => {
        if (analyticsTab === "budget") {
            getBudgetConfig().then(setBudgets).catch(() => { });
            getAlerts().then(setAlerts).catch(() => { });
        }
        if (analyticsTab === "benchmark") {
            getBenchmark().then(setBenchmark).catch(() => { });
        }
    }, [analyticsTab]);

    const handleSaveBudget = async () => {
        if (!newBudgetModel) return;
        setSavingBudget(true);
        try {
            const updated = [...budgets.map(b => ({ model_id: b.model_id, daily_token_limit: b.daily_token_limit, alert_threshold_pct: b.alert_threshold_pct })),
            { model_id: newBudgetModel, daily_token_limit: newBudgetLimit, alert_threshold_pct: newBudgetThreshold }];
            await saveBudgetConfig(updated);
            const fresh = await getBudgetConfig();
            setBudgets(fresh);
            setNewBudgetModel("");
        } catch (e) {
            console.error("Failed to save budget", e);
        } finally {
            setSavingBudget(false);
        }
    };

    const kpiCards = [
        {
            title: "Total Calls",
            value: summary ? formatNumber(summary.total_calls) : "—",
            icon: Activity,
            color: "text-blue-500",
            bg: "bg-blue-50 dark:bg-blue-950/30",
        },
        {
            title: "Total Tokens",
            value: summary ? formatNumber(summary.total_tokens) : "—",
            subtitle: summary ? `↗ ${formatNumber(summary.total_input_tokens)} in / ↙ ${formatNumber(summary.total_output_tokens)} out` : undefined,
            icon: Zap,
            color: "text-amber-500",
            bg: "bg-amber-50 dark:bg-amber-950/30",
        },
        {
            title: "Avg Latency",
            value: summary ? `${summary.avg_latency_ms.toLocaleString()} ms` : "—",
            icon: Clock,
            color: "text-green-500",
            bg: "bg-green-50 dark:bg-green-950/30",
        },
        {
            title: "Est. Cost",
            value: summary ? `$${summary.estimated_cost_usd.toFixed(2)}` : "—",
            icon: DollarSign,
            color: "text-purple-500",
            bg: "bg-purple-50 dark:bg-purple-950/30",
        },
    ];

    const dateRangeOptions: { label: string; value: DateRange }[] = [
        { label: "Today", value: "today" },
        { label: "7 Days", value: "7d" },
        { label: "30 Days", value: "30d" },
        { label: "All Time", value: "all" },
    ];

    const totalPages = Math.ceil(totalLogs / 20);

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <Activity className="w-6 h-6 text-blue-500" />
                        LLM Analytics
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1">
                        Monitor LLM usage, costs, and performance across all features
                    </p>
                </div>
                <div className="flex items-center gap-1 bg-muted rounded-lg p-1">
                    {dateRangeOptions.map((opt) => (
                        <Button
                            key={opt.value}
                            variant={dateRange === opt.value ? "default" : "ghost"}
                            size="sm"
                            onClick={() => handleRangeChange(opt.value)}
                            className="text-xs"
                        >
                            {opt.label}
                        </Button>
                    ))}
                </div>
            </div>

            {/* Analytics Tab Navigation */}
            <div className="flex gap-1 bg-muted rounded-lg p-1">
                <Button variant={analyticsTab === "usage" ? "default" : "ghost"} size="sm"
                    onClick={() => setAnalyticsTab("usage")} className="text-xs">
                    <Activity className="w-3.5 h-3.5 mr-1.5" /> Usage
                </Button>
                <Button variant={analyticsTab === "budget" ? "default" : "ghost"} size="sm"
                    onClick={() => setAnalyticsTab("budget")} className="text-xs">
                    <Shield className="w-3.5 h-3.5 mr-1.5" /> Budget & Alerts
                    {alerts.length > 0 && (
                        <Badge variant="destructive" className="ml-1.5 text-[10px] px-1.5 py-0">{alerts.length}</Badge>
                    )}
                </Button>
                <Button variant={analyticsTab === "benchmark" ? "default" : "ghost"} size="sm"
                    onClick={() => setAnalyticsTab("benchmark")} className="text-xs">
                    <BarChart3 className="w-3.5 h-3.5 mr-1.5" /> Benchmark
                </Button>
            </div>

            {analyticsTab === "usage" && (<>

                {/* KPI Cards */}
                {loading ? (
                    <div className="flex items-center justify-center py-12">
                        <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
                    </div>
                ) : (
                    <>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                            {kpiCards.map((kpi) => {
                                const Icon = kpi.icon;
                                return (
                                    <Card key={kpi.title} className="relative overflow-hidden">
                                        <CardContent className="p-5">
                                            <div className="flex items-center justify-between">
                                                <div>
                                                    <p className="text-sm text-muted-foreground">{kpi.title}</p>
                                                    <p className="text-2xl font-bold mt-1">{kpi.value}</p>
                                                    {kpi.subtitle && (
                                                        <p className="text-xs text-muted-foreground mt-1">{kpi.subtitle}</p>
                                                    )}
                                                </div>
                                                <div className={`p-3 rounded-full ${kpi.bg}`}>
                                                    <Icon className={`w-5 h-5 ${kpi.color}`} />
                                                </div>
                                            </div>
                                        </CardContent>
                                    </Card>
                                );
                            })}
                        </div>

                        {/* Model Comparison Table */}
                        <Card>
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-lg">
                                    <TrendingUp className="w-5 h-5 text-blue-500" />
                                    Model Comparison
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                {summary && summary.models.length > 0 ? (
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead>Model</TableHead>
                                                <TableHead>Provider</TableHead>
                                                <TableHead className="text-right">Calls</TableHead>
                                                <TableHead className="text-right">Total Tokens</TableHead>
                                                <TableHead className="text-right">Avg Latency</TableHead>
                                                <TableHead className="text-right">Est. Cost</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {summary.models.map((model) => (
                                                <TableRow key={`${model.model_id}-${model.provider}`}>
                                                    <TableCell className="font-medium">{model.model_id}</TableCell>
                                                    <TableCell>
                                                        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300">
                                                            {model.provider}
                                                        </span>
                                                    </TableCell>
                                                    <TableCell className="text-right">{model.total_calls.toLocaleString()}</TableCell>
                                                    <TableCell className="text-right">{formatNumber(model.total_tokens)}</TableCell>
                                                    <TableCell className="text-right">{model.avg_latency_ms.toLocaleString()} ms</TableCell>
                                                    <TableCell className="text-right font-medium">${model.estimated_cost_usd.toFixed(4)}</TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                ) : (
                                    <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
                                        <AlertCircle className="w-10 h-10 mb-3 opacity-50" />
                                        <p className="text-sm">No LLM usage data yet</p>
                                        <p className="text-xs mt-1">Data will appear here after LLM calls are made</p>
                                    </div>
                                )}
                            </CardContent>
                        </Card>

                        {/* Recent Calls Log */}
                        <Card>
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-lg">
                                    <Activity className="w-5 h-5 text-green-500" />
                                    Recent LLM Calls
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                {logs.length > 0 ? (
                                    <>
                                        <Table>
                                            <TableHeader>
                                                <TableRow>
                                                    <TableHead>Time</TableHead>
                                                    <TableHead>Model</TableHead>
                                                    <TableHead>Caller</TableHead>
                                                    <TableHead>Status</TableHead>
                                                    <TableHead className="text-right">Tokens</TableHead>
                                                    <TableHead className="text-right">Latency</TableHead>
                                                </TableRow>
                                            </TableHeader>
                                            <TableBody>
                                                {logs.map((log) => (
                                                    <TableRow key={log.id}>
                                                        <TableCell className="text-xs text-muted-foreground whitespace-nowrap">
                                                            {new Date(log.created_at).toLocaleString()}
                                                        </TableCell>
                                                        <TableCell className="font-medium text-sm">{log.model_id}</TableCell>
                                                        <TableCell>
                                                            {log.caller ? (
                                                                <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/40 dark:text-violet-300">
                                                                    {log.caller}
                                                                </span>
                                                            ) : (
                                                                <span className="text-xs text-muted-foreground">—</span>
                                                            )}
                                                        </TableCell>
                                                        <TableCell>
                                                            <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${log.status === "success"
                                                                ? "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"
                                                                : log.status === "error"
                                                                    ? "bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300"
                                                                    : "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300"
                                                                }`}>
                                                                {log.status}
                                                            </span>
                                                        </TableCell>
                                                        <TableCell className="text-right text-sm">
                                                            {formatNumber(log.total_tokens)}
                                                            <span className="text-xs text-muted-foreground ml-1">
                                                                ({log.input_tokens}↗ / {log.output_tokens}↙)
                                                            </span>
                                                        </TableCell>
                                                        <TableCell className="text-right text-sm">{log.latency_ms.toLocaleString()} ms</TableCell>
                                                    </TableRow>
                                                ))}
                                            </TableBody>
                                        </Table>

                                        {/* Pagination */}
                                        {totalPages > 1 && (
                                            <div className="flex items-center justify-between mt-4">
                                                <span className="text-xs text-muted-foreground">
                                                    Page {currentPage} of {totalPages} ({totalLogs} total)
                                                </span>
                                                <div className="flex gap-2">
                                                    <Button
                                                        variant="outline"
                                                        size="sm"
                                                        disabled={currentPage <= 1}
                                                        onClick={() => setCurrentPage(p => p - 1)}
                                                    >
                                                        Previous
                                                    </Button>
                                                    <Button
                                                        variant="outline"
                                                        size="sm"
                                                        disabled={currentPage >= totalPages}
                                                        onClick={() => setCurrentPage(p => p + 1)}
                                                    >
                                                        Next
                                                    </Button>
                                                </div>
                                            </div>
                                        )}
                                    </>
                                ) : (
                                    <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
                                        <AlertCircle className="w-10 h-10 mb-3 opacity-50" />
                                        <p className="text-sm">No LLM calls recorded yet</p>
                                        <p className="text-xs mt-1">Use AI features to see usage data here</p>
                                    </div>
                                )}
                            </CardContent>
                        </Card>
                    </>
                )}
            </>)}

            {/* Budget & Alerts Tab */}
            {analyticsTab === "budget" && (
                <div className="space-y-6">
                    {/* Active Alerts */}
                    {alerts.length > 0 && (
                        <div className="space-y-2">
                            {alerts.map((alert, i) => (
                                <div key={i} className={`flex items-center gap-3 px-4 py-3 rounded-lg border ${alert.severity === "critical"
                                        ? "bg-red-50 border-red-200 dark:bg-red-900/20 dark:border-red-800"
                                        : alert.severity === "warning"
                                            ? "bg-amber-50 border-amber-200 dark:bg-amber-900/20 dark:border-amber-800"
                                            : "bg-blue-50 border-blue-200 dark:bg-blue-900/20 dark:border-blue-800"
                                    }`}>
                                    <Bell className={`w-5 h-5 flex-shrink-0 ${alert.severity === "critical" ? "text-red-500" : alert.severity === "warning" ? "text-amber-500" : "text-blue-500"
                                        }`} />
                                    <div className="flex-1">
                                        <p className="text-sm font-medium">{alert.message}</p>
                                        <p className="text-xs text-gray-500">{alert.model_id} · {alert.alert_type}</p>
                                    </div>
                                    <Badge variant={alert.severity === "critical" ? "destructive" : "secondary"}>
                                        {alert.severity}
                                    </Badge>
                                </div>
                            ))}
                        </div>
                    )}

                    {/* Budget Config */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Shield className="w-5 h-5" /> Daily Token Budgets
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            {budgets.length > 0 && (
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <TableHead>Model</TableHead>
                                            <TableHead className="text-right">Daily Token Limit</TableHead>
                                            <TableHead className="text-right">Alert Threshold</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {budgets.map(b => (
                                            <TableRow key={b.id}>
                                                <TableCell className="font-medium">{b.model_id}</TableCell>
                                                <TableCell className="text-right">{formatNumber(b.daily_token_limit)}</TableCell>
                                                <TableCell className="text-right">{b.alert_threshold_pct}%</TableCell>
                                            </TableRow>
                                        ))}
                                    </TableBody>
                                </Table>
                            )}

                            <div className="flex items-end gap-3 pt-4 border-t">
                                <div className="flex-1">
                                    <label className="text-xs text-muted-foreground">Model ID</label>
                                    <Input value={newBudgetModel} onChange={e => setNewBudgetModel(e.target.value)}
                                        placeholder="e.g. llama3.2" className="mt-1" />
                                </div>
                                <div className="w-40">
                                    <label className="text-xs text-muted-foreground">Daily Limit</label>
                                    <Input type="number" value={newBudgetLimit}
                                        onChange={e => setNewBudgetLimit(parseInt(e.target.value) || 100000)} className="mt-1" />
                                </div>
                                <div className="w-32">
                                    <label className="text-xs text-muted-foreground">Alert %</label>
                                    <Input type="number" value={newBudgetThreshold}
                                        onChange={e => setNewBudgetThreshold(parseInt(e.target.value) || 80)} className="mt-1" />
                                </div>
                                <Button onClick={handleSaveBudget} disabled={!newBudgetModel || savingBudget}>
                                    {savingBudget ? <Loader2 className="w-4 h-4 animate-spin" /> : "Add Budget"}
                                </Button>
                            </div>
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* Benchmark Tab */}
            {analyticsTab === "benchmark" && (
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <BarChart3 className="w-5 h-5" /> Model Benchmark Report
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        {benchmark.length > 0 ? (
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Model</TableHead>
                                        <TableHead>Provider</TableHead>
                                        <TableHead className="text-right">Calls</TableHead>
                                        <TableHead className="text-right">Success %</TableHead>
                                        <TableHead className="text-right">Avg Latency</TableHead>
                                        <TableHead className="text-right">P50</TableHead>
                                        <TableHead className="text-right">P95</TableHead>
                                        <TableHead className="text-right">Avg Tokens</TableHead>
                                        <TableHead className="text-right">Total Tokens</TableHead>
                                        <TableHead className="text-right">Est. Cost</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {benchmark.map(b => (
                                        <TableRow key={`${b.model_id}-${b.provider}`}>
                                            <TableCell className="font-medium">{b.model_id}</TableCell>
                                            <TableCell>
                                                <Badge variant="outline" className="text-xs">{b.provider}</Badge>
                                            </TableCell>
                                            <TableCell className="text-right">{b.total_calls.toLocaleString()}</TableCell>
                                            <TableCell className="text-right">
                                                <span className={b.success_rate >= 95 ? "text-green-600" : b.success_rate >= 80 ? "text-amber-600" : "text-red-600"}>
                                                    {b.success_rate.toFixed(1)}%
                                                </span>
                                            </TableCell>
                                            <TableCell className="text-right">{b.avg_latency_ms.toFixed(0)}ms</TableCell>
                                            <TableCell className="text-right">{b.p50_latency_ms.toFixed(0)}ms</TableCell>
                                            <TableCell className="text-right">{b.p95_latency_ms.toFixed(0)}ms</TableCell>
                                            <TableCell className="text-right">{formatNumber(b.avg_tokens_per_call)}</TableCell>
                                            <TableCell className="text-right">{formatNumber(b.total_tokens)}</TableCell>
                                            <TableCell className="text-right font-medium">${b.estimated_cost.toFixed(4)}</TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        ) : (
                            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
                                <BarChart3 className="w-10 h-10 mb-3 opacity-50" />
                                <p className="text-sm">No benchmark data available</p>
                                <p className="text-xs mt-1">Usage data will generate benchmarks automatically</p>
                            </div>
                        )}
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
