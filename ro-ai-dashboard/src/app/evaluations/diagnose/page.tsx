"use client";

// Sprint 47 B-47f — Bottleneck attribution dashboard.
//
// Surfaces the GET /api/v1/eval/runs/:id/diagnose API. Given a target run
// (and an optional baseline run via `?vs=`), shows:
//   - Aggregate HBp + per-dim scores for both
//   - Δ HBp + per-dim deltas
//   - Per-question attribution (RAG helped / neutral / hurt / both failed)
//   - Plain-language summary + recommended next-sprint focus
//
// Works for any paired A/B run (e.g. RAG-on vs RAG-off, LoRA vs base, etc.).

import { useEffect, useState, Suspense } from "react";
import Link from "next/link";
import { useSearchParams, useRouter } from "next/navigation";
import { authFetch, API_BASE_URL } from "@/lib/api";

// Pre-baked example for the Sprint 47 B-47e A/B that we already ran.
const EXAMPLE_TARGET = "2d63aa95-7677-40f1-a068-39c206fff090";  // RAG-on
const EXAMPLE_VS = "8fe05299-1566-4715-babf-641e60cd880a";       // RAG-off

interface Aggregate {
    run_id: string;
    name: string | null;
    n: number;
    hbp: number;
    avg_accuracy: number;
    avg_completeness: number;
    avg_relevance: number;
    avg_safety: number;
    unsafe_count: number;
    avg_latency_ms: number;
    rag_chunks_avg: number | null;
}

interface Delta {
    hbp_delta_pp: number;
    accuracy_delta: number;
    completeness_delta: number;
    relevance_delta: number;
    safety_delta: number;
    n_rag_helped: number;
    n_rag_neutral: number;
    n_rag_hurt: number;
    n_both_failed: number;
    n_both_passed: number;
}

interface DiagnoseResponse {
    target: Aggregate;
    baseline: Aggregate | null;
    delta: Delta | null;
    summary: string;
    recommendation: string;
}

function DiagnosePageInner() {
    const params = useSearchParams();
    const router = useRouter();
    const [runId, setRunId] = useState<string>(params.get("id") || "");
    const [vsId, setVsId] = useState<string>(params.get("vs") || "");
    const [data, setData] = useState<DiagnoseResponse | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const load = async (id: string, vs: string) => {
        if (!id.trim()) return;
        setLoading(true);
        setError(null);
        try {
            const url = new URL(`${API_BASE_URL}/eval/runs/${id}/diagnose`);
            if (vs.trim()) url.searchParams.set("vs", vs);
            const res = await authFetch(url.toString(), { cache: "no-store" });
            if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
            setData((await res.json()) as DiagnoseResponse);
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "fetch failed");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        if (params.get("id")) load(params.get("id")!, params.get("vs") || "");
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    const submit = (e: React.FormEvent) => {
        e.preventDefault();
        const url = new URL(window.location.pathname, window.location.origin);
        url.searchParams.set("id", runId);
        if (vsId.trim()) url.searchParams.set("vs", vsId);
        router.push(`${url.pathname}?${url.searchParams.toString()}`);
        load(runId, vsId);
    };

    const useExample = () => {
        setRunId(EXAMPLE_TARGET);
        setVsId(EXAMPLE_VS);
        load(EXAMPLE_TARGET, EXAMPLE_VS);
    };

    return (
        <main className="container mx-auto p-6 max-w-6xl">
            <header className="mb-6">
                <Link href="/evaluations" className="text-sm text-blue-600 hover:underline">
                    ← Back to /evaluations
                </Link>
                <h1 className="text-2xl font-semibold mt-2">RAG Bottleneck Diagnose (B-47f)</h1>
                <p className="text-sm text-gray-600 mt-1">
                    Sprint 47 attribution: given a target run + optional paired baseline, classify
                    each question as RAG-helped, RAG-neutral, RAG-hurt, both-failed, or both-passed.
                    Drives Sprint 39d direction (corpus retrain vs retrieval enhancement).
                </p>
            </header>

            <form onSubmit={submit} className="border rounded p-4 mb-6 bg-gray-50">
                <div className="grid grid-cols-2 gap-3">
                    <label className="text-sm">
                        Target run_id
                        <input
                            type="text"
                            value={runId}
                            onChange={(e) => setRunId(e.target.value)}
                            placeholder="e.g. 2d63aa95-7677-..."
                            className="block w-full border rounded px-2 py-1 mt-1 font-mono text-xs"
                        />
                    </label>
                    <label className="text-sm">
                        Baseline run_id (optional, for paired Δ)
                        <input
                            type="text"
                            value={vsId}
                            onChange={(e) => setVsId(e.target.value)}
                            placeholder="e.g. 8fe05299-1566-..."
                            className="block w-full border rounded px-2 py-1 mt-1 font-mono text-xs"
                        />
                    </label>
                </div>
                <div className="mt-3 flex gap-2">
                    <button
                        type="submit"
                        disabled={loading || !runId.trim()}
                        className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:bg-gray-300"
                    >
                        {loading ? "Loading…" : "Diagnose"}
                    </button>
                    <button
                        type="button"
                        onClick={useExample}
                        className="px-4 py-1.5 bg-gray-200 text-gray-800 rounded text-sm hover:bg-gray-300"
                    >
                        Try Sprint 47 B-47e example (RAG on vs off)
                    </button>
                </div>
                {error && <p className="text-sm text-red-600 mt-2">⚠ {error}</p>}
            </form>

            {data && (
                <>
                    {/* Verdict banner */}
                    <section className="border-l-4 border-blue-500 bg-blue-50 px-4 py-3 mb-6 rounded">
                        <p className="text-base font-medium text-blue-900">{data.summary}</p>
                        <p className="text-sm text-blue-800 mt-1"><strong>Recommendation:</strong> {data.recommendation}</p>
                    </section>

                    {/* Aggregate comparison */}
                    <section className="mb-6">
                        <h2 className="font-medium mb-2">Aggregate scores</h2>
                        <table className="w-full text-sm border-collapse">
                            <thead className="text-xs text-gray-600 border-b">
                                <tr>
                                    <th className="text-left p-2">Run</th>
                                    <th className="text-right p-2">n</th>
                                    <th className="text-right p-2">HBp%</th>
                                    <th className="text-right p-2">Acc</th>
                                    <th className="text-right p-2">Comp</th>
                                    <th className="text-right p-2">Rel</th>
                                    <th className="text-right p-2">Safety</th>
                                    <th className="text-right p-2">Unsafe</th>
                                    <th className="text-right p-2">Lat (s)</th>
                                    <th className="text-right p-2">Chunks avg</th>
                                </tr>
                            </thead>
                            <tbody>
                                <Row label={`Target: ${data.target.name ?? data.target.run_id.slice(0, 8)}`} a={data.target} />
                                {data.baseline && (
                                    <Row label={`Baseline: ${data.baseline.name ?? data.baseline.run_id.slice(0, 8)}`} a={data.baseline} />
                                )}
                                {data.delta && (
                                    <tr className="border-t-2 font-medium bg-yellow-50">
                                        <td className="p-2">Δ (target − baseline)</td>
                                        <td className="p-2 text-right text-gray-400">—</td>
                                        <td className={`p-2 text-right ${deltaColor(data.delta.hbp_delta_pp, 5)}`}>
                                            {data.delta.hbp_delta_pp >= 0 ? "+" : ""}{data.delta.hbp_delta_pp.toFixed(1)}pp
                                        </td>
                                        <td className={`p-2 text-right ${deltaColor(data.delta.accuracy_delta * 4, 0.5)}`}>
                                            {data.delta.accuracy_delta >= 0 ? "+" : ""}{data.delta.accuracy_delta.toFixed(2)}
                                        </td>
                                        <td className={`p-2 text-right ${deltaColor(data.delta.completeness_delta * 4, 0.5)}`}>
                                            {data.delta.completeness_delta >= 0 ? "+" : ""}{data.delta.completeness_delta.toFixed(2)}
                                        </td>
                                        <td className={`p-2 text-right ${deltaColor(data.delta.relevance_delta * 4, 0.5)}`}>
                                            {data.delta.relevance_delta >= 0 ? "+" : ""}{data.delta.relevance_delta.toFixed(2)}
                                        </td>
                                        <td className={`p-2 text-right ${deltaColor(data.delta.safety_delta, 0.1)}`}>
                                            {data.delta.safety_delta >= 0 ? "+" : ""}{data.delta.safety_delta.toFixed(2)}
                                        </td>
                                        <td className="p-2 text-right text-gray-400">—</td>
                                        <td className="p-2 text-right text-gray-400">—</td>
                                        <td className="p-2 text-right text-gray-400">—</td>
                                    </tr>
                                )}
                            </tbody>
                        </table>
                    </section>

                    {/* Per-question attribution */}
                    {data.delta && (
                        <section className="mb-6">
                            <h2 className="font-medium mb-2">Per-question attribution (RAG vs LLM bottleneck)</h2>
                            <div className="grid grid-cols-5 gap-3">
                                <AttributionCard
                                    label="RAG helped"
                                    value={data.delta.n_rag_helped}
                                    total={totalQuestions(data.delta)}
                                    color="green"
                                    note="target Δ ≥ +0.10 HBp"
                                />
                                <AttributionCard
                                    label="RAG neutral"
                                    value={data.delta.n_rag_neutral}
                                    total={totalQuestions(data.delta)}
                                    color="gray"
                                    note="|Δ| < 0.10"
                                />
                                <AttributionCard
                                    label="RAG hurt"
                                    value={data.delta.n_rag_hurt}
                                    total={totalQuestions(data.delta)}
                                    color="red"
                                    note="baseline ≥ +0.10 vs target"
                                />
                                <AttributionCard
                                    label="Both passed"
                                    value={data.delta.n_both_passed}
                                    total={totalQuestions(data.delta)}
                                    color="blue"
                                    note="both ≥ 0.7 HBp · easy questions"
                                />
                                <AttributionCard
                                    label="Both failed"
                                    value={data.delta.n_both_failed}
                                    total={totalQuestions(data.delta)}
                                    color="orange"
                                    note="both < 0.5 · neither RAG nor LLM enough"
                                />
                            </div>
                        </section>
                    )}

                    {/* Raw JSON for power users */}
                    <details className="mt-6 text-xs text-gray-500">
                        <summary className="cursor-pointer hover:text-gray-700">Raw JSON</summary>
                        <pre className="mt-2 bg-gray-50 p-3 rounded overflow-auto">
                            {JSON.stringify(data, null, 2)}
                        </pre>
                    </details>
                </>
            )}
        </main>
    );
}

function Row({ label, a }: { label: string; a: Aggregate }) {
    return (
        <tr className="border-b">
            <td className="p-2 font-mono text-xs">{label}</td>
            <td className="p-2 text-right">{a.n}</td>
            <td className="p-2 text-right font-medium">{a.hbp.toFixed(1)}%</td>
            <td className="p-2 text-right">{a.avg_accuracy.toFixed(2)}</td>
            <td className="p-2 text-right">{a.avg_completeness.toFixed(2)}</td>
            <td className="p-2 text-right">{a.avg_relevance.toFixed(2)}</td>
            <td className="p-2 text-right">{a.avg_safety.toFixed(2)}</td>
            <td className="p-2 text-right">{a.unsafe_count > 0 ? <span className="text-red-600">{a.unsafe_count}</span> : "0"}</td>
            <td className="p-2 text-right text-gray-600">{(a.avg_latency_ms / 1000).toFixed(1)}</td>
            <td className="p-2 text-right text-gray-600">{a.rag_chunks_avg !== null ? a.rag_chunks_avg.toFixed(1) : "—"}</td>
        </tr>
    );
}

function AttributionCard({
    label, value, total, color, note,
}: { label: string; value: number; total: number; color: string; note: string }) {
    const pct = total > 0 ? ((value / total) * 100).toFixed(0) : "0";
    const colorClass: Record<string, string> = {
        green: "border-green-300 bg-green-50",
        red: "border-red-300 bg-red-50",
        gray: "border-gray-300 bg-gray-50",
        blue: "border-blue-300 bg-blue-50",
        orange: "border-orange-300 bg-orange-50",
    };
    return (
        <div className={`border rounded p-3 ${colorClass[color] || colorClass.gray}`}>
            <div className="text-xs text-gray-600">{label}</div>
            <div className="text-2xl font-semibold mt-1">{value}</div>
            <div className="text-xs text-gray-500 mt-1">{pct}% of {total}</div>
            <div className="text-xs text-gray-400 mt-1">{note}</div>
        </div>
    );
}

function totalQuestions(d: Delta): number {
    return d.n_rag_helped + d.n_rag_neutral + d.n_rag_hurt + d.n_both_failed + d.n_both_passed;
}

function deltaColor(value: number, threshold: number): string {
    if (value >= threshold) return "text-green-700";
    if (value <= -threshold) return "text-red-700";
    return "text-gray-700";
}

export default function DiagnosePage() {
    return (
        <Suspense fallback={<div className="p-6">Loading…</div>}>
            <DiagnosePageInner />
        </Suspense>
    );
}
