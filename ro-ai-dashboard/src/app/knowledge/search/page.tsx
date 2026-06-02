"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import {
    Card, CardContent, CardDescription, CardHeader, CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
    ArrowLeft, Search, Loader2, AlertCircle, Sparkles, Zap,
} from "lucide-react";
import { API_BASE_URL, authFetch } from "@/lib/api";

type KbHit = {
    kb_id: string;
    kb_name: string;
    items: Record<string, any>[];
    count: number;
    latency_ms: number;
};

type SearchResponse = {
    q: string;
    k: number;
    results: KbHit[];
    total_ms: number;
};

// ── per-KB row renderers ──────────────────────────────────────────────────
// Each KB has its own item shape (the L3 backend doesn't normalize).
// Render the most useful identity + label for the user to recognize a hit.

function ItemRow({ kbId, item }: { kbId: string; item: any }) {
    if (kbId === "icd10-tm") {
        return (
            <div className="grid grid-cols-[7rem_1fr] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-foreground">
                    {item.code}
                </code>
                <div className="min-w-0">
                    <div className="text-sm font-medium truncate">{item.en_label}</div>
                    {item.th_label && (
                        <div className="text-xs text-muted-foreground truncate">{item.th_label}</div>
                    )}
                </div>
            </div>
        );
    }
    if (kbId === "tpc") {
        return (
            <div className="grid grid-cols-[7rem_1fr] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-foreground">{item.code}</code>
                <div className="text-sm truncate">{item.en_label}</div>
            </div>
        );
    }
    if (kbId === "loinc") {
        return (
            <div className="grid grid-cols-[7rem_1fr_4rem] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-foreground">{item.loinc_num}</code>
                <div className="text-sm truncate">{item.long_common_name}</div>
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
                    {item.class}
                </span>
            </div>
        );
    }
    if (kbId === "tmt") {
        return (
            <div className="grid grid-cols-[5rem_4rem_1fr] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-foreground">{item.tmt_id}</code>
                <span className="text-[10px] uppercase font-mono text-muted-foreground">
                    {item.concept_type}
                </span>
                <div className="text-sm truncate">{item.fsn}</div>
            </div>
        );
    }
    if (kbId === "tmlt") {
        return (
            <div className="grid grid-cols-[5rem_4rem_1fr] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-foreground">{item.tmlt_id}</code>
                <span className="text-[10px] uppercase font-mono text-muted-foreground">
                    {item.concept_type}
                </span>
                <div className="text-sm truncate">{item.fsn}</div>
            </div>
        );
    }
    if (kbId === "primekg") {
        return (
            <div className="grid grid-cols-[7rem_1fr_4rem] gap-3 items-start">
                <code className="text-xs font-mono text-muted-foreground">
                    {item.entity_index}
                </code>
                <div className="min-w-0">
                    <div className="text-sm font-medium truncate">{item.name}</div>
                    <div className="text-[10px] text-muted-foreground">
                        {item.entity_type} · {item.source}
                    </div>
                </div>
                {typeof item.score === "number" && (
                    <span className="text-[11px] font-mono text-purple-600 dark:text-purple-300">
                        {item.score.toFixed(2)}
                    </span>
                )}
            </div>
        );
    }
    if (kbId === "symptoms") {
        const icd: string[] = Array.isArray(item.icd_codes) ? item.icd_codes : [];
        const matched: string[] = Array.isArray(item.matched_symptoms) ? item.matched_symptoms : [];
        return (
            <div className="space-y-1">
                <div className="flex items-baseline gap-2">
                    <div className="text-sm font-medium truncate flex-1">{item.name}</div>
                    {typeof item.match_count === "number" && (
                        <span className="text-[11px] font-mono text-orange-600 dark:text-orange-300">
                            {item.match_count} matches
                        </span>
                    )}
                </div>
                {matched.length > 0 && (
                    <div className="text-[10px] text-muted-foreground truncate">
                        from: {matched.join(", ")}
                    </div>
                )}
                {icd.length > 0 && (
                    <div className="flex gap-1 flex-wrap">
                        {icd.map((c, i) => (
                            <code
                                key={i}
                                className="text-[10px] font-mono font-bold px-1 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
                            >
                                {c}
                            </code>
                        ))}
                    </div>
                )}
            </div>
        );
    }
    if (kbId === "snomed-icd10cm") {
        return (
            <div className="grid grid-cols-[6rem_1fr_5rem] gap-3 items-start">
                <code className="text-xs font-mono font-bold text-blue-700 dark:text-blue-300">
                    {item.icd10cm_code || "—"}
                </code>
                <div className="min-w-0">
                    <div className="text-sm truncate">{item.term || item.concept_id}</div>
                    {item.map_advice && (
                        <div className="text-[10px] text-muted-foreground truncate">{item.map_advice}</div>
                    )}
                </div>
                {item.needs_review && (
                    <span className="text-[10px] uppercase tracking-wide text-amber-600 dark:text-amber-400">
                        review
                    </span>
                )}
            </div>
        );
    }
    return (
        <div className="text-xs font-mono text-muted-foreground truncate">
            {JSON.stringify(item)}
        </div>
    );
}

function KbBadge({ kb_id }: { kb_id: string }) {
    const palette: Record<string, string> = {
        "icd10-tm": "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
        tpc: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-300",
        loinc: "bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300",
        tmt: "bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300",
        tmlt: "bg-rose-100 text-rose-700 dark:bg-rose-900/40 dark:text-rose-300",
        primekg: "bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300",
        symptoms: "bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300",
        "snomed-icd10cm": "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/40 dark:text-cyan-300",
    };
    const cls = palette[kb_id] || "bg-zinc-100 text-zinc-700";
    return (
        <span className={`px-1.5 py-0.5 rounded text-[10px] font-mono font-bold uppercase ${cls}`}>
            {kb_id}
        </span>
    );
}

export default function UnifiedKnowledgeSearchPage() {
    const [q, setQ] = useState("");
    const [k, setK] = useState(3);
    const [data, setData] = useState<SearchResponse | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const reqIdRef = useRef(0);

    const runSearch = async (query: string, top: number) => {
        const trimmed = query.trim();
        if (!trimmed) {
            setData(null);
            setError(null);
            return;
        }
        const myReq = ++reqIdRef.current;
        setLoading(true);
        try {
            const url = `${API_BASE_URL}/knowledge/search?q=${encodeURIComponent(trimmed)}&k=${top}`;
            const res = await authFetch(url, { cache: "no-store" });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const body: SearchResponse = await res.json();
            if (myReq !== reqIdRef.current) return; // a newer query started
            setData(body);
            setError(null);
        } catch (e: any) {
            if (myReq !== reqIdRef.current) return;
            setError(e?.message || "Search failed");
        } finally {
            if (myReq === reqIdRef.current) setLoading(false);
        }
    };

    useEffect(() => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => runSearch(q, k), 300);
        return () => {
            if (debounceRef.current) clearTimeout(debounceRef.current);
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [q, k]);

    const totalHits = data?.results.reduce((acc, r) => acc + r.count, 0) ?? 0;
    const kbsWithHits = data?.results.filter((r) => r.count > 0).length ?? 0;

    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* ── Breadcrumb ────────────────────────────────────────── */}
            <Link
                href="/knowledge/shared"
                className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
                <ArrowLeft className="w-3.5 h-3.5" />
                Back to Shared Knowledge
            </Link>

            {/* ── Header ────────────────────────────────────────────── */}
            <div>
                <h1 className="text-2xl font-bold flex items-center gap-2">
                    <Sparkles className="w-6 h-6 text-purple-600" />
                    Cross-KB Search
                </h1>
                <p className="text-muted-foreground text-sm mt-1">
                    One query, every shared KB. Each runs its native lookup in parallel —
                    cascade/FULLTEXT/LIKE for SQL stores, BGE-M3 semantic for PrimeKG.
                </p>
            </div>

            {/* ── Search bar ────────────────────────────────────────── */}
            <Card>
                <CardContent className="py-4 flex items-center gap-3">
                    <Search className="w-5 h-5 text-muted-foreground shrink-0" />
                    <Input
                        autoFocus
                        placeholder="e.g. metformin, E11, glucose, paracetamol…"
                        value={q}
                        onChange={(e) => setQ(e.target.value)}
                        className="flex-1"
                    />
                    <label className="text-xs text-muted-foreground flex items-center gap-1">
                        top-k
                        <select
                            value={k}
                            onChange={(e) => setK(Number(e.target.value))}
                            className="bg-transparent border rounded px-1.5 py-1 text-sm"
                        >
                            <option value={3}>3</option>
                            <option value={5}>5</option>
                            <option value={10}>10</option>
                        </select>
                    </label>
                </CardContent>
            </Card>

            {/* ── Summary strip ─────────────────────────────────────── */}
            {data && !error && (
                <div className="flex items-center gap-4 text-xs text-muted-foreground">
                    <span className="inline-flex items-center gap-1">
                        <Zap className="w-3 h-3" /> {data.total_ms}ms total
                    </span>
                    <span>·</span>
                    <span>
                        <strong className="text-foreground">{totalHits}</strong> hits across{" "}
                        <strong className="text-foreground">{kbsWithHits}</strong>/{data.results.length} KBs
                    </span>
                    {loading && (
                        <span className="inline-flex items-center gap-1">
                            <Loader2 className="w-3 h-3 animate-spin" /> refreshing…
                        </span>
                    )}
                </div>
            )}

            {/* ── Error ─────────────────────────────────────────────── */}
            {error && (
                <Card className="border-red-300 bg-red-50 dark:bg-red-900/20">
                    <CardContent className="py-4 flex items-center gap-3">
                        <AlertCircle className="w-5 h-5 text-red-600" />
                        <span className="text-sm">Search failed: {error}</span>
                    </CardContent>
                </Card>
            )}

            {/* ── Initial empty state ───────────────────────────────── */}
            {!data && !error && !loading && q.trim().length === 0 && (
                <Card className="border-dashed">
                    <CardContent className="py-10 text-center text-sm text-muted-foreground">
                        Type a code, drug name, lab, or concept to search every shared KB at once.
                    </CardContent>
                </Card>
            )}

            {/* ── Loading shimmer ───────────────────────────────────── */}
            {!data && loading && (
                <div className="flex items-center justify-center py-16 text-muted-foreground">
                    <Loader2 className="w-5 h-5 animate-spin mr-2" />
                    Searching…
                </div>
            )}

            {/* ── Grouped per-KB results ────────────────────────────── */}
            {data && (
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                    {data.results.map((kb) => (
                        <Card key={kb.kb_id} className={kb.count === 0 ? "opacity-60" : ""}>
                            <CardHeader className="pb-2">
                                <div className="flex items-center justify-between gap-2">
                                    <div className="flex items-center gap-2 min-w-0">
                                        <KbBadge kb_id={kb.kb_id} />
                                        <CardTitle className="text-sm truncate">
                                            {kb.kb_name}
                                        </CardTitle>
                                    </div>
                                    <div className="text-[11px] text-muted-foreground shrink-0">
                                        {kb.count} · {kb.latency_ms}ms
                                    </div>
                                </div>
                                <CardDescription className="text-xs">
                                    <Link
                                        href={`/knowledge/shared/${kb.kb_id}?q=${encodeURIComponent(q)}`}
                                        className="text-blue-600 dark:text-blue-400 hover:underline"
                                    >
                                        Open in browser →
                                    </Link>
                                </CardDescription>
                            </CardHeader>
                            <CardContent className="pt-0">
                                {kb.count === 0 ? (
                                    <div className="text-xs italic text-muted-foreground py-1">
                                        no matches
                                    </div>
                                ) : (
                                    <ul className="divide-y divide-border">
                                        {kb.items.map((it, i) => (
                                            <li key={i} className="py-2">
                                                <ItemRow kbId={kb.kb_id} item={it} />
                                            </li>
                                        ))}
                                    </ul>
                                )}
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}
        </div>
    );
}
