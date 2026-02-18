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
import { ArrowLeft, RefreshCw, PlayCircle } from "lucide-react";
import Link from "next/link";

export default function RunDetailsPage() {
    const params = useParams();
    const id = params?.id as string;
    const [run, setRun] = useState<RunDetails | null>(null);
    const [loading, setLoading] = useState(true);

    const loadRun = async (silent = false) => {
        if (!id) return;
        if (!silent) setLoading(true);
        try {
            const data = await fetchRunDetails(id);
            setRun(data);
        } catch (error) {
            console.error("Failed to load run details", error);
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
                            <Button variant="outline" size="sm" onClick={async () => {
                                if (confirm("Resume run?")) {
                                    try {
                                        await import("@/lib/api").then(m => m.resumeRun(run.id));
                                        loadRun();
                                    } catch (e) {
                                        alert("Failed to resume run");
                                    }
                                }
                            }}>
                                <PlayCircle className="mr-2 h-4 w-4" />
                                Resume
                            </Button>
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
                                <TableCell className="font-medium">{step.file_name}</TableCell>
                                <TableCell>#{step.chunk_index}</TableCell>
                                <TableCell>
                                    <StatusBadge status={step.status} />
                                </TableCell>
                                <TableCell className="text-right space-x-2">
                                    {step.status === "FAILED" && (
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            className="text-red-500 hover:text-red-700 hover:bg-red-50"
                                            onClick={async () => {
                                                if (confirm(`Retry step #${step.id}?`)) {
                                                    try {
                                                        await import("@/lib/api").then(m => m.retryStep(step.id));
                                                        loadRun();
                                                    } catch (e) {
                                                        alert("Failed to retry step");
                                                    }
                                                }
                                            }}
                                        >
                                            <RefreshCw className="h-3 w-3 mr-1" /> Retry
                                        </Button>
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
