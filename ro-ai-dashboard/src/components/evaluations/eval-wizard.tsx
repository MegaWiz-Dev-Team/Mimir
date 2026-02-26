"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Play, Loader2, Check, ExternalLink } from "lucide-react";

interface Agent {
    id: string;
    name: string;
    description?: string;
}

interface Model {
    model_id: string;
    provider: string;
}

interface EvalWizardProps {
    onTriggerRun: () => void;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api";

export function EvalWizard({ onTriggerRun }: EvalWizardProps) {
    const [open, setOpen] = useState(false);
    const [step, setStep] = useState(1);
    const [loading, setLoading] = useState(false);
    const [agents, setAgents] = useState<Agent[]>([]);
    const [models, setModels] = useState<Model[]>([]);

    // Form state
    const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
    const [selectedModels, setSelectedModels] = useState<string[]>([]);
    const [questionLimit, setQuestionLimit] = useState(50);

    const loadData = async () => {
        try {
            // Simplified loading. In a real app we'd fetch actual agents.
            // Using a hardcoded list for now as per the requirements or we can fetch them.
            setAgents([{ id: 'simple_npc', name: 'Simple NPC' }, { id: 'oracle_rag', name: 'Oracle RAG' }]);

            const modelRes = await fetch(`${API_BASE}/v1/models`);
            if (modelRes.ok) {
                const data = await modelRes.json();
                setModels(data);
            }
        } catch (e) {
            console.error("Failed to load options", e);
        }
    };

    useEffect(() => {
        if (open) {
            setStep(1);
            setSelectedAgents([]);
            setSelectedModels([]);
            setQuestionLimit(50);
            loadData();
        }
    }, [open]);

    const handleRun = async () => {
        if (selectedAgents.length === 0 || selectedModels.length === 0) return;

        setLoading(true);
        try {
            const res = await fetch(`${API_BASE}/v1/eval/run`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    tenant_id: "", // Will be filled by backend context unless SuperAdmin
                    agent_names: selectedAgents,
                    model_ids: selectedModels,
                    question_limit: questionLimit
                }),
            });

            if (res.ok) {
                setOpen(false);
                onTriggerRun();
            } else {
                const err = await res.json();
                alert(`Error starting run: ${err.error || 'Unknown error'}`);
            }
        } catch (e) {
            console.error("Failed to start run", e);
            alert("Failed to connect to server");
        } finally {
            setLoading(false);
        }
    };

    const toggleAgent = (id: string) => {
        setSelectedAgents(prev =>
            prev.includes(id) ? prev.filter(a => a !== id) : [...prev, id]
        );
    };

    const toggleModel = (id: string) => {
        setSelectedModels(prev =>
            prev.includes(id) ? prev.filter(m => m !== id) : [...prev, id]
        );
    };

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
                <Button>
                    <Play className="mr-2 h-4 w-4" />
                    New Evaluation
                </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-[500px]">
                <DialogHeader>
                    <DialogTitle>New Evaluation Run</DialogTitle>
                    <DialogDescription>
                        Step {step} of 3: {
                            step === 1 ? "Select Agents" :
                                step === 2 ? "Select Models" :
                                    "Configure & Run"
                        }
                    </DialogDescription>
                </DialogHeader>

                <div className="py-4">
                    {step === 1 && (
                        <div className="space-y-4">
                            <p className="text-sm font-medium">Select agents to evaluate against your golden dataset:</p>
                            <div className="grid grid-cols-2 gap-3">
                                {agents.map(a => (
                                    <div
                                        key={a.id}
                                        onClick={() => toggleAgent(a.id)}
                                        className={`p-3 border rounded-lg cursor-pointer transition-colors ${selectedAgents.includes(a.id) ? 'border-primary bg-primary/10' : 'hover:bg-accent'}`}
                                    >
                                        <div className="flex items-center justify-between">
                                            <span className="font-medium text-sm">{a.name}</span>
                                            {selectedAgents.includes(a.id) && <Check className="h-4 w-4 text-primary" />}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}

                    {step === 2 && (
                        <div className="space-y-4">
                            <p className="text-sm font-medium">Select LLM models to test the agents with:</p>
                            <div className="space-y-2 max-h-[300px] overflow-y-auto">
                                {models.length === 0 ? (
                                    <div className="text-sm text-muted-foreground italic">Loading models or none found...</div>
                                ) : models.map(m => (
                                    <div
                                        key={m.model_id}
                                        onClick={() => toggleModel(m.model_id)}
                                        className={`p-3 border rounded-lg cursor-pointer transition-colors flex items-center justify-between ${selectedModels.includes(m.model_id) ? 'border-primary bg-primary/10' : 'hover:bg-accent'}`}
                                    >
                                        <div>
                                            <div className="font-medium text-sm">{m.model_id}</div>
                                            <div className="text-xs text-muted-foreground">{m.provider}</div>
                                        </div>
                                        {selectedModels.includes(m.model_id) && <Check className="h-4 w-4 text-primary" />}
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}

                    {step === 3 && (
                        <div className="space-y-4">
                            <div className="bg-muted p-4 rounded-lg space-y-2">
                                <div className="flex justify-between text-sm">
                                    <span className="text-muted-foreground">Agents Selected:</span>
                                    <span className="font-medium">{selectedAgents.length}</span>
                                </div>
                                <div className="flex justify-between text-sm">
                                    <span className="text-muted-foreground">Models Selected:</span>
                                    <span className="font-medium">{selectedModels.length}</span>
                                </div>
                            </div>

                            <div className="space-y-2 pt-2 border-t">
                                <label className="text-sm font-medium">Question Limit</label>
                                <p className="text-xs text-muted-foreground mb-2">Maximum number of questions to evaluate from your golden dataset.</p>
                                <input
                                    type="number"
                                    className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                    value={questionLimit}
                                    onChange={(e) => setQuestionLimit(Number(e.target.value))}
                                    min="1"
                                    max="1000"
                                />
                            </div>

                            <div className="text-xs text-muted-foreground flex gap-2 items-start mt-4 bg-blue-500/10 text-blue-400 p-3 rounded">
                                <ExternalLink className="h-4 w-4 shrink-0 mt-0.5" />
                                <p>This run will evaluate {selectedAgents.length * selectedModels.length * questionLimit} combinations. Evaluation runs asynchronously in the background.</p>
                            </div>
                        </div>
                    )}
                </div>

                <DialogFooter className="flex justify-between sm:justify-between items-center w-full">
                    <div>
                        {step > 1 && (
                            <Button variant="outline" onClick={() => setStep(step - 1)}>
                                Back
                            </Button>
                        )}
                    </div>
                    <div>
                        {step < 3 ? (
                            <Button
                                onClick={() => setStep(step + 1)}
                                disabled={(step === 1 && selectedAgents.length === 0) || (step === 2 && selectedModels.length === 0)}
                            >
                                Next
                            </Button>
                        ) : (
                            <Button onClick={handleRun} disabled={loading || selectedAgents.length === 0 || selectedModels.length === 0}>
                                {loading ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Play className="mr-2 h-4 w-4" />}
                                Start Evaluation
                            </Button>
                        )}
                    </div>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
