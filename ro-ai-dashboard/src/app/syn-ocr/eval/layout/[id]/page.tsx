"use client";

// OCR Layout Eval — run detail (Sprint 53).
//
// Shows one run's provenance + summary metrics + per-image items. Tenant comes
// from the ?tenant= query param (set by the list page link) so the detail
// fetch hits the same tenant scope; defaults to asgard_platform.

import { useEffect, useState } from "react";
import Link from "next/link";
import { useParams, useSearchParams } from "next/navigation";
import {
    getOcrLayoutRun,
    OCR_EVAL_TENANTS,
    type OcrLayoutRunDetail,
} from "@/lib/ocr-eval-api";

function fmtDuration(ms: number | null): string {
    if (ms == null) return "—";
    return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}

/** Render an arbitrary metrics blob as compact key=value chips. */
function MetricChips({ metrics }: { metrics: Record<string, unknown> | null }) {
    if (!metrics || Object.keys(metrics).length === 0) return <span className="text-gray-300">—</span>;
    return (
        <div className="flex flex-wrap gap-1">
            {Object.entries(metrics).map(([k, v]) => (
                <span
                    key={k}
                    className="px-1.5 py-0.5 text-xs rounded bg-gray-100 text-gray-700 font-mono"
                >
                    {k}={typeof v === "number" ? v : JSON.stringify(v)}
                </span>
            ))}
        </div>
    );
}

export default function OcrLayoutEvalDetailPage() {
    const params = useParams();
    const search = useSearchParams();
    const id = String(params.id);
    const tenantParam = search.get("tenant");
    const tenant =
        tenantParam && OCR_EVAL_TENANTS.includes(tenantParam as never)
            ? tenantParam
            : OCR_EVAL_TENANTS[0];

    const [run, setRun] = useState<OcrLayoutRunDetail | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        setError(null);
        getOcrLayoutRun(id, tenant)
            .then((r) => {
                if (!cancelled) setRun(r);
            })
            .catch((e) => {
                if (!cancelled) setError(e instanceof Error ? e.message : String(e));
            })
            .finally(() => {
                if (!cancelled) setLoading(false);
            });
        return () => {
            cancelled = true;
        };
    }, [id, tenant]);

    return (
        <div className="p-6 max-w-6xl mx-auto">
            <Link href="/syn-ocr/eval" className="text-sm text-blue-600 hover:underline">
                ← Back to runs
            </Link>

            {loading && <div className="mt-6 text-gray-400">Loading…</div>}
            {error && (
                <div className="mt-6 p-3 text-sm rounded-md bg-red-50 text-red-700 border border-red-200">
                    {error}
                </div>
            )}

            {run && (
                <>
                    <h1 className="mt-3 text-2xl font-bold flex items-center gap-2">
                        <span className="font-mono">{run.eval_kind}</span>
                        {run.is_synthetic && (
                            <span title="synthetic" className="text-amber-500 text-base">
                                ⚗
                            </span>
                        )}
                    </h1>
                    <div className="mt-1 text-sm text-gray-500 flex flex-wrap gap-x-4 gap-y-1">
                        <span>tenant {run.tenant}</span>
                        <span>syn {run.syn_version}</span>
                        <span>{run.model_name}</span>
                        {run.commit_sha && <span>commit {run.commit_sha.slice(0, 8)}</span>}
                        {run.iou_threshold != null && <span>IoU≥{run.iou_threshold}</span>}
                        <span>{fmtDuration(run.duration_ms)}</span>
                    </div>
                    <p className="mt-1 text-xs text-gray-400 font-mono">id {run.id}</p>

                    {/* Summary */}
                    <section className="mt-5">
                        <h2 className="text-sm font-semibold text-gray-600 mb-2">Summary</h2>
                        <div className="flex flex-wrap gap-2 mb-2 text-sm">
                            <Stat label="images" value={run.n_images} />
                            <Stat label="GT regions" value={run.n_gt_regions} />
                            <Stat label="predictions" value={run.n_predictions} />
                        </div>
                        <pre className="p-3 text-xs bg-gray-50 border border-gray-200 rounded-md overflow-auto">
                            {JSON.stringify(run.summary, null, 2)}
                        </pre>
                    </section>

                    {/* Per-image items */}
                    <section className="mt-6">
                        <h2 className="text-sm font-semibold text-gray-600 mb-2">
                            Per-image items ({run.items.length})
                        </h2>
                        <div className="border border-gray-200 rounded-lg overflow-hidden">
                            <table className="w-full text-sm">
                                <thead className="bg-gray-50 text-gray-600">
                                    <tr>
                                        <th className="text-left px-3 py-2 font-medium">Image</th>
                                        <th className="text-right px-3 py-2 font-medium">GT</th>
                                        <th className="text-right px-3 py-2 font-medium">Pred</th>
                                        <th className="text-right px-3 py-2 font-medium">Matched</th>
                                        <th className="text-left px-3 py-2 font-medium">Metrics</th>
                                        <th className="text-right px-3 py-2 font-medium">Latency</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {run.items.length === 0 && (
                                        <tr>
                                            <td colSpan={6} className="px-3 py-6 text-center text-gray-400">
                                                No per-image items.
                                            </td>
                                        </tr>
                                    )}
                                    {run.items.map((it) => {
                                        const missed = it.n_matched < it.n_gt;
                                        return (
                                            <tr
                                                key={it.id}
                                                className={`border-t border-gray-100 ${missed ? "bg-amber-50" : ""}`}
                                            >
                                                <td className="px-3 py-2 font-mono text-gray-700">
                                                    {it.image_name ?? (
                                                        <span title="hash-only (real data)">
                                                            {it.image_hash
                                                                ? `${it.image_hash.slice(0, 12)}…`
                                                                : "—"}
                                                        </span>
                                                    )}
                                                </td>
                                                <td className="px-3 py-2 text-right">{it.n_gt}</td>
                                                <td className="px-3 py-2 text-right">{it.n_pred}</td>
                                                <td
                                                    className={`px-3 py-2 text-right ${missed ? "text-amber-700 font-medium" : ""}`}
                                                >
                                                    {it.n_matched}
                                                </td>
                                                <td className="px-3 py-2">
                                                    <MetricChips metrics={it.metrics} />
                                                </td>
                                                <td className="px-3 py-2 text-right text-gray-500">
                                                    {it.latency_ms != null ? `${it.latency_ms}ms` : "—"}
                                                </td>
                                            </tr>
                                        );
                                    })}
                                </tbody>
                            </table>
                        </div>
                        <p className="mt-2 text-xs text-gray-400">
                            Rows where matched &lt; GT are highlighted. Real-data images show a hash
                            (no PHI); synthetic images show the file name.
                        </p>
                    </section>
                </>
            )}
        </div>
    );
}

function Stat({ label, value }: { label: string; value: number | null }) {
    return (
        <span className="px-3 py-1.5 rounded-md bg-gray-100 text-gray-700">
            <span className="font-semibold">{value ?? "—"}</span>{" "}
            <span className="text-gray-500">{label}</span>
        </span>
    );
}
