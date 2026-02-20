"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { CheckCircle2, AlertTriangle, ArrowRight, Save, Edit3, RefreshCw, Zap } from "lucide-react";
import { fetchQcClusters, resolveQcCluster, triggerQcGeneration } from "@/lib/api";

export default function QualityControlPage() {
    const [activeTab, setActiveTab] = useState<"conflicts" | "duplicates">("conflicts");
    const [clusters, setClusters] = useState<any[]>([]);
    const [loading, setLoading] = useState(true);
    const [generating, setGenerating] = useState(false);

    const loadData = async () => {
        setLoading(true);
        try {
            const data = await fetchQcClusters("PENDING");
            setClusters(data.clusters || []);
        } catch (e) {
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadData();
    }, []);

    const handleResolve = async (clusterId: string, resolutionType: string, goldenAnswer?: string) => {
        try {
            await resolveQcCluster(clusterId, resolutionType, goldenAnswer);
            loadData(); // Refresh list after resolving
        } catch (e) {
            alert("Failed to resolve cluster");
        }
    };

    const handleGenerate = async () => {
        setGenerating(true);
        try {
            await triggerQcGeneration();
            alert("QC generation started in background. Please wait a few seconds and refresh.");
            setTimeout(loadData, 5000); // Check back in 5 secs
        } catch (e) {
            alert("Failed to trigger generation");
        } finally {
            setGenerating(false);
        }
    };

    const conflicts = clusters.filter(c => c.cluster_type === "CONFLICT");
    const duplicates = clusters.filter(c => c.cluster_type === "DUPLICATE");

    return (
        <div className="container mx-auto p-8">
            <div className="flex justify-between items-end mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Data Quality Control</h1>
                    <p className="text-muted-foreground">Review Q/A Clusters, resolve conflicts, and approve automated merges.</p>
                </div>
                <div className="flex gap-2">
                    <Button variant="outline" onClick={loadData} disabled={loading}>
                        <RefreshCw className={`w-4 h-4 mr-2 ${loading ? 'animate-spin' : ''}`} /> Refresh
                    </Button>
                    <Button onClick={handleGenerate} disabled={generating}>
                        <Zap className="w-4 h-4 mr-2" /> {generating ? "Scanning..." : "Auto-scan QC Issues"}
                    </Button>
                </div>
            </div>

            <div className="grid gap-4 md:grid-cols-3 mb-8">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between pb-2">
                        <CardTitle className="text-sm font-medium">Pending Unresolved Clusters</CardTitle>
                    </CardHeader>
                    <CardContent><div className="text-2xl font-bold">{clusters.length}</div></CardContent>
                </Card>
                <Card className="border-red-200 dark:border-red-900/50 bg-red-50/50 dark:bg-red-950/20">
                    <CardHeader className="flex flex-row items-center justify-between pb-2">
                        <CardTitle className="text-sm font-medium text-red-600 dark:text-red-400">Pending Conflicts</CardTitle>
                        <AlertTriangle className="w-4 h-4 text-red-600 dark:text-red-400" />
                    </CardHeader>
                    <CardContent><div className="text-2xl font-bold text-red-600 dark:text-red-400">{conflicts.length}</div></CardContent>
                </Card>
                <Card className="border-green-200 dark:border-green-900/50 bg-green-50/50 dark:bg-green-950/20">
                    <CardHeader className="flex flex-row items-center justify-between pb-2">
                        <CardTitle className="text-sm font-medium text-green-600 dark:text-green-400">Auto-Merged (Pending Review)</CardTitle>
                        <CheckCircle2 className="w-4 h-4 text-green-600 dark:text-green-400" />
                    </CardHeader>
                    <CardContent><div className="text-2xl font-bold text-green-600 dark:text-green-400">{duplicates.length}</div></CardContent>
                </Card>
            </div>

            <div className="flex gap-4 mb-6">
                <Button
                    variant={activeTab === "conflicts" ? "default" : "outline"}
                    onClick={() => setActiveTab("conflicts")}
                    className={activeTab === "conflicts" ? "bg-red-600 hover:bg-red-700 text-white" : ""}
                >
                    Action Required (Conflicts)
                </Button>
                <Button
                    variant={activeTab === "duplicates" ? "default" : "outline"}
                    onClick={() => setActiveTab("duplicates")}
                    className={activeTab === "duplicates" ? "bg-green-600 hover:bg-green-700 text-white" : ""}
                >
                    Review Auto-Merges (Consensus)
                </Button>
            </div>

            {activeTab === "conflicts" && (
                <div className="space-y-6">
                    {conflicts.length === 0 ? (
                        <div className="text-center p-8 text-gray-500 bg-gray-50 rounded-lg dark:bg-zinc-900/50">No pending conflicts!</div>
                    ) : (
                        conflicts.map(c => (
                            <Card key={c.id} className="border-red-200 dark:border-zinc-800">
                                <CardHeader className="bg-red-50/50 dark:bg-zinc-900 border-b border-red-100 dark:border-zinc-800">
                                    <div className="flex justify-between items-start">
                                        <div>
                                            <CardTitle className="text-lg">Topic: {c.topic}</CardTitle>
                                            <p className="text-sm text-red-600 mt-2 font-medium bg-red-100 dark:bg-red-900/30 p-2 rounded flex items-start gap-2">
                                                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                                                AI Analysis: {c.reasoning}
                                            </p>
                                        </div>
                                    </div>
                                </CardHeader>
                                <CardContent className="p-6">
                                    <div className="grid md:grid-cols-2 gap-6">
                                        {c.items.slice(0, 2).map((item: any, idx: number) => (
                                            <div key={idx} className="p-4 border border-gray-200 dark:border-zinc-800 rounded-lg bg-gray-50 dark:bg-zinc-900/50">
                                                <div className="text-xs font-bold text-gray-500 mb-2 uppercase tracking-wider">Source {item.source_label}</div>
                                                <div className="font-medium mb-2">Q: {item.question}</div>
                                                <div className="text-gray-600 dark:text-gray-400">A: {item.answer}</div>
                                                <Button variant="outline" className="w-full mt-4" onClick={() => handleResolve(c.id, `ACCEPT_${item.source_label}`)}>
                                                    Accept Source {item.source_label}
                                                </Button>
                                            </div>
                                        ))}
                                    </div>
                                    <div className="mt-6 flex justify-center">
                                        <Button variant="secondary" onClick={() => alert("Open Manual Override Editor")}>
                                            <Edit3 className="w-4 h-4 mr-2" />
                                            Manual Override (Write Golden Answer)
                                        </Button>
                                    </div>
                                </CardContent>
                            </Card>
                        ))
                    )}
                </div>
            )}

            {activeTab === "duplicates" && (
                <div className="space-y-6">
                    {duplicates.length === 0 ? (
                        <div className="text-center p-8 text-gray-500 bg-gray-50 rounded-lg dark:bg-zinc-900/50">No pending merges!</div>
                    ) : (
                        duplicates.map(d => (
                            <Card key={d.id}>
                                <CardHeader className="bg-green-50/50 dark:bg-zinc-900 border-b border-green-100 dark:border-zinc-800">
                                    <div className="flex justify-between items-center">
                                        <CardTitle className="text-lg">Topic: {d.topic}</CardTitle>
                                        <div className="text-sm text-green-600 font-medium">({d.items.length} sources merged)</div>
                                    </div>
                                </CardHeader>
                                <CardContent className="p-6">
                                    <div className="grid md:grid-cols-[1fr_auto_1fr] gap-6 items-center">
                                        <div className="space-y-3">
                                            {d.items.map((s: any, idx: number) => (
                                                <div key={idx} className="p-3 bg-gray-50 dark:bg-zinc-900/50 rounded-md border border-gray-100 dark:border-zinc-800 text-sm">
                                                    <strong className="block mb-1">Q: {s.question}</strong>
                                                    <span className="text-gray-600 dark:text-gray-400">{s.answer}</span>
                                                </div>
                                            ))}
                                        </div>
                                        <ArrowRight className="w-8 h-8 text-gray-400 hidden md:block" />
                                        <div className="p-4 border-2 border-green-200 dark:border-green-900/50 bg-green-50/30 dark:bg-green-900/10 rounded-lg h-full">
                                            <div className="text-xs font-bold text-green-600 dark:text-green-500 mb-2 uppercase tracking-wider flex items-center gap-1">
                                                <CheckCircle2 className="w-4 h-4" /> Suggested Golden Answer
                                            </div>
                                            <p className="text-gray-800 dark:text-gray-200 leading-relaxed">
                                                {d.golden_answer}
                                            </p>
                                            <div className="mt-6 flex gap-2">
                                                <Button className="w-full bg-green-600 hover:bg-green-700 text-white" onClick={() => handleResolve(d.id, "MERGE", d.golden_answer)}>
                                                    <Save className="w-4 h-4 mr-2" /> Approve & Index
                                                </Button>
                                                <Button variant="outline" title="Edit before approve" onClick={() => alert("Open editor")}><Edit3 className="w-4 h-4" /></Button>
                                            </div>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>
                        ))
                    )}
                </div>
            )}
        </div>
    );
}
