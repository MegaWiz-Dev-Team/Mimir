"use client";

import { Card, CardContent } from "@/components/ui/card";
import { PipelineStep, StepStatus } from "@/types/pipeline";
import {
    FileText,
    Cpu,
    Database,
    CheckCircle2,
    CircleDot,
    AlertCircle,
    ArrowRight
} from "lucide-react";
import { cn } from "@/lib/utils";

interface PipelineFlowProps {
    steps: PipelineStep[];
}

export function PipelineFlow({ steps }: PipelineFlowProps) {
    const totalSteps = steps.length;
    const completedSteps = steps.filter(s => s.status === "COMPLETED").length;
    const failedSteps = steps.filter(s => s.status === "FAILED").length;
    const runningSteps = steps.filter(s => s.status === "RUNNING").length;

    const isAnyRunning = runningSteps > 0;
    const isAnyFailed = failedSteps > 0;
    const isAllCompleted = totalSteps > 0 && completedSteps === totalSteps;

    const getStatusColor = (status: "pending" | "running" | "completed" | "failed") => {
        switch (status) {
            case "running": return "text-blue-500 border-blue-500 bg-blue-50 dark:bg-blue-900/20";
            case "completed": return "text-green-500 border-green-500 bg-green-50 dark:bg-green-900/20";
            case "failed": return "text-red-500 border-red-500 bg-red-50 dark:bg-red-900/20";
            default: return "text-muted-foreground border-border bg-muted/50";
        }
    };

    const nodes = [
        {
            id: "ingestion",
            label: "Ingestion",
            icon: FileText,
            description: "Wiki -> Markdown",
            status: isAllCompleted ? "completed" : (isAnyRunning ? "running" : (isAnyFailed ? "failed" : "pending"))
        },
        {
            id: "processing",
            label: "AI Workshop",
            icon: Cpu,
            description: "Multi-Agent Q&A",
            status: isAllCompleted ? "completed" : (isAnyRunning ? "running" : (isAnyFailed ? "failed" : "pending"))
        },
        {
            id: "storage",
            label: "Persistence",
            icon: Database,
            description: "MariaDB / Qdrant",
            status: isAllCompleted ? "completed" : "pending"
        }
    ];

    return (
        <div className="w-full mb-8">
            <div className="flex flex-col md:flex-row items-center justify-between gap-4 relative">
                {nodes.map((node, index) => (
                    <div key={node.id} className="flex-1 flex flex-col items-center group w-full md:w-auto">
                        <Card className={cn(
                            "w-full transition-all duration-300 border-2",
                            getStatusColor(node.status as any)
                        )}>
                            <CardContent className="p-4 flex items-center gap-4">
                                <div className={cn(
                                    "p-2 rounded-full",
                                    node.status === "running" ? "animate-pulse" : ""
                                )}>
                                    <node.icon className="h-6 w-6" />
                                </div>
                                <div className="flex-1">
                                    <div className="flex items-center justify-between">
                                        <h3 className="font-semibold">{node.label}</h3>
                                        {node.status === "completed" && <CheckCircle2 className="h-4 w-4 text-green-500" />}
                                        {node.status === "running" && <CircleDot className="h-4 w-4 text-blue-500 animate-spin" />}
                                        {node.status === "failed" && <AlertCircle className="h-4 w-4 text-red-500" />}
                                    </div>
                                    <p className="text-xs opacity-70">{node.description}</p>
                                </div>
                            </CardContent>
                        </Card>

                        {index < nodes.length - 1 && (
                            <div className="hidden md:flex items-center absolute" style={{
                                left: `${(index + 1) * 33 - 2}%`,
                                top: "50%",
                                transform: "translateY(-50%)"
                            }}>
                                <ArrowRight className="text-muted-foreground h-6 w-6 opacity-30" />
                            </div>
                        )}
                    </div>
                ))}
            </div>

            <div className="mt-6">
                <div className="flex items-center justify-between text-sm mb-2">
                    <span className="text-muted-foreground">Overall Progress</span>
                    <span className="font-medium">{completedSteps + failedSteps} / {totalSteps} Chunks</span>
                </div>
                <div className="w-full bg-muted rounded-full h-2.5 overflow-hidden">
                    <div
                        className={cn(
                            "h-full transition-all duration-500",
                            isAnyFailed ? "bg-red-500" : "bg-green-500"
                        )}
                        style={{ width: `${totalSteps > 0 ? ((completedSteps + failedSteps) / totalSteps) * 100 : 0}%` }}
                    />
                </div>
            </div>
        </div>
    );
}
