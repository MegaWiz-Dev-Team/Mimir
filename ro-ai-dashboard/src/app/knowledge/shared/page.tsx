"use client";

import { useEffect, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "@/components/ui/card";
import {
    Boxes,
    Database,
    Network,
    AlertCircle,
    CheckCircle2,
    HelpCircle,
    Loader2,
    ExternalLink,
} from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";

type SharedKb = {
    id: string;
    name: string;
    description: string;
    kind: "ontology" | "graph_ontology" | "terminology" | string;
    stores: string[];
    counts: Record<string, number>;
    source: string;
    source_version: string | null;
    status: "active" | "pending_data" | "degraded" | string;
    notes: string | null;
};

function StatusBadge({ status }: { status: string }) {
    if (status === "active") {
        return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300">
                <CheckCircle2 className="w-3 h-3" /> Active
            </span>
        );
    }
    if (status === "degraded") {
        return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
                <AlertCircle className="w-3 h-3" /> Degraded
            </span>
        );
    }
    return (
        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
            <HelpCircle className="w-3 h-3" /> Pending data
        </span>
    );
}

function KindIcon({ kind }: { kind: string }) {
    if (kind === "graph_ontology") return <Network className="w-5 h-5 text-purple-600" />;
    if (kind === "ontology") return <Database className="w-5 h-5 text-blue-600" />;
    return <Boxes className="w-5 h-5 text-zinc-600" />;
}

function formatCount(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return n.toString();
}

export default function SharedKnowledgePage() {
    const [kbs, setKbs] = useState<SharedKb[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        (async () => {
            try {
                setLoading(true);
                const res = await authFetch(`${API_BASE_URL}/knowledge/shared`, {
                    cache: "no-store",
                });
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const data = await res.json();
                if (!cancelled) {
                    setKbs(data.items || []);
                    setError(null);
                }
            } catch (e: any) {
                if (!cancelled) setError(e?.message || "Failed to load shared knowledge");
            } finally {
                if (!cancelled) setLoading(false);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, []);

    return (
        <div className="container mx-auto p-8 space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold flex items-center gap-2">
                        <Boxes className="w-6 h-6 text-purple-600" />
                        Shared Knowledge Bases
                    </h1>
                    <p className="text-muted-foreground text-sm mt-1">
                        Universal reference data — not per-tenant. ICD-10, PrimeKG, LOINC etc. are
                        shared across every tenant on this Mac mini deployment.
                    </p>
                </div>
                <div className="text-sm text-muted-foreground">
                    {loading ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                        <span>{kbs.length} KB{kbs.length === 1 ? "" : "s"}</span>
                    )}
                </div>
            </div>

            {error && (
                <Card className="border-red-300 bg-red-50 dark:bg-red-900/20">
                    <CardContent className="py-4 flex items-center gap-3">
                        <AlertCircle className="w-5 h-5 text-red-600" />
                        <span className="text-sm">Failed to load: {error}</span>
                    </CardContent>
                </Card>
            )}

            {loading ? (
                <div className="flex items-center justify-center py-16 text-muted-foreground">
                    <Loader2 className="w-5 h-5 animate-spin mr-2" />
                    Loading shared knowledge bases...
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    {kbs.map((kb) => (
                        <Card key={kb.id} className="overflow-hidden">
                            <CardHeader className="pb-3">
                                <div className="flex items-start justify-between gap-3">
                                    <div className="flex items-center gap-2">
                                        <KindIcon kind={kb.kind} />
                                        <CardTitle className="text-base">{kb.name}</CardTitle>
                                    </div>
                                    <StatusBadge status={kb.status} />
                                </div>
                                <CardDescription className="text-xs">{kb.description}</CardDescription>
                            </CardHeader>
                            <CardContent className="space-y-3">
                                {/* Stores + version row */}
                                <div className="flex items-center gap-2 flex-wrap text-xs">
                                    {kb.stores.length === 0 ? (
                                        <span className="text-muted-foreground italic">no stores yet</span>
                                    ) : (
                                        kb.stores.map((s) => (
                                            <span
                                                key={s}
                                                className="px-2 py-0.5 rounded bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 font-mono uppercase tracking-wide"
                                            >
                                                {s}
                                            </span>
                                        ))
                                    )}
                                    {kb.source_version && (
                                        <span className="text-muted-foreground">
                                            · v{kb.source_version}
                                        </span>
                                    )}
                                </div>

                                {/* Counts grid */}
                                {Object.keys(kb.counts).length > 0 && (
                                    <div className="grid grid-cols-3 gap-2 text-sm">
                                        {Object.entries(kb.counts).map(([k, v]) => (
                                            <div key={k} className="rounded-md bg-muted/40 p-2">
                                                <div className="text-xs text-muted-foreground truncate">
                                                    {k.replace(/_/g, " ")}
                                                </div>
                                                <div className="font-mono font-semibold">
                                                    {formatCount(v)}
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                )}

                                {/* Notes */}
                                {kb.notes && (
                                    <p className="text-xs text-muted-foreground italic border-l-2 border-zinc-300 dark:border-zinc-700 pl-2">
                                        {kb.notes}
                                    </p>
                                )}

                                {/* Source link */}
                                {kb.source && (
                                    <div className="flex items-center gap-1 text-xs text-muted-foreground">
                                        <ExternalLink className="w-3 h-3" />
                                        <span className="truncate">{kb.source}</span>
                                    </div>
                                )}
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            <Card className="bg-zinc-50 dark:bg-zinc-900/60 border-dashed">
                <CardContent className="py-4 text-xs text-muted-foreground">
                    <p>
                        <strong>Why "shared"?</strong> These knowledge bases are universal reference
                        data — ICD-10 codes, biomedical knowledge graphs, LOINC observation codes —
                        and they apply across every tenant on this Mac mini deployment. Per-tenant
                        ingest (uploaded PDFs, FAQs, etc.) lives under Sources / Knowledge with a
                        specific <code>tenant_id</code>; shared KBs have <code>tenant_id=NULL</code>.
                    </p>
                </CardContent>
            </Card>
        </div>
    );
}
