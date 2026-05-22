"use client";

// OCR Layout Eval — run list (Sprint 53).
//
// Lists region-detection eval runs (mAP / parity / GriTS) produced by Syn's
// syn-eval-ingest and stored via POST /eval/ocr/layout/runs. Read-only viewer.
//
// Multi-tenant: the tenant selector sets the X-Tenant-Id sent per request, so
// each domain (medical / insurance / wellness) sees its own runs while
// asgard_platform holds cross-cutting engineering benchmarks (the default).

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
    listOcrLayoutRuns,
    headlineScore,
    OCR_EVAL_TENANTS,
    type EvalKind,
    type OcrLayoutRunSummary,
} from "@/lib/ocr-eval-api";

const EVAL_KINDS: (EvalKind | "")[] = ["", "mAP", "parity", "grits"];

function fmtDuration(ms: number | null): string {
    if (ms == null) return "—";
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
}

function fmtDate(iso: string | null): string {
    if (!iso) return "—";
    return new Date(iso).toLocaleString();
}

export default function OcrLayoutEvalPage() {
    const [tenant, setTenant] = useState<string>(OCR_EVAL_TENANTS[0]);
    const [evalKind, setEvalKind] = useState<EvalKind | "">("");
    const [runs, setRuns] = useState<OcrLayoutRunSummary[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const load = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const res = await listOcrLayoutRuns({ tenant, eval_kind: evalKind, limit: 100 });
            setRuns(res.runs);
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
            setRuns([]);
        } finally {
            setLoading(false);
        }
    }, [tenant, evalKind]);

    useEffect(() => {
        load();
    }, [load]);

    return (
        <div className="p-6 max-w-6xl mx-auto">
            <div className="flex items-center justify-between mb-1">
                <h1 className="text-2xl font-bold">OCR Layout Eval</h1>
                <button
                    onClick={load}
                    className="px-3 py-1.5 text-sm rounded-md border border-gray-300 hover:bg-gray-50"
                >
                    🔄 Refresh
                </button>
            </div>
            <p className="text-sm text-gray-500 mb-5">
                Region-detection eval runs (mAP / parity / GriTS) ingested from Syn.
            </p>

            {/* Filters */}
            <div className="flex flex-wrap gap-3 mb-4">
                <label className="flex flex-col text-xs text-gray-500">
                    Tenant
                    <select
                        value={tenant}
                        onChange={(e) => setTenant(e.target.value)}
                        className="mt-1 px-2 py-1.5 text-sm border border-gray-300 rounded-md"
                    >
                        {OCR_EVAL_TENANTS.map((t) => (
                            <option key={t} value={t}>
                                {t}
                            </option>
                        ))}
                    </select>
                </label>
                <label className="flex flex-col text-xs text-gray-500">
                    Eval kind
                    <select
                        value={evalKind}
                        onChange={(e) => setEvalKind(e.target.value as EvalKind | "")}
                        className="mt-1 px-2 py-1.5 text-sm border border-gray-300 rounded-md"
                    >
                        {EVAL_KINDS.map((k) => (
                            <option key={k || "all"} value={k}>
                                {k || "all"}
                            </option>
                        ))}
                    </select>
                </label>
            </div>

            {error && (
                <div className="mb-4 p-3 text-sm rounded-md bg-red-50 text-red-700 border border-red-200">
                    {error}
                </div>
            )}

            <div className="border border-gray-200 rounded-lg overflow-hidden">
                <table className="w-full text-sm">
                    <thead className="bg-gray-50 text-gray-600">
                        <tr>
                            <th className="text-left px-3 py-2 font-medium">Kind</th>
                            <th className="text-left px-3 py-2 font-medium">Syn ver</th>
                            <th className="text-left px-3 py-2 font-medium">Model</th>
                            <th className="text-left px-3 py-2 font-medium">Dataset</th>
                            <th className="text-right px-3 py-2 font-medium">Images</th>
                            <th className="text-right px-3 py-2 font-medium">Score</th>
                            <th className="text-right px-3 py-2 font-medium">Duration</th>
                            <th className="text-left px-3 py-2 font-medium">Finished</th>
                        </tr>
                    </thead>
                    <tbody>
                        {loading && (
                            <tr>
                                <td colSpan={8} className="px-3 py-6 text-center text-gray-400">
                                    Loading…
                                </td>
                            </tr>
                        )}
                        {!loading && runs.length === 0 && (
                            <tr>
                                <td colSpan={8} className="px-3 py-6 text-center text-gray-400">
                                    No runs for {tenant}
                                    {evalKind ? ` · ${evalKind}` : ""}.
                                </td>
                            </tr>
                        )}
                        {!loading &&
                            runs.map((run) => {
                                const score = headlineScore(run);
                                return (
                                    <tr key={run.id} className="border-t border-gray-100 hover:bg-blue-50">
                                        <td className="px-3 py-2">
                                            <Link
                                                href={`/syn-ocr/eval/${run.id}?tenant=${tenant}`}
                                                className="text-blue-600 hover:underline font-mono"
                                            >
                                                {run.eval_kind}
                                            </Link>
                                        </td>
                                        <td className="px-3 py-2 font-mono text-gray-700">{run.syn_version}</td>
                                        <td className="px-3 py-2 text-gray-700">{run.model_name}</td>
                                        <td className="px-3 py-2 text-gray-700">
                                            {run.dataset_name}
                                            {run.is_synthetic && (
                                                <span title="synthetic" className="ml-1 text-amber-500">
                                                    ⚗
                                                </span>
                                            )}
                                        </td>
                                        <td className="px-3 py-2 text-right">{run.n_images ?? "—"}</td>
                                        <td className="px-3 py-2 text-right font-mono">
                                            {score ? (
                                                <span title={score.label}>{score.value}</span>
                                            ) : (
                                                "—"
                                            )}
                                        </td>
                                        <td className="px-3 py-2 text-right text-gray-500">
                                            {fmtDuration(run.duration_ms)}
                                        </td>
                                        <td className="px-3 py-2 text-gray-500">{fmtDate(run.finished_at)}</td>
                                    </tr>
                                );
                            })}
                    </tbody>
                </table>
            </div>
            <p className="mt-3 text-xs text-gray-400">⚗ = synthetic dataset (per-item names visible)</p>
        </div>
    );
}
