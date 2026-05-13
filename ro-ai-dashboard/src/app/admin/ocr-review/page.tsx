"use client";

// Admin-gated Curator review queue for OCR documents flagged as pending review.
// Flags are set by syn-api when: confidence < 0.7 OR high_stakes=true on the call.
//
// Endpoints (Syn API):
//   GET  /syn/ocr/review-queue?limit=N&offset=N  → {tenant_id, limit, offset, rows}
//   POST /syn/ocr/documents/{id}/review          → {decision, note, reviewed_by}
//
// Visible only when isAdmin in navbar (Admin group filter, same as /admin/skuggi).

import { useEffect, useState, useCallback } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { authFetch, SYN_API_BASE_URL } from "@/lib/api";
import {
    RefreshCw,
    CheckCircle,
    XCircle,
    ChevronLeft,
    ChevronRight,
    ClipboardList,
    Loader2,
    AlertCircle,
    Info,
} from "lucide-react";

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

const PAGE_SIZE = 20;

function engineBadgeClass(engine: string): string {
    if (engine.startsWith("gemini")) return "bg-sky-100 text-sky-800 dark:bg-sky-900/40 dark:text-sky-300";
    if (engine === "typhoon-local") return "bg-violet-100 text-violet-800 dark:bg-violet-900/40 dark:text-violet-300";
    if (engine === "paddleocr-local") return "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300";
    return "bg-slate-100 text-slate-600 dark:bg-zinc-800 dark:text-zinc-400";
}

function ConfidenceBadge({ value }: { value: number | null }) {
    if (value == null) return <span className="text-xs text-muted-foreground">—</span>;
    const cls =
        value < 0.5
            ? "bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-300"
            : value < 0.7
            ? "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/40 dark:text-yellow-300"
            : "bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-300";
    return (
        <span className={`text-xs px-1.5 py-0.5 rounded font-medium ${cls}`}>
            {(value * 100).toFixed(0)}%
        </span>
    );
}

export default function CuratorReviewQueuePage() {
    const [data, setData] = useState<QueueResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [page, setPage] = useState(0);

    const [notes, setNotes] = useState<Record<string, string>>({});
    const [busy, setBusy] = useState<Record<string, boolean>>({});
    const [decided, setDecided] = useState<Record<string, "approved" | "rejected">>({});

    const load = useCallback(async (p: number) => {
        setLoading(true);
        setError(null);
        try {
            const qs = new URLSearchParams({
                limit: String(PAGE_SIZE),
                offset: String(p * PAGE_SIZE),
            });
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/review-queue?${qs}`, {
                cache: "no-store",
            });
            if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
            setData(await r.json());
        } catch (e: any) {
            setError(e.message ?? "fetch failed");
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        load(page);
    }, [load, page]);

    const decide = useCallback(async (id: string, decision: "approved" | "rejected") => {
        setBusy((b) => ({ ...b, [id]: true }));
        try {
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/documents/${id}/review`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ decision, note: notes[id] || null }),
            });
            if (!r.ok) {
                const txt = await r.text();
                setError(`${r.status} ${txt}`);
                return;
            }
            setDecided((d) => ({ ...d, [id]: decision }));
        } catch (e: any) {
            setError(e.message ?? "decide failed");
        } finally {
            setBusy((b) => ({ ...b, [id]: false }));
        }
    }, [notes]);

    const rows = data?.rows ?? [];
    const pendingCount = rows.filter((r) => !decided[r.id]).length;
    const hasNext = rows.length === PAGE_SIZE;

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            <div className="flex items-center justify-between flex-wrap gap-3">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <ClipboardList className="w-6 h-6 text-amber-500" />
                        Curator Review Queue
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1">
                        OCR documents flagged for review — confidence &lt; 0.7 or high-stakes.
                        Approve or reject; the audit row stays in ocr_documents.
                    </p>
                </div>
                <Button variant="outline" onClick={() => load(page)} disabled={loading}>
                    <RefreshCw className={`w-4 h-4 mr-2 ${loading ? "animate-spin" : ""}`} />
                    Refresh
                </Button>
            </div>

            {error && (
                <div className="p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 flex items-center gap-2 text-sm">
                    <AlertCircle className="w-5 h-5 flex-shrink-0" />
                    {error}
                </div>
            )}

            <Card>
                <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Info className="w-4 h-4" />
                                Pending Review
                                {!loading && (
                                    <span className="ml-2 px-2 py-0.5 rounded-full text-xs bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300 font-medium">
                                        {pendingCount} left on this page
                                    </span>
                                )}
                            </CardTitle>
                            <CardDescription className="mt-1">
                                Page {page + 1} · {PAGE_SIZE} per page
                            </CardDescription>
                        </div>
                        <div className="flex items-center gap-2">
                            <Button
                                variant="outline" size="sm"
                                onClick={() => setPage((p) => Math.max(0, p - 1))}
                                disabled={page === 0 || loading}
                            >
                                <ChevronLeft className="w-4 h-4" />
                            </Button>
                            <Button
                                variant="outline" size="sm"
                                onClick={() => setPage((p) => p + 1)}
                                disabled={!hasNext || loading}
                            >
                                <ChevronRight className="w-4 h-4" />
                            </Button>
                        </div>
                    </div>
                </CardHeader>
                <CardContent className="p-0">
                    {loading ? (
                        <div className="flex items-center justify-center py-16">
                            <Loader2 className="w-6 h-6 animate-spin text-amber-500" />
                        </div>
                    ) : rows.length === 0 ? (
                        <div className="text-center py-16 text-muted-foreground">
                            <ClipboardList className="w-12 h-12 mx-auto mb-3 opacity-20" />
                            <p className="text-sm">Queue is empty — no documents pending review.</p>
                        </div>
                    ) : (
                        <div className="divide-y dark:divide-zinc-800">
                            {rows.map((item) => {
                                const isDone = !!decided[item.id];
                                const verdict = decided[item.id];
                                return (
                                    <div
                                        key={item.id}
                                        className={`p-4 transition-colors ${
                                            isDone
                                                ? "opacity-50 bg-muted/30"
                                                : "hover:bg-muted/20"
                                        }`}
                                    >
                                        {/* Header row */}
                                        <div className="flex items-start gap-3">
                                            <div className="flex-1 min-w-0 space-y-2">
                                                {/* Metadata badges */}
                                                <div className="flex items-center flex-wrap gap-2 text-xs">
                                                    <span className={`px-2 py-0.5 rounded font-medium ${engineBadgeClass(item.engine_used)}`}>
                                                        {item.engine_used}
                                                    </span>
                                                    <ConfidenceBadge value={item.confidence} />
                                                    {item.router_reason && (
                                                        <span className="text-muted-foreground">
                                                            {item.router_reason}
                                                        </span>
                                                    )}
                                                    <span className="text-muted-foreground">
                                                        ${item.cost_usd.toFixed(5)}
                                                    </span>
                                                    {item.latency_ms != null && (
                                                        <span className="text-muted-foreground">
                                                            {item.latency_ms}ms
                                                        </span>
                                                    )}
                                                    <span className="text-muted-foreground">
                                                        {new Date(item.created_at).toLocaleString()}
                                                    </span>
                                                </div>

                                                {/* SHA + audit ID */}
                                                <div className="text-xs font-mono text-muted-foreground">
                                                    sha256:{item.image_sha256.slice(0, 16)}… · id:{item.id.slice(0, 8)}…
                                                </div>

                                                {/* Extracted text */}
                                                <div className="max-h-40 overflow-y-auto rounded bg-muted/50 p-2 text-sm font-mono whitespace-pre-wrap leading-relaxed">
                                                    {item.extracted_text || (
                                                        <span className="text-muted-foreground italic">no text extracted</span>
                                                    )}
                                                </div>

                                                {/* Note input */}
                                                {!isDone && (
                                                    <Input
                                                        placeholder="Optional note (PHI-safe — short)"
                                                        value={notes[item.id] ?? ""}
                                                        onChange={(e) =>
                                                            setNotes((n) => ({ ...n, [item.id]: e.target.value }))
                                                        }
                                                        className="text-sm h-8"
                                                    />
                                                )}
                                            </div>

                                            {/* Action buttons */}
                                            <div className="flex flex-col gap-2 shrink-0">
                                                {isDone ? (
                                                    <div className={`flex items-center gap-1.5 text-sm font-medium ${verdict === "approved" ? "text-green-600" : "text-red-600"}`}>
                                                        {verdict === "approved"
                                                            ? <CheckCircle className="w-4 h-4" />
                                                            : <XCircle className="w-4 h-4" />}
                                                        {verdict}
                                                    </div>
                                                ) : (
                                                    <>
                                                        <Button
                                                            size="sm"
                                                            onClick={() => decide(item.id, "approved")}
                                                            disabled={busy[item.id]}
                                                            className="bg-green-600 hover:bg-green-700 text-white"
                                                        >
                                                            {busy[item.id]
                                                                ? <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                                                : <CheckCircle className="w-3.5 h-3.5 mr-1" />}
                                                            Approve
                                                        </Button>
                                                        <Button
                                                            size="sm"
                                                            variant="destructive"
                                                            onClick={() => decide(item.id, "rejected")}
                                                            disabled={busy[item.id]}
                                                        >
                                                            {busy[item.id]
                                                                ? <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                                                : <XCircle className="w-3.5 h-3.5 mr-1" />}
                                                            Reject
                                                        </Button>
                                                    </>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* Summary row after decisions */}
            {Object.keys(decided).length > 0 && (
                <div className="flex items-center gap-4 text-sm text-muted-foreground p-3 bg-muted/30 rounded-lg">
                    <span className="text-green-600 font-medium">
                        ✓ {Object.values(decided).filter((d) => d === "approved").length} approved
                    </span>
                    <span className="text-red-600 font-medium">
                        ✗ {Object.values(decided).filter((d) => d === "rejected").length} rejected
                    </span>
                    <span className="ml-auto text-xs">
                        Refresh to clear decided items and see new pending
                    </span>
                    <Button variant="outline" size="sm" onClick={() => { setDecided({}); setNotes({}); load(page); }}>
                        <RefreshCw className="w-3.5 h-3.5 mr-1.5" /> Refresh
                    </Button>
                </div>
            )}
        </div>
    );
}
