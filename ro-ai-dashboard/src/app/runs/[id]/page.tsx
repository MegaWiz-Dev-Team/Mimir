"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { fetchRunDetails } from "@/lib/api";
import { RunDetails } from "@/types/pipeline";
import { StatusBadge } from "@/components/ui/status-badge";
import { PipelineFlow } from "@/components/pipeline-flow";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ArrowLeft, RefreshCw, PlayCircle, FileText, ExternalLink } from "lucide-react";
import Link from "next/link";

export default function RunDetailsPage() {
    const params = useParams();
    const id = params?.id as string;
    const [run, setRun] = useState<RunDetails | null>(null);
    const [loading, setLoading] = useState(true);
    const [confirmingRetry, setConfirmingRetry] = useState<number | null>(null);
    const [confirmingResume, setConfirmingResume] = useState(false);

    const loadRun = async (silent = false) => {
        if (!id) return;
        if (!silent) setLoading(true);
        try {
            const data = await fetchRunDetails(id);
            setRun(data);
        } catch (error) {
            console.warn("[Runs] Failed to load run details:", error);
        } finally {
            if (!silent) setLoading(false);
        }
    };

    useEffect(() => {
        loadRun();
        // Auto-refresh if running or steps are running
        const interval = setInterval(() => {
            const hasRunningSteps = run?.steps?.some(s => s.status === "RUNNING");
            if (run?.status === "RUNNING" || hasRunningSteps) {
                loadRun(true);
            }
        }, 3000);
        return () => clearInterval(interval);
    }, [id, run?.status, run?.steps?.length]); // Added dependency to check for steps status changes indirectly

    if (loading && !run) {
        return <div className="p-8">Loading...</div>;
    }

    if (!run) {
        return <div className="p-8">Run not found</div>;
    }

    return (
        <div className="container mx-auto p-8">
            <div className="mb-6">
                <Button variant="ghost" asChild className="mb-4 pl-0 hover:bg-transparent">
                    <Link href="/" className="flex items-center gap-2 text-muted-foreground hover:text-foreground">
                        <ArrowLeft className="h-4 w-4" /> Back to Dashboard
                    </Link>
                </Button>

                <div className="flex justify-between items-start">
                    <div>
                        <div className="flex items-center gap-3 mb-2">
                            <h1 className="text-3xl font-bold tracking-tight">Run Details</h1>
                            <StatusBadge status={run.status} className="text-sm px-3 py-1" />
                        </div>
                        <p className="text-muted-foreground font-mono text-sm">{run.id}</p>
                    </div>
                    <div className="flex gap-2">
                        {run.status === "FAILED" && (
                            confirmingResume ? (
                                <div className="flex items-center gap-2">
                                    <span className="text-sm font-medium text-muted-foreground mr-1">Resume run?</span>
                                    <Button size="sm" variant="default" onClick={async () => {
                                        setConfirmingResume(false);
                                        try {
                                            await import("@/lib/api").then(m => m.resumeRun(run.id));
                                            loadRun();
                                        } catch (e) {
                                            alert("Failed to resume run");
                                        }
                                    }}>Yes</Button>
                                    <Button size="sm" variant="outline" onClick={() => setConfirmingResume(false)}>No</Button>
                                </div>
                            ) : (
                                <Button variant="outline" size="sm" onClick={() => setConfirmingResume(true)}>
                                    <PlayCircle className="mr-2 h-4 w-4" />
                                    Resume
                                </Button>
                            )
                        )}
                        <Button variant="outline" size="sm" onClick={() => loadRun()}>
                            <RefreshCw className={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
                            Refresh
                        </Button>
                    </div>
                </div>
            </div>

            <PipelineFlow steps={run.steps} />

            <div className="grid gap-6 md:grid-cols-2 mb-8">
                <Card>
                    <CardHeader className="pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Configuration</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <dl className="grid grid-cols-2 gap-4 text-sm">
                            <div>
                                <dt className="text-muted-foreground">Provider</dt>
                                <dd className="font-medium">{run.provider}</dd>
                            </div>
                            <div>
                                <dt className="text-muted-foreground">Model</dt>
                                <dd className="font-medium">{run.model}</dd>
                            </div>
                        </dl>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Timing</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <dl className="grid grid-cols-2 gap-4 text-sm">
                            <div>
                                <dt className="text-muted-foreground">Started</dt>
                                <dd className="font-medium">{new Date(run.started_at).toLocaleString()}</dd>
                            </div>
                            <div>
                                <dt className="text-muted-foreground">Duration</dt>
                                <dd className="font-medium">
                                    {run.finished_at
                                        ? `${((new Date(run.finished_at).getTime() - new Date(run.started_at).getTime()) / 1000).toFixed(1)}s`
                                        : "Running..."}
                                </dd>
                            </div>
                        </dl>
                    </CardContent>
                </Card>
            </div>

            <h2 className="text-xl font-semibold mb-4">Pipeline Steps ({run.steps.length})</h2>

            <Card>
                <Table>
                    <TableHeader>
                        <TableRow>
                            <TableHead>ID</TableHead>
                            <TableHead>Step Type</TableHead>
                            <TableHead>File Name</TableHead>
                            <TableHead>Chunk</TableHead>
                            <TableHead>Q/A Count</TableHead>
                            <TableHead>Coverage</TableHead>
                            <TableHead>Status</TableHead>
                            <TableHead className="text-right">Actions</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {run.steps.map((step) => (
                            <TableRow key={step.id}>
                                <TableCell className="font-mono text-xs">{step.id}</TableCell>
                                <TableCell>
                                    <span className="inline-flex items-center px-2 py-1 rounded-md bg-muted text-xs font-medium">
                                        {step.step_type}
                                    </span>
                                </TableCell>
                                <TableCell className="font-medium">
                                    <div className="flex items-center gap-2">
                                        <span>{step.file_name}</span>
                                        <div className="flex items-center gap-1 opacity-50 hover:opacity-100 transition-opacity">
                                            <a
                                                href={`http://localhost:8080/api/wiki/${step.file_name}`}
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                title="View Markdown Source"
                                                className="hover:text-primary bg-muted p-1 rounded"
                                            >
                                                <FileText className="h-3.5 w-3.5" />
                                            </a>
                                            <button
                                                onClick={async () => {
                                                    try {
                                                        const resp = await fetch(`http://localhost:8080/api/wiki/${step.file_name}`);
                                                        const text = await resp.text();
                                                        const urlMatch = text.match(/url:\s*"([^"]+)"/);
                                                        if (urlMatch && urlMatch[1]) {
                                                            window.open(urlMatch[1], '_blank');
                                                        } else {
                                                            alert("Original URL not found in markdown frontmatter.");
                                                        }
                                                    } catch (e) {
                                                        console.warn("[Runs]", e);
                                                        alert("Failed to load original URL.");
                                                    }
                                                }}
                                                title="View Original Website"
                                                className="hover:text-primary bg-muted p-1 rounded"
                                            >
                                                <ExternalLink className="h-3.5 w-3.5" />
                                            </button>
                                        </div>
                                    </div>
                                </TableCell>
                                <TableCell>#{step.chunk_index}</TableCell>
                                <TableCell>{step.qa_count}</TableCell>
                                <TableCell>
                                    {step.coverage_score !== undefined && step.coverage_score !== null ? (() => {
                                        const normalizedScore = step.coverage_score > 1 ? step.coverage_score / 100 : step.coverage_score;
                                        const percentage = Math.min(100, Math.round(normalizedScore * 100));
                                        const colorClass = percentage === 100 ? "text-green-500" : (percentage > 50 ? "text-yellow-500" : "text-red-500");
                                        return <span className={`font-medium ${colorClass}`}>{percentage}%</span>;
                                    })() : '-'}
                                </TableCell>
                                <TableCell>
                                    <StatusBadge status={step.status} />
                                </TableCell>
                                <TableCell className="text-right space-x-2">
                                    {step.status === "FAILED" && (
                                        confirmingRetry === step.id ? (
                                            <div className="flex items-center gap-1 justify-end">
                                                <Button size="sm" variant="destructive" onClick={async () => {
                                                    setConfirmingRetry(null);
                                                    try {
                                                        await import("@/lib/api").then(m => m.retryStep(step.id));
                                                        loadRun();
                                                    } catch (e) {
                                                        alert("Failed to retry step");
                                                    }
                                                }}>Yes</Button>
                                                <Button size="sm" variant="outline" onClick={() => setConfirmingRetry(null)}>No</Button>
                                            </div>
                                        ) : (
                                            <Button
                                                variant="outline"
                                                size="sm"
                                                className="text-red-500 hover:text-red-700 hover:bg-red-50"
                                                onClick={() => setConfirmingRetry(step.id)}
                                            >
                                                <RefreshCw className="h-3 w-3 mr-1" /> Retry
                                            </Button>
                                        )
                                    )}
                                    <Button asChild variant="ghost" size="sm">
                                        <Link href={`/steps/${step.id}?runId=${run.id}`}>View Results</Link>
                                    </Button>
                                </TableCell>
                            </TableRow>
                        ))}
                        {run.steps.length === 0 && (
                            <TableRow>
                                <TableCell colSpan={6} className="text-center h-24 text-muted-foreground">
                                    No steps recorded yet.
                                </TableCell>
                            </TableRow>
                        )}
                    </TableBody>
                </Table>
            </Card>
        </div>
    );
}
