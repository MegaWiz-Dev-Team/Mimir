"use client";

// Unified evaluation scoreboard — one view, every family.
// Reads evx_scoreboard via the registry; no per-family rendering code beyond
// EVAL_FAMILIES. Adding a family = a registry entry, this component is untouched.

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { RefreshCw, ArrowUp, ArrowDown, Minus } from "lucide-react";
import { getScoreboard, ScoreboardRow } from "@/lib/evx-api";
import { familySpec } from "@/lib/eval-family-registry";

const TENANTS = ["asgard_platform", "asgard_medical", "asgard_insurance", "asgard_wellness"] as const;

function fmtValue(row: ScoreboardRow): { text: string; pass?: boolean } {
    if (row.primary_value == null) return { text: "—" };
    const spec = familySpec(row.family);
    return { text: spec.format(row.primary_value), pass: spec.gate?.(row.primary_value) };
}

function DirIcon({ higher }: { higher: boolean | null }) {
    if (higher === null) return <Minus className="w-3 h-3 text-gray-400" />;
    return higher
        ? <ArrowUp className="w-3 h-3 text-gray-400" />
        : <ArrowDown className="w-3 h-3 text-gray-400" />;
}

export function EvxScoreboard() {
    const [rows, setRows] = useState<ScoreboardRow[]>([]);
    const [tenant, setTenant] = useState<string>("asgard_platform");
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const load = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            setRows(await getScoreboard({ tenant }));
        } catch (e) {
            setError(e instanceof Error ? e.message : "failed to load");
        } finally {
            setLoading(false);
        }
    }, [tenant]);

    useEffect(() => { void load(); }, [load]);

    // group by family, preserving the order the API returned (family ASC)
    const byFamily = rows.reduce<Record<string, ScoreboardRow[]>>((acc, r) => {
        (acc[r.family] ??= []).push(r);
        return acc;
    }, {});

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <h2 className="text-lg font-semibold">Evaluation Scoreboard</h2>
                <div className="flex items-center gap-2">
                    <Select value={tenant} onValueChange={setTenant}>
                        <SelectTrigger className="w-48"><SelectValue /></SelectTrigger>
                        <SelectContent>
                            {TENANTS.map((t) => <SelectItem key={t} value={t}>{t}</SelectItem>)}
                        </SelectContent>
                    </Select>
                    <button
                        onClick={() => void load()}
                        className="flex items-center gap-1 px-3 py-2 text-sm rounded-md border hover:bg-gray-50 dark:hover:bg-zinc-800"
                    >
                        <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} /> Refresh
                    </button>
                </div>
            </div>

            {error && <div className="text-sm text-red-600">⚠️ {error}</div>}
            {!loading && rows.length === 0 && !error && (
                <div className="text-sm text-gray-500">No eval runs for this tenant yet.</div>
            )}

            {Object.entries(byFamily).map(([family, frows]) => {
                const spec = familySpec(family);
                return (
                    <Card key={family}>
                        <CardHeader className="pb-2">
                            <CardTitle className="text-base flex items-center gap-2">
                                {spec.label}
                                <span className="text-xs font-normal text-gray-400 flex items-center gap-1">
                                    {spec.primaryMetric} <DirIcon higher={spec.higherIsBetter} />
                                </span>
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Target</TableHead>
                                        <TableHead>{spec.primaryMetric}</TableHead>
                                        <TableHead>95% CI</TableHead>
                                        <TableHead className="text-right">n</TableHead>
                                        <TableHead>Dataset</TableHead>
                                        <TableHead>Finished</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {frows.map((r) => {
                                        const v = fmtValue(r);
                                        const color = v.pass === undefined ? ""
                                            : v.pass ? "text-green-600" : "text-red-600";
                                        return (
                                            <TableRow key={r.run_id}>
                                                <TableCell className="font-medium">
                                                    {r.target_name}
                                                    {r.runtime && <span className="ml-1 text-xs text-gray-400">({r.runtime})</span>}
                                                </TableCell>
                                                <TableCell className={`font-mono ${color}`}>{v.text}</TableCell>
                                                <TableCell className="font-mono text-xs text-gray-500">
                                                    {r.ci_low != null && r.ci_high != null
                                                        ? `${spec.format(r.ci_low)}–${spec.format(r.ci_high)}`
                                                        : "—"}
                                                </TableCell>
                                                <TableCell className="text-right text-gray-500">{r.n_items}</TableCell>
                                                <TableCell className="text-xs text-gray-500">{r.dataset_id ?? "—"}</TableCell>
                                                <TableCell className="text-xs text-gray-500">
                                                    {r.finished_at ? new Date(r.finished_at).toLocaleString() : "—"}
                                                </TableCell>
                                            </TableRow>
                                        );
                                    })}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                );
            })}
        </div>
    );
}
