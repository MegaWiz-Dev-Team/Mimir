"use client";

// Sprint 39 Mimir Curator — datasets list page.
// Lists training corpus datasets (visible to caller's tenant + shared NULL-tenant ones).

import { useEffect, useState } from "react";
import Link from "next/link";
import {
    listDatasets,
    createDataset,
    type CorpusDataset,
} from "@/lib/training-api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Plus, Database, Globe, Users, RefreshCw, Loader2 } from "lucide-react";

export default function TrainingDatasetsPage() {
    const [datasets, setDatasets] = useState<CorpusDataset[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [creating, setCreating] = useState(false);
    const [newName, setNewName] = useState("");
    const [newDescription, setNewDescription] = useState("");
    const [newSource, setNewSource] = useState("local-gemma-4-26b");
    const [shareToAll, setShareToAll] = useState(false);

    async function load() {
        setLoading(true);
        setError(null);
        try {
            const data = await listDatasets();
            setDatasets(data);
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        } finally {
            setLoading(false);
        }
    }

    useEffect(() => {
        load();
    }, []);

    async function handleCreate(e: React.FormEvent) {
        e.preventDefault();
        if (!newName.trim()) return;
        setCreating(true);
        try {
            await createDataset({
                name: newName.trim(),
                description: newDescription.trim() || undefined,
                source: newSource.trim() || undefined,
                tenant_id: shareToAll ? null : undefined,
            });
            setNewName("");
            setNewDescription("");
            await load();
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        } finally {
            setCreating(false);
        }
    }

    function progressPct(d: CorpusDataset): number {
        if (d.total_items === 0) return 0;
        return Math.round(
            ((d.approved_items + d.rejected_items) / d.total_items) * 100
        );
    }

    return (
        <div className="container mx-auto p-6 space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold">Training Curator</h1>
                    <p className="text-muted-foreground mt-1">
                        Sprint 39 Phase 0 — annotation workflow for LoRA fine-tune corpora.
                        Build a Q-A dataset, review it, export as JSONL.
                    </p>
                </div>
                <Button variant="outline" onClick={load} disabled={loading}>
                    {loading ? (
                        <Loader2 className="h-4 w-4 animate-spin mr-2" />
                    ) : (
                        <RefreshCw className="h-4 w-4 mr-2" />
                    )}
                    Refresh
                </Button>
            </div>

            {error && (
                <Card className="border-destructive">
                    <CardContent className="pt-6 text-destructive">{error}</CardContent>
                </Card>
            )}

            {/* Create new dataset */}
            <Card>
                <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                        <Plus className="h-5 w-5" /> Create dataset
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    <form onSubmit={handleCreate} className="grid gap-4 md:grid-cols-2">
                        <div>
                            <Label htmlFor="name">Name</Label>
                            <Input
                                id="name"
                                placeholder="Eir LoRA training set v1"
                                value={newName}
                                onChange={(e) => setNewName(e.target.value)}
                                required
                            />
                        </div>
                        <div>
                            <Label htmlFor="source">Source</Label>
                            <Input
                                id="source"
                                placeholder="local-gemma-4-26b | gemini-2.5-pro | manual"
                                value={newSource}
                                onChange={(e) => setNewSource(e.target.value)}
                            />
                        </div>
                        <div className="md:col-span-2">
                            <Label htmlFor="description">Description (optional)</Label>
                            <Input
                                id="description"
                                placeholder="MVP corpus, ~200 pairs synthesized locally for self-review"
                                value={newDescription}
                                onChange={(e) => setNewDescription(e.target.value)}
                            />
                        </div>
                        <div className="flex items-center gap-2">
                            <input
                                type="checkbox"
                                id="share"
                                checked={shareToAll}
                                onChange={(e) => setShareToAll(e.target.checked)}
                            />
                            <Label htmlFor="share" className="cursor-pointer">
                                Share to all tenants (tenant_id = NULL)
                            </Label>
                        </div>
                        <div className="md:col-span-2 flex justify-end">
                            <Button type="submit" disabled={creating || !newName.trim()}>
                                {creating ? (
                                    <Loader2 className="h-4 w-4 animate-spin mr-2" />
                                ) : (
                                    <Plus className="h-4 w-4 mr-2" />
                                )}
                                Create
                            </Button>
                        </div>
                    </form>
                </CardContent>
            </Card>

            {/* Datasets list */}
            <Card>
                <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                        <Database className="h-5 w-5" /> Datasets ({datasets.length})
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    {loading ? (
                        <div className="flex justify-center py-10">
                            <Loader2 className="h-6 w-6 animate-spin" />
                        </div>
                    ) : datasets.length === 0 ? (
                        <p className="text-muted-foreground text-sm py-6 text-center">
                            No datasets yet. Create one above.
                        </p>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Name</TableHead>
                                    <TableHead>Scope</TableHead>
                                    <TableHead>Source</TableHead>
                                    <TableHead>Progress</TableHead>
                                    <TableHead className="text-right">Total</TableHead>
                                    <TableHead className="text-right">✅ Approved</TableHead>
                                    <TableHead className="text-right">❌ Rejected</TableHead>
                                    <TableHead></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {datasets.map((d) => (
                                    <TableRow key={d.id}>
                                        <TableCell>
                                            <div className="font-medium">{d.name}</div>
                                            {d.description && (
                                                <div className="text-xs text-muted-foreground">
                                                    {d.description}
                                                </div>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            {d.tenant_id === null ? (
                                                <Badge variant="outline" className="gap-1">
                                                    <Globe className="h-3 w-3" /> Shared
                                                </Badge>
                                            ) : (
                                                <Badge variant="secondary" className="gap-1">
                                                    <Users className="h-3 w-3" />{" "}
                                                    {d.tenant_id}
                                                </Badge>
                                            )}
                                        </TableCell>
                                        <TableCell className="text-sm">{d.source ?? "—"}</TableCell>
                                        <TableCell>
                                            <div className="w-32 h-2 bg-muted rounded">
                                                <div
                                                    className="h-2 bg-primary rounded"
                                                    style={{ width: `${progressPct(d)}%` }}
                                                />
                                            </div>
                                            <span className="text-xs text-muted-foreground">
                                                {progressPct(d)}%
                                            </span>
                                        </TableCell>
                                        <TableCell className="text-right">
                                            {d.total_items}
                                        </TableCell>
                                        <TableCell className="text-right text-green-600">
                                            {d.approved_items}
                                        </TableCell>
                                        <TableCell className="text-right text-red-600">
                                            {d.rejected_items}
                                        </TableCell>
                                        <TableCell>
                                            <Link
                                                href={`/training/${d.id}`}
                                                className="text-sm text-primary hover:underline"
                                            >
                                                Review →
                                            </Link>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}
