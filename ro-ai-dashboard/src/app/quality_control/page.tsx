"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { CheckCircle2, AlertTriangle, ArrowRight, Save, Edit3 } from "lucide-react";

// Fake Data for mock UI
const MOCK_CONFLICTS = [
    {
        id: "c1",
        topic: "Maximum Base Level",
        reasoning: "Source A implies max level is 99, but Source B mentions level 150 mechanics.",
        sourceA: { q: "What is the max level?", a: "The maximum base level a character can reach is 99." },
        sourceB: { q: "How to reach max level?", a: "To reach level 150, you must transcend first." }
    }
];

const MOCK_DUPLICATES = [
    {
        id: "d1",
        topic: "Prontera Capital",
        goldenAnswer: "Prontera is the capital city of the Rune-Midgarts Kingdom, located in the center of the continent. It is the primary trading hub.",
        sources: [
            { q: "What is Prontera?", a: "It is the capital city." },
            { q: "Where is the main trading hub?", a: "Prontera is the center of trade in Rune-Midgarts." }
        ]
    }
];

export default function QualityControlPage() {
    const [activeTab, setActiveTab] = useState<"conflicts" | "duplicates">("conflicts");
    const [conflicts, setConflicts] = useState(MOCK_CONFLICTS);
    const [duplicates, setDuplicates] = useState(MOCK_DUPLICATES);

    return (
        <div className="container mx-auto p-8">
            <div className="mb-8">
                <h1 className="text-3xl font-bold tracking-tight">Data Quality Control</h1>
                <p className="text-muted-foreground">Review Q/A Clusters, resolve conflicts, and approve automated merges.</p>
            </div>

            <div className="grid gap-4 md:grid-cols-3 mb-8">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between pb-2">
                        <CardTitle className="text-sm font-medium">Total Clusters</CardTitle>
                    </CardHeader>
                    <CardContent><div className="text-2xl font-bold">124</div></CardContent>
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
                                        <div className="p-4 border border-gray-200 dark:border-zinc-800 rounded-lg bg-gray-50 dark:bg-zinc-900/50">
                                            <div className="text-xs font-bold text-gray-500 mb-2 uppercase tracking-wider">Source A</div>
                                            <div className="font-medium mb-2">Q: {c.sourceA.q}</div>
                                            <div className="text-gray-600 dark:text-gray-400">A: {c.sourceA.a}</div>
                                            <Button variant="outline" className="w-full mt-4" onClick={() => setConflicts(conflicts.filter(x => x.id !== c.id))}>
                                                Accept Source A
                                            </Button>
                                        </div>
                                        <div className="p-4 border border-gray-200 dark:border-zinc-800 rounded-lg bg-gray-50 dark:bg-zinc-900/50">
                                            <div className="text-xs font-bold text-gray-500 mb-2 uppercase tracking-wider">Source B</div>
                                            <div className="font-medium mb-2">Q: {c.sourceB.q}</div>
                                            <div className="text-gray-600 dark:text-gray-400">A: {c.sourceB.a}</div>
                                            <Button variant="outline" className="w-full mt-4" onClick={() => setConflicts(conflicts.filter(x => x.id !== c.id))}>
                                                Accept Source B
                                            </Button>
                                        </div>
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
                                        <div className="text-sm text-green-600 font-medium">({d.sources.length} sources merged)</div>
                                    </div>
                                </CardHeader>
                                <CardContent className="p-6">
                                    <div className="grid md:grid-cols-[1fr_auto_1fr] gap-6 items-center">
                                        <div className="space-y-3">
                                            {d.sources.map((s, idx) => (
                                                <div key={idx} className="p-3 bg-gray-50 dark:bg-zinc-900/50 rounded-md border border-gray-100 dark:border-zinc-800 text-sm">
                                                    <strong className="block mb-1">Q: {s.q}</strong>
                                                    <span className="text-gray-600 dark:text-gray-400">{s.a}</span>
                                                </div>
                                            ))}
                                        </div>
                                        <ArrowRight className="w-8 h-8 text-gray-400 hidden md:block" />
                                        <div className="p-4 border-2 border-green-200 dark:border-green-900/50 bg-green-50/30 dark:bg-green-900/10 rounded-lg h-full">
                                            <div className="text-xs font-bold text-green-600 dark:text-green-500 mb-2 uppercase tracking-wider flex items-center gap-1">
                                                <CheckCircle2 className="w-4 h-4" /> Suggested Golden Answer
                                            </div>
                                            <p className="text-gray-800 dark:text-gray-200 leading-relaxed">
                                                {d.goldenAnswer}
                                            </p>
                                            <div className="mt-6 flex gap-2">
                                                <Button className="w-full bg-green-600 hover:bg-green-700 text-white" onClick={() => setDuplicates(duplicates.filter(x => x.id !== d.id))}>
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
