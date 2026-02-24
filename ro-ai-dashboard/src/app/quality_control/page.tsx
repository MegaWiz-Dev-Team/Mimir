"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { CheckCircle2, AlertTriangle, ArrowRight, Save, Edit3, RefreshCw, Zap, GripVertical, FileText } from "lucide-react";
import { fetchQcClusters, resolveQcCluster, triggerQcGeneration } from "@/lib/api";
import { DragDropContext, Droppable, Draggable, DropResult } from "@hello-pangea/dnd";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogFooter,
} from "@/components/ui/dialog";

type ClusterStatus = "PENDING" | "RESOLVED_A" | "RESOLVED_B" | "MERGED" | "MANUAL_OVERRIDE";

interface KanbanColumn {
    id: string;
    title: string;
    status: ClusterStatus[];
}

const COLUMNS: KanbanColumn[] = [
    { id: "pending", title: "Pending Review", status: ["PENDING"] },
    { id: "resolved", title: "Resolved", status: ["RESOLVED_A", "RESOLVED_B", "MERGED", "MANUAL_OVERRIDE"] }
];

export default function QualityControlPage() {
    const [clusters, setClusters] = useState<any[]>([]);
    const [loading, setLoading] = useState(true);
    const [generating, setGenerating] = useState(false);

    // Dialog state
    const [selectedCluster, setSelectedCluster] = useState<any | null>(null);
    const [goldenAnswerText, setGoldenAnswerText] = useState("");

    const loadData = async () => {
        setLoading(true);
        try {
            // Fetch all instead of just pending to show both columns
            const data = await fetchQcClusters("");
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
            setSelectedCluster(null);
            loadData(); // Refresh list after resolving
        } catch (e) {
            alert("Failed to resolve cluster");
        }
    };

    const handleGenerate = async () => {
        setGenerating(true);
        try {
            await triggerQcGeneration();
            alert("QC generation started. Please wait a few seconds and refresh.");
            setTimeout(loadData, 5000);
        } catch (e) {
            alert("Failed to trigger generation");
        } finally {
            setGenerating(false);
        }
    };

    const onDragEnd = (result: DropResult) => {
        const { source, destination, draggableId } = result;

        // Dropped outside a valid droppable area
        if (!destination) return;

        // Same list, same position
        if (source.droppableId === destination.droppableId && source.index === destination.index) return;

        // Find cluster that was dragged
        const cluster = clusters.find(c => c.id === draggableId);
        if (!cluster) return;

        // If moving from Pending to Resolved
        if (source.droppableId === "pending" && destination.droppableId === "resolved") {
            // Automatically open the resolution dialog to let the user select HOW it was resolved
            openResolveDialog(cluster);
        }

        // If moving backward (Resolved -> Pending), we don't allow it explicitly via API right now, 
        // but could implement a "re-open" endpoint if needed.
        if (source.droppableId === "resolved" && destination.droppableId === "pending") {
            alert("Cannot move resolved items back to pending via drag-and-drop.");
            return;
        }
    };

    const openResolveDialog = (cluster: any) => {
        setSelectedCluster(cluster);
        // Pre-fill golden answer if it's a DUPLICATE suggestion
        setGoldenAnswerText(cluster.golden_answer || "");
    };

    // Derived states for columns
    const pendingClusters = clusters.filter(c => COLUMNS[0].status.includes(c.status));
    const resolvedClusters = clusters.filter(c => COLUMNS[1].status.includes(c.status));

    return (
        <div className="container mx-auto p-8 h-[calc(100vh-4rem)] flex flex-col">
            <div className="flex justify-between items-end mb-8 shrink-0">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Data Quality Kanban</h1>
                    <p className="text-muted-foreground">Drag pending issues to Resolved to review and approve Golden Answers.</p>
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

            {/* Kanban Board Area */}
            <DragDropContext onDragEnd={onDragEnd}>
                <div className="flex gap-6 overflow-hidden grow">
                    {/* Pending Column */}
                    <Droppable droppableId="pending">
                        {(provided, snapshot) => (
                            <div
                                ref={provided.innerRef}
                                {...provided.droppableProps}
                                className={`flex-1 flex flex-col bg-zinc-100 dark:bg-zinc-900/40 rounded-xl border border-zinc-200 dark:border-zinc-800 transition-colors ${snapshot.isDraggingOver ? 'bg-zinc-200/50 dark:bg-zinc-800/50' : ''}`}
                            >
                                <div className="p-4 font-bold border-b border-zinc-200 dark:border-zinc-800 flex justify-between items-center bg-zinc-50 dark:bg-zinc-900 rounded-t-xl shrink-0">
                                    <div className="flex items-center gap-2">
                                        <div className="w-2.5 h-2.5 rounded-full bg-yellow-500"></div>
                                        Pending Review
                                    </div>
                                    <span className="text-xs font-semibold px-2 py-1 bg-zinc-200 dark:bg-zinc-800 rounded-full">{pendingClusters.length}</span>
                                </div>
                                <div className="p-4 overflow-y-auto grow space-y-4">
                                    {pendingClusters.map((cluster, index) => (
                                        <Draggable key={cluster.id} draggableId={cluster.id} index={index}>
                                            {(provided, snapshot) => (
                                                <div
                                                    ref={provided.innerRef}
                                                    {...provided.draggableProps}
                                                    {...provided.dragHandleProps}
                                                    style={{ ...provided.draggableProps.style }}
                                                    onClick={() => openResolveDialog(cluster)}
                                                >
                                                    <ClusterCard cluster={cluster} isDragging={snapshot.isDragging} />
                                                </div>
                                            )}
                                        </Draggable>
                                    ))}
                                    {provided.placeholder}
                                </div>
                            </div>
                        )}
                    </Droppable>

                    {/* Resolved Column */}
                    <Droppable droppableId="resolved">
                        {(provided, snapshot) => (
                            <div
                                ref={provided.innerRef}
                                {...provided.droppableProps}
                                className={`flex-1 flex flex-col bg-zinc-100 dark:bg-zinc-900/40 rounded-xl border border-zinc-200 dark:border-zinc-800 transition-colors ${snapshot.isDraggingOver ? 'bg-green-50 dark:bg-green-950/20 border-green-200 dark:border-green-900' : ''}`}
                            >
                                <div className="p-4 font-bold border-b border-zinc-200 dark:border-zinc-800 flex justify-between items-center bg-zinc-50 dark:bg-zinc-900 rounded-t-xl shrink-0">
                                    <div className="flex items-center gap-2">
                                        <div className="w-2.5 h-2.5 rounded-full bg-green-500"></div>
                                        Resolved
                                    </div>
                                    <span className="text-xs font-semibold px-2 py-1 bg-zinc-200 dark:bg-zinc-800 rounded-full">{resolvedClusters.length}</span>
                                </div>
                                <div className="p-4 overflow-y-auto grow space-y-4 opacity-75">
                                    {resolvedClusters.map((cluster, index) => (
                                        <Draggable key={cluster.id} draggableId={cluster.id} index={index}>
                                            {(provided, snapshot) => (
                                                <div
                                                    ref={provided.innerRef}
                                                    {...provided.draggableProps}
                                                    {...provided.dragHandleProps}
                                                    style={{ ...provided.draggableProps.style }}
                                                >
                                                    <ClusterCard cluster={cluster} isDragging={snapshot.isDragging} />
                                                </div>
                                            )}
                                        </Draggable>
                                    ))}
                                    {provided.placeholder}
                                </div>
                            </div>
                        )}
                    </Droppable>
                </div>
            </DragDropContext>

            {/* Resolution Dialog */}
            <Dialog open={!!selectedCluster} onOpenChange={(open) => !open && setSelectedCluster(null)}>
                <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2 text-xl">
                            {selectedCluster?.cluster_type === 'CONFLICT' ? (
                                <><AlertTriangle className="text-red-500 w-5 h-5" /> Resolve Conflict</>
                            ) : (
                                <><FileText className="text-blue-500 w-5 h-5" /> Review Duplicate</>
                            )}
                        </DialogTitle>
                        <DialogDescription>
                            Topic: {selectedCluster?.topic}
                        </DialogDescription>
                    </DialogHeader>

                    {selectedCluster && (
                        <div className="py-4 space-y-6">
                            {/* AI Reasoning Panel */}
                            <div className={`p-4 rounded-lg border text-sm ${selectedCluster.cluster_type === 'CONFLICT'
                                    ? 'bg-red-50/50 border-red-100 dark:bg-red-950/20 dark:border-red-900/30'
                                    : 'bg-blue-50/50 border-blue-100 dark:bg-blue-950/20 dark:border-blue-900/30'
                                }`}>
                                <h4 className="font-semibold mb-1 flex items-center gap-2">
                                    <Zap className="w-4 h-4 text-yellow-500" /> AI Analysis
                                </h4>
                                <p className="text-muted-foreground">{selectedCluster.reasoning || "No reasoning provided."}</p>
                            </div>

                            {/* Side by side comparison */}
                            <div className="grid md:grid-cols-2 gap-4">
                                {selectedCluster.items.map((item: any) => (
                                    <div key={item.qa_id} className="border rounded-xl p-4 flex flex-col h-full bg-card shadow-sm">
                                        <div className="mb-4">
                                            <div className="text-xs font-bold text-muted-foreground uppercase tracking-wider mb-2">
                                                Source {item.source_label}
                                            </div>
                                            <div className="font-medium text-sm mb-2">Q: {item.question}</div>
                                            <div className="text-sm text-muted-foreground">A: {item.answer}</div>
                                        </div>
                                        <div className="mt-auto pt-4 border-t">
                                            <Button
                                                variant="outline"
                                                className="w-full justify-center"
                                                onClick={() => handleResolve(selectedCluster.id, `ACCEPT_${item.source_label}`)}
                                            >
                                                Mark as Correct Answer
                                            </Button>
                                        </div>
                                    </div>
                                ))}
                            </div>

                            {/* Custom Merge Block */}
                            <div className="border rounded-xl p-4 bg-muted/30">
                                <h4 className="font-medium mb-2 text-sm">Write Custom Golden Answer (Merge)</h4>
                                <textarea
                                    className="w-full min-h-[100px] p-3 rounded-md border bg-background text-sm resize-y focus:outline-none focus:ring-2 focus:ring-ring"
                                    value={goldenAnswerText}
                                    onChange={(e) => setGoldenAnswerText(e.target.value)}
                                    placeholder="Combine the best parts of both answers..."
                                />
                                <div className="mt-4 flex justify-end">
                                    <Button
                                        className="bg-green-600 hover:bg-green-700 text-white"
                                        disabled={!goldenAnswerText.trim()}
                                        onClick={() => handleResolve(selectedCluster.id, "MERGE", goldenAnswerText)}
                                    >
                                        <Save className="w-4 h-4 mr-2" /> Save Merged Answer
                                    </Button>
                                </div>
                            </div>
                        </div>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    );
}

// Sub-component for the Kanban Card
function ClusterCard({ cluster, isDragging }: { cluster: any, isDragging: boolean }) {
    const isConflict = cluster.cluster_type === "CONFLICT";

    return (
        <Card className={`shadow-sm cursor-grab active:cursor-grabbing hover:border-primary/50 transition-colors ${isDragging ? 'shadow-lg border-primary rotate-1 scale-[1.02] z-50' : ''
            } ${isConflict ? 'border-red-200/50 dark:border-red-900/30' : ''}`}>
            <CardHeader className="p-4 pb-2 flex flex-row items-start justify-between space-y-0">
                <div className="font-medium text-sm line-clamp-2 leading-relaxed pr-6">
                    {cluster.topic}
                </div>
                <div className="absolute right-4 top-4 text-muted-foreground/50">
                    <GripVertical className="w-4 h-4" />
                </div>
            </CardHeader>
            <CardContent className="p-4 pt-1">
                <div className="flex items-center justify-between mt-3">
                    <div className="flex items-center gap-1.5">
                        {isConflict ? (
                            <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-semibold bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400">
                                CONFLICT
                            </span>
                        ) : (
                            <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-semibold bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                                DUPLICATE
                            </span>
                        )}
                        <span className="text-xs text-muted-foreground">{cluster.items.length} pairs</span>
                    </div>
                    {cluster.status !== "PENDING" && (
                        <span className="text-[10px] uppercase font-bold text-green-600 bg-green-100 px-2 py-0.5 rounded">
                            {cluster.status.replace("RESOLVED_", "SRC:")}
                        </span>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}
