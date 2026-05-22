"use client";

// OCR Text Eval — run detail. Per-engine CER/WER summary + per-(case,engine)
// metrics. Metrics only — no raw OCR text / ground truth (PHI). Tenant from
// ?tenant= (set by the list link), default asgard_platform.

import { useEffect, useState } from "react";
import Link from "next/link";
import { useParams, useSearchParams } from "next/navigation";
import {
    getOcrTextRun,
    OCR_EVAL_TENANTS,
    type OcrTextRunDetail,
} from "@/lib/ocr-eval-api";

const fmt = (v: number | null, digits = 4) => (v == null ? "—" : v.toFixed(digits));
const fmtMs = (v: number | null) => (v == null ? "—" : `${Math.round(v)}ms`);

export default function OcrTextEvalDetailPage() {
    const params = useParams();
    const search = useSearchParams();
    const id = String(params.id);
    const tenantParam = search.get("tenant");
    const tenant =
        tenantParam && OCR_EVAL_TENANTS.includes(tenantParam as never)
            ? tenantParam
            : OCR_EVAL_TENANTS[0];

    const [run, setRun] = useState<OcrTextRunDetail | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        setError(null);
        getOcrTextRun(id, tenant)
            .then((r) => !cancelled && setRun(r))
            .catch((e) => !cancelled && setError(e instanceof Error ? e.message : String(e)))
            .finally(() => !cancelled && setLoading(false));
        return () => {
            cancelled = true;
        };
    }, [id, tenant]);

    return (
        <div className="p-6 max-w-6xl mx-auto">
            <Link href="/syn-ocr/eval" className="text-sm text-blue-600 hover:underline">
                ← Back to OCR Eval
            </Link>

            {loading && <div className="mt-6 text-gray-400">Loading…</div>}
            {error && (
                <div className="mt-6 p-3 text-sm rounded-md bg-red-50 text-red-700 border border-red-200">
                    {error}
                </div>
            )}

            {run && (
                <>
                    <h1 className="mt-3 text-2xl font-bold">{run.name ?? "Text eval run"}</h1>
                    <div className="mt-1 text-sm text-gray-500 flex flex-wrap gap-x-4 gap-y-1">
                        <span>tenant {run.tenant}</span>
                        <span>dataset {run.dataset_name}</span>
                        <span>source {run.dataset_source}</span>
                        {run.prompt_label && <span>prompt {run.prompt_label}</span>}
                        {run.started_at && <span>{new Date(run.started_at).toLocaleString()}</span>}
                    </div>
                    <p className="mt-1 text-xs text-gray-400 font-mono">id {run.id}</p>

                    {/* Per-engine summary */}
                    <section className="mt-5">
                        <h2 className="text-sm font-semibold text-gray-600 mb-2">Per-engine summary</h2>
                        <div className="border border-gray-200 rounded-lg overflow-hidden">
                            <table className="w-full text-sm">
                                <thead className="bg-gray-50 text-gray-600">
                                    <tr>
                                        <th className="text-left px-3 py-2 font-medium">Engine</th>
                                        <th className="text-right px-3 py-2 font-medium">Cases</th>
                                        <th className="text-right px-3 py-2 font-medium">OK</th>
                                        <th className="text-right px-3 py-2 font-medium">Mean CER</th>
                                        <th className="text-right px-3 py-2 font-medium">Mean WER</th>
                                        <th className="text-right px-3 py-2 font-medium">CER range</th>
                                        <th className="text-right px-3 py-2 font-medium">Mean latency</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {run.engine_summary.length === 0 && (
                                        <tr>
                                            <td colSpan={7} className="px-3 py-6 text-center text-gray-400">
                                                No results.
                                            </td>
                                        </tr>
                                    )}
                                    {run.engine_summary.map((e, i) => (
                                        <tr
                                            key={e.engine}
                                            className={`border-t border-gray-100 ${i === 0 ? "bg-green-50" : ""}`}
                                            title={i === 0 ? "lowest mean CER" : undefined}
                                        >
                                            <td className="px-3 py-2 font-mono text-gray-800">
                                                {e.engine}
                                                {i === 0 && <span className="ml-1 text-green-600">★</span>}
                                            </td>
                                            <td className="px-3 py-2 text-right">{e.n ?? "—"}</td>
                                            <td className="px-3 py-2 text-right">{e.n_ok ?? "—"}</td>
                                            <td className="px-3 py-2 text-right font-mono">{fmt(e.mean_cer)}</td>
                                            <td className="px-3 py-2 text-right font-mono">{fmt(e.mean_wer)}</td>
                                            <td className="px-3 py-2 text-right font-mono text-gray-500">
                                                {fmt(e.min_cer, 2)}–{fmt(e.max_cer, 2)}
                                            </td>
                                            <td className="px-3 py-2 text-right text-gray-500">
                                                {fmtMs(e.mean_wall_ms)}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                        <p className="mt-2 text-xs text-gray-400">★ = lowest mean CER (best). CER/WER lower is better.</p>
                    </section>

                    {/* Per-case results */}
                    <section className="mt-6">
                        <h2 className="text-sm font-semibold text-gray-600 mb-2">
                            Per-case results ({run.results.length})
                        </h2>
                        <div className="border border-gray-200 rounded-lg overflow-hidden">
                            <table className="w-full text-sm">
                                <thead className="bg-gray-50 text-gray-600">
                                    <tr>
                                        <th className="text-left px-3 py-2 font-medium">Case</th>
                                        <th className="text-left px-3 py-2 font-medium">Doc type</th>
                                        <th className="text-left px-3 py-2 font-medium">Engine</th>
                                        <th className="text-left px-3 py-2 font-medium">Status</th>
                                        <th className="text-right px-3 py-2 font-medium">CER</th>
                                        <th className="text-right px-3 py-2 font-medium">WER</th>
                                        <th className="text-right px-3 py-2 font-medium">GT / OCR chars</th>
                                        <th className="text-right px-3 py-2 font-medium">Latency</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {run.results.map((r, i) => (
                                        <tr
                                            key={`${r.case_id}-${r.engine}-${i}`}
                                            className={`border-t border-gray-100 ${r.status !== "ok" ? "bg-red-50" : ""}`}
                                        >
                                            <td className="px-3 py-2 font-mono text-gray-700">{r.case_id}</td>
                                            <td className="px-3 py-2 text-gray-500">{r.doc_type ?? "—"}</td>
                                            <td className="px-3 py-2 font-mono text-gray-700">{r.engine}</td>
                                            <td className={`px-3 py-2 ${r.status !== "ok" ? "text-red-600" : "text-gray-500"}`}>
                                                {r.status}
                                            </td>
                                            <td className="px-3 py-2 text-right font-mono">{fmt(r.cer)}</td>
                                            <td className="px-3 py-2 text-right font-mono">{fmt(r.wer)}</td>
                                            <td className="px-3 py-2 text-right text-gray-500">
                                                {r.gt_chars ?? "—"} / {r.extracted_chars ?? "—"}
                                            </td>
                                            <td className="px-3 py-2 text-right text-gray-500">{fmtMs(r.wall_ms)}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                        <p className="mt-2 text-xs text-gray-400">
                            Errors highlighted. Char counts shown instead of raw text (PHI not exposed).
                        </p>
                    </section>
                </>
            )}
        </div>
    );
}
