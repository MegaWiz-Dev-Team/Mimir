"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { fetchStepQA, fetchStepReport } from "@/lib/api";
import { QAResult, EvaluationReport } from "@/types/pipeline";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { QACard } from "@/components/ui/qa-card";
import { CoverageChart } from "@/components/ui/coverage-chart";
import { ArrowLeft, CheckCircle2, AlertCircle, RefreshCw } from "lucide-react";
import Link from "next/link";
import { Badge } from "@/components/ui/badge";

export default function StepDetailsPage() {
    const params = useParams();
    const id = params?.id ? parseInt(params.id as string) : null;

    const [qaList, setQaList] = useState<QAResult[]>([]);
    const [report, setReport] = useState<EvaluationReport | null>(null);
    const [loading, setLoading] = useState(true);
    const [runId, setRunId] = useState<string | null>(null);

    useEffect(() => {
        // Parse runId from query params
        const searchParams = new URLSearchParams(window.location.search);
        const rId = searchParams.get("runId");
        if (rId) setRunId(rId);
    }, []);

    useEffect(() => {
        const loadData = async () => {
            if (!id) return;
            setLoading(true);
            try {
                const [qaData, reportData] = await Promise.all([
                    fetchStepQA(id),
                    fetchStepReport(id)
                ]);
                setQaList(qaData);
                setReport(reportData);
            } catch (error) {
                console.error("Failed to load step details", error);
            } finally {
                setLoading(false);
            }
        };
        loadData();
    }, [id]);

    const backLink = runId ? `/runs/${runId}` : "/";
    const backText = runId ? "Back to Run Details" : "Back to Dashboard";

    if (loading) return <div className="p-8">Loading...</div>;
    if (!id) return <div className="p-8">Invalid Step ID</div>;


    return (
        <div className="container mx-auto p-8">
            <Button variant="ghost" asChild className="mb-6 pl-0 hover:bg-transparent">
                <Link href={backLink} className="flex items-center gap-2 text-muted-foreground hover:text-foreground">
                    <ArrowLeft className="h-4 w-4" /> {backText}
                </Link>
            </Button>

            <div className="flex items-center justify-between mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Step Details #{id}</h1>
                    <p className="text-muted-foreground">Analysis and Generation Results</p>
                </div>
                <Button variant="outline" onClick={async () => {
                    if (confirm("Retry this step?")) {
                        await import("@/lib/api").then(m => m.retryStep(id));
                        // Since backend update is now sync, we can just reload immediately
                        window.location.reload();
                    }
                }}>
                    <RefreshCw className="mr-2 h-4 w-4" /> [Beta] Force Retry Step
                </Button>
            </div>

            <div className="grid gap-8 lg:grid-cols-3">
                {/* Left Column: QA Results (2/3 width) */}
                <div className="lg:col-span-2 space-y-6">
                    <h2 className="text-xl font-semibold flex items-center gap-2">
                        Generated Q/A Pairs
                        <Badge variant="secondary" className="ml-2">{qaList.length}</Badge>
                    </h2>

                    {qaList.length > 0 ? (
                        <div className="space-y-4">
                            {qaList.map((qa) => (
                                <QACard key={qa.id} qa={qa} />
                            ))}
                        </div>
                    ) : (
                        <Card className="bg-muted/50 border-dashed">
                            <CardContent className="flex items-center justify-center p-8 text-muted-foreground">
                                No Q/A pairs generated for this step.
                            </CardContent>
                        </Card>
                    )}
                </div>

                {/* Right Column: Evaluation Report (1/3 width) */}
                <div className="space-y-6">
                    <h2 className="text-xl font-semibold">Evaluation Report</h2>

                    {report ? (
                        <div className="space-y-6">
                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-sm font-medium text-center">Coverage Score</CardTitle>
                                </CardHeader>
                                <CardContent>
                                    <CoverageChart score={report.coverage_score} />
                                </CardContent>
                            </Card>

                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-sm font-medium">Reasoning</CardTitle>
                                </CardHeader>
                                <CardContent className="text-sm text-muted-foreground">
                                    {report.reasoning || "No reasoning provided."}
                                </CardContent>
                            </Card>

                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-sm font-medium flex items-center gap-2">
                                        <CheckCircle2 className="h-4 w-4 text-green-500" />
                                        Facts Covered
                                    </CardTitle>
                                </CardHeader>
                                <CardContent className="text-sm">
                                    {report.atomic_facts && report.atomic_facts.length > 0 ? (
                                        <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                                            {report.atomic_facts.slice(0, 5).map((fact: any, i) => (
                                                <li key={i} className="truncate" title={typeof fact === 'string' ? fact : fact.fact}>
                                                    {typeof fact === 'string' ? fact : fact.fact}
                                                </li>
                                            ))}
                                            {report.atomic_facts.length > 5 && (
                                                <li className="text-xs italic">...and {report.atomic_facts.length - 5} more</li>
                                            )}
                                        </ul>
                                    ) : (
                                        <p className="text-muted-foreground italic">None</p>
                                    )}
                                </CardContent>
                            </Card>

                            <Card>
                                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                    <div className="flex items-center gap-2">
                                        <CardTitle className="text-sm font-medium flex items-center gap-2">
                                            <AlertCircle className="h-4 w-4 text-amber-500" />
                                            Missing Facts
                                        </CardTitle>
                                    </div>
                                    {report.coverage_score < 1.0 && report.missing_facts && report.missing_facts.length > 0 && (
                                        <Button
                                            size="sm"
                                            variant="secondary"
                                            className="h-8 text-xs font-semibold bg-amber-100 text-amber-900 hover:bg-amber-200"
                                            onClick={async () => {
                                                if (!id) return;
                                                setLoading(true);
                                                try {
                                                    const { generateMissingQA } = await import("@/lib/api");
                                                    await generateMissingQA(id);

                                                    // Reload data without full page refresh
                                                    const [qaData, reportData] = await Promise.all([
                                                        fetchStepQA(id),
                                                        fetchStepReport(id)
                                                    ]);
                                                    setQaList(qaData);
                                                    setReport(reportData);
                                                } catch (e) {
                                                    alert("Failed to generate missing Q/A");
                                                } finally {
                                                    setLoading(false);
                                                }
                                            }}
                                        >
                                            ✨ Generate Missing Q/A
                                        </Button>
                                    )}
                                </CardHeader>
                                <CardContent className="text-sm">
                                    {report.missing_facts && report.missing_facts.length > 0 ? (
                                        <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                                            {report.missing_facts.map((fact: any, i) => (
                                                <li key={i}>
                                                    {typeof fact === 'string' ? fact : fact.fact}
                                                </li>
                                            ))}
                                        </ul>
                                    ) : (
                                        <p className="text-green-600 font-medium text-xs">All facts covered!</p>
                                    )}
                                </CardContent>
                            </Card>
                        </div>
                    ) : (
                        <Card className="bg-muted/50 border-dashed">
                            <CardContent className="flex items-center justify-center p-8 text-muted-foreground">
                                No evaluation report available.
                            </CardContent>
                        </Card>
                    )}
                </div>
            </div>
        </div>
    );
}
