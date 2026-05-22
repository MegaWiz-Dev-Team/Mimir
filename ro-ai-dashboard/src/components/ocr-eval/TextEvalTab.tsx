"use client";

// Text eval tab — CER/WER per engine (ocr_eval_*). Metrics only (no raw text).
// Tenant is owned by the parent OCR Eval page and passed in.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { listOcrTextRuns, type OcrTextRunSummary } from "@/lib/ocr-eval-api";

function fmtDate(iso: string | null): string {
    return iso ? new Date(iso).toLocaleString() : "—";
}

export default function TextEvalTab({ tenant }: { tenant: string }) {
    const [runs, setRuns] = useState<OcrTextRunSummary[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const load = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const res = await listOcrTextRuns({ tenant, limit: 100 });
            setRuns(res.runs);
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
            setRuns([]);
        } finally {
            setLoading(false);
        }
    }, [tenant]);

    useEffect(() => {
        load();
    }, [load]);

    return (
        <div>
            <div className="flex items-center justify-end mb-3">
                <button
                    onClick={load}
                    className="px-3 py-1.5 text-sm rounded-md border border-gray-300 hover:bg-gray-50"
                >
                    🔄 Refresh
                </button>
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
                            <th className="text-left px-3 py-2 font-medium">Run</th>
                            <th className="text-left px-3 py-2 font-medium">Dataset</th>
                            <th className="text-left px-3 py-2 font-medium">Engines</th>
                            <th className="text-right px-3 py-2 font-medium">Results</th>
                            <th className="text-left px-3 py-2 font-medium">Started</th>
                        </tr>
                    </thead>
                    <tbody>
                        {loading && (
                            <tr>
                                <td colSpan={5} className="px-3 py-6 text-center text-gray-400">
                                    Loading…
                                </td>
                            </tr>
                        )}
                        {!loading && runs.length === 0 && (
                            <tr>
                                <td colSpan={5} className="px-3 py-6 text-center text-gray-400">
                                    No text runs for {tenant}.
                                </td>
                            </tr>
                        )}
                        {!loading &&
                            runs.map((run) => (
                                <tr key={run.id} className="border-t border-gray-100 hover:bg-blue-50">
                                    <td className="px-3 py-2">
                                        <Link
                                            href={`/syn-ocr/eval/text/${run.id}?tenant=${tenant}`}
                                            className="text-blue-600 hover:underline"
                                        >
                                            {run.name ?? run.id.slice(0, 8)}
                                        </Link>
                                    </td>
                                    <td className="px-3 py-2 text-gray-700">{run.dataset_name}</td>
                                    <td className="px-3 py-2">
                                        <div className="flex flex-wrap gap-1">
                                            {(run.engines ?? []).map((e) => (
                                                <span
                                                    key={e}
                                                    className="px-1.5 py-0.5 text-xs rounded bg-gray-100 text-gray-700 font-mono"
                                                >
                                                    {e}
                                                </span>
                                            ))}
                                        </div>
                                    </td>
                                    <td className="px-3 py-2 text-right">{run.n_results ?? "—"}</td>
                                    <td className="px-3 py-2 text-gray-500">{fmtDate(run.started_at)}</td>
                                </tr>
                            ))}
                    </tbody>
                </table>
            </div>
            <p className="mt-3 text-xs text-gray-400">
                Metrics only — raw OCR text and ground truth are not exposed (PHI).
            </p>
        </div>
    );
}
