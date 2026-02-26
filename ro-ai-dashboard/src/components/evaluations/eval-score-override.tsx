"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Check, X, Edit2, Loader2 } from "lucide-react";

interface EvalScoreOverrideProps {
    scoreId: number;
    initialAccuracy: number | null;
    initialCompleteness: number | null;
    initialRelevance: number | null;
    initialNotes: string | null;
    onSaved: () => void;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api";

export function EvalScoreOverride({
    scoreId,
    initialAccuracy,
    initialCompleteness,
    initialRelevance,
    initialNotes,
    onSaved
}: EvalScoreOverrideProps) {
    const [isEditing, setIsEditing] = useState(false);
    const [loading, setLoading] = useState(false);

    const [acc, setAcc] = useState<string>(initialAccuracy !== null ? String(initialAccuracy) : "");
    const [comp, setComp] = useState<string>(initialCompleteness !== null ? String(initialCompleteness) : "");
    const [rel, setRel] = useState<string>(initialRelevance !== null ? String(initialRelevance) : "");
    const [notes, setNotes] = useState<string>(initialNotes || "");

    const handleSave = async () => {
        setLoading(true);
        try {
            const res = await fetch(`${API_BASE}/v1/eval/scores/${scoreId}/review`, {
                method: "PATCH",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    accuracy_score: acc ? parseInt(acc) : null,
                    completeness_score: comp ? parseInt(comp) : null,
                    relevance_score: rel ? parseInt(rel) : null,
                    notes: notes || null,
                    reviewed_by: "admin" // Hardcoded for now, should come from auth ctx
                }),
            });

            if (res.ok) {
                setIsEditing(false);
                onSaved();
            } else {
                alert("Failed to save review");
            }
        } catch (e) {
            console.error(e);
            alert("Error saving review");
        } finally {
            setLoading(false);
        }
    };

    if (!isEditing) {
        return (
            <Button variant="ghost" size="xs" onClick={() => setIsEditing(true)} className="text-xs">
                <Edit2 className="h-3 w-3 mr-1" /> Override
            </Button>
        );
    }

    return (
        <div className="bg-muted p-3 rounded-md border text-left min-w-[200px] shadow-sm">
            <div className="flex justify-between items-center mb-2">
                <span className="text-xs font-semibold">Human Override</span>
                <Button variant="ghost" size="icon-xs" onClick={() => setIsEditing(false)}>
                    <X className="h-3 w-3" />
                </Button>
            </div>

            <div className="grid grid-cols-3 gap-2 mb-2">
                <div>
                    <label className="text-[10px] text-muted-foreground">Acc</label>
                    <input
                        type="number"
                        min="1" max="5"
                        value={acc} onChange={e => setAcc(e.target.value)}
                        className="w-full text-xs p-1 rounded border bg-background text-center"
                        placeholder="-"
                    />
                </div>
                <div>
                    <label className="text-[10px] text-muted-foreground">Comp</label>
                    <input
                        type="number"
                        min="1" max="5"
                        value={comp} onChange={e => setComp(e.target.value)}
                        className="w-full text-xs p-1 rounded border bg-background text-center"
                        placeholder="-"
                    />
                </div>
                <div>
                    <label className="text-[10px] text-muted-foreground">Rel</label>
                    <input
                        type="number"
                        min="1" max="5"
                        value={rel} onChange={e => setRel(e.target.value)}
                        className="w-full text-xs p-1 rounded border bg-background text-center"
                        placeholder="-"
                    />
                </div>
            </div>

            <div className="mb-2">
                <label className="text-[10px] text-muted-foreground">Notes</label>
                <input
                    type="text"
                    value={notes} onChange={e => setNotes(e.target.value)}
                    className="w-full text-xs p-1 rounded border bg-background"
                    placeholder="Reasoning..."
                />
            </div>

            <Button size="sm" className="w-full h-7 text-xs" onClick={handleSave} disabled={loading}>
                {loading ? <Loader2 className="h-3 w-3 animate-spin mx-auto" /> : <><Check className="h-3 w-3 mr-1" /> Save</>}
            </Button>
        </div>
    );
}
