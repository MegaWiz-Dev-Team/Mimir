"use client";

// Sprint 50 B-50f — Curator review queue.
//
// Lists ocr_documents rows with review_status='pending' for the current
// tenant. Each row gets approve / reject buttons; a note field is optional.
//
// Auto-flag rules (server-side, in syn-api do_extract):
//   - status='succeeded' AND confidence < 0.7
//   - status='succeeded' AND high_stakes=true was set on the call
//
// The Curator decision lands on the same row (review_status, reviewed_by,
// reviewed_at, review_note) — no separate review table.

import { useEffect, useState } from "react";
import Link from "next/link";
import { authFetch, SYN_API_BASE_URL } from "@/lib/api";

interface ReviewItem {
    id: string;
    tenant_id: string;
    image_sha256: string;
    engine_used: string;
    router_reason: string | null;
    confidence: number | null;
    extracted_text: string | null;
    cost_usd: number;
    latency_ms: number | null;
    status: string;
    review_status: string | null;
    review_note: string | null;
    reviewed_by: string | null;
    reviewed_at: string | null;
    created_at: string;
}

interface QueueResponse {
    tenant_id: string;
    limit: number;
    offset: number;
    rows: ReviewItem[];
}

function engineBadge(engine: string): string {
    if (engine.startsWith("gemini")) return "bg-sky-100 text-sky-800";
    if (engine === "chandra-local") return "bg-indigo-100 text-indigo-800";
    if (engine === "paddleocr-local") return "bg-emerald-100 text-emerald-800";
    return "bg-slate-100 text-slate-800";
}

export default function SynOcrReviewPage() {
    const [data, setData] = useState<QueueResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [notes, setNotes] = useState<Record<string, string>>({});
    const [busy, setBusy] = useState<Record<string, boolean>>({});

    const load = async () => {
        setLoading(true);
        setError(null);
        try {
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/review-queue?limit=100`, {
                cache: "no-store",
            });
            if (!r.ok) {
                setError(`${r.status} ${r.statusText}`);
                return;
            }
            setData((await r.json()) as QueueResponse);
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "fetch failed");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        load();
    }, []);

    const decide = async (id: string, decision: "approved" | "rejected") => {
        setBusy((b) => ({ ...b, [id]: true }));
        try {
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/documents/${id}/review`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    decision,
                    note: notes[id] || null,
                }),
            });
            if (!r.ok) {
                const text = await r.text();
                setError(`${r.status} ${text}`);
                return;
            }
            setData((prev) =>
                prev ? { ...prev, rows: prev.rows.filter((x) => x.id !== id) } : prev
            );
            setNotes((n) => {
                const copy = { ...n };
                delete copy[id];
                return copy;
            });
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "decide failed");
        } finally {
            setBusy((b) => ({ ...b, [id]: false }));
        }
    };

    const rows = data?.rows ?? [];

    return (
        <div className="p-6">
            <div className="flex items-center justify-between mb-2">
                <div>
                    <h1 className="text-2xl font-bold">Curator Review Queue</h1>
                    <p className="text-sm text-slate-600">
                        OCR calls flagged for review — low confidence (&lt; 0.7) or
                        Curator-marked high-stakes. Decide approve or reject; the row stays in
                        ocr_documents but exits the queue.
                    </p>
                </div>
                <div className="flex gap-2">
                    <Link
                        href="/syn-ocr"
                        className="px-3 py-1.5 text-sm bg-slate-200 hover:bg-slate-300 rounded"
                    >
                        ← Audit log
                    </Link>
                    <button
                        onClick={load}
                        className="px-3 py-1.5 text-sm bg-slate-200 hover:bg-slate-300 rounded"
                    >
                        {loading ? "…" : "Refresh"}
                    </button>
                </div>
            </div>

            {error && (
                <div className="mb-4 p-3 bg-red-50 border border-red-200 text-red-800 rounded text-sm">
                    {error}
                </div>
            )}

            {loading ? (
                <div className="text-sm text-slate-500">Loading…</div>
            ) : rows.length === 0 ? (
                <div className="p-6 bg-white border rounded text-center text-slate-500">
                    Queue is empty — nothing pending review.
                </div>
            ) : (
                <div className="space-y-3">
                    {rows.map((r) => (
                        <div key={r.id} className="bg-white border rounded p-4 shadow-sm">
                            <div className="flex items-start justify-between gap-3">
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-2 mb-1 text-xs">
                                        <span className={`px-2 py-0.5 rounded ${engineBadge(r.engine_used)}`}>
                                            {r.engine_used}
                                        </span>
                                        <span className="text-slate-500 font-mono">
                                            reason: {r.router_reason ?? "—"}
                                        </span>
                                        {r.confidence != null && (
                                            <span
                                                className={`px-2 py-0.5 rounded ${
                                                    r.confidence < 0.5
                                                        ? "bg-red-100 text-red-800"
                                                        : "bg-yellow-100 text-yellow-800"
                                                }`}
                                            >
                                                conf {r.confidence.toFixed(2)}
                                            </span>
                                        )}
                                        <span className="text-slate-500">
                                            ${r.cost_usd.toFixed(5)} · {r.latency_ms}ms
                                        </span>
                                        <span className="text-slate-400">
                                            {new Date(r.created_at).toLocaleString()}
                                        </span>
                                    </div>
                                    <div className="text-xs font-mono text-slate-500 truncate">
                                        sha256: {r.image_sha256.slice(0, 16)}… · audit_id:{" "}
                                        {r.id.slice(0, 8)}…
                                    </div>
                                    <div className="mt-2 max-h-48 overflow-y-auto bg-slate-50 p-2 rounded text-sm font-mono whitespace-pre-wrap">
                                        {r.extracted_text || (
                                            <span className="text-slate-400 italic">
                                                no text extracted
                                            </span>
                                        )}
                                    </div>
                                    <input
                                        type="text"
                                        placeholder="Optional note (PHI-safe — short)"
                                        value={notes[r.id] || ""}
                                        onChange={(e) =>
                                            setNotes((n) => ({ ...n, [r.id]: e.target.value }))
                                        }
                                        className="mt-2 w-full text-sm border rounded px-2 py-1"
                                    />
                                </div>
                                <div className="flex flex-col gap-2 shrink-0">
                                    <button
                                        onClick={() => decide(r.id, "approved")}
                                        disabled={busy[r.id]}
                                        className="px-4 py-1.5 text-sm bg-green-600 hover:bg-green-700 text-white rounded font-semibold disabled:opacity-50"
                                    >
                                        Approve
                                    </button>
                                    <button
                                        onClick={() => decide(r.id, "rejected")}
                                        disabled={busy[r.id]}
                                        className="px-4 py-1.5 text-sm bg-red-600 hover:bg-red-700 text-white rounded font-semibold disabled:opacity-50"
                                    >
                                        Reject
                                    </button>
                                </div>
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
