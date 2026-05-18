"use client";

import { useEffect, useMemo, useState } from "react";
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
    Calendar,
    Globe,
    Languages,
    ShieldCheck,
    FileText,
    Building2,
    RefreshCw,
    Tag,
} from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";

type SharedKb = {
    // meta
    id: string;
    name: string;
    description: string;
    kind: "ontology" | "graph_ontology" | "terminology" | string;
    stores: string[];
    source_url: string;
    maintainer: string;
    region: string;
    languages: string[];
    vintage_year: number | null;
    license: string;
    fhir_binding: string | null;
    update_cadence: string;
    schema_version: string;
    notes: string | null;
    // live
    counts: Record<string, number>;
    source_version: string | null;
    status: "active" | "active_fallback" | "degraded" | "pending_data" | string;
    last_local_refresh: string | null;
};

function StatusBadge({ status }: { status: string }) {
    if (status === "active") {
        return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300">
                <CheckCircle2 className="w-3 h-3" /> Active
            </span>
        );
    }
    if (status === "active_fallback") {
        return (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
                <AlertCircle className="w-3 h-3" /> Fallback
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
    return <Boxes className="w-5 h-5 text-emerald-600" />;
}

function RegionBadge({ region }: { region: string }) {
    const color =
        region === "TH"
            ? "bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300"
            : region === "INTL"
              ? "bg-sky-100 text-sky-700 dark:bg-sky-900/40 dark:text-sky-300"
              : "bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300";
    return (
        <span className={`px-1.5 py-0.5 rounded text-[10px] font-mono font-bold ${color}`}>
            {region}
        </span>
    );
}

function formatCount(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return n.toString();
}

function formatRefresh(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const diffMs = Date.now() - d.getTime();
    const diffH = Math.floor(diffMs / 3_600_000);
    if (diffH < 24) return `${diffH}h ago`;
    const diffD = Math.floor(diffH / 24);
    if (diffD < 30) return `${diffD}d ago`;
    return d.toLocaleDateString();
}

function MetaRow({
    icon: Icon,
    label,
    value,
}: {
    icon: React.ComponentType<{ className?: string }>;
    label: string;
    value: React.ReactNode;
}) {
    return (
        <div className="flex items-start gap-2 text-xs">
            <Icon className="w-3.5 h-3.5 text-muted-foreground mt-0.5 shrink-0" />
            <div className="flex-1 min-w-0">
                <div className="text-muted-foreground">{label}</div>
                <div className="font-medium text-foreground truncate">{value}</div>
            </div>
        </div>
    );
}

export default function SharedKnowledgePage() {
    const [kbs, setKbs] = useState<SharedKb[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [reloading, setReloading] = useState(false);

    const loadKbs = async () => {
        try {
            setReloading(true);
            const res = await authFetch(`${API_BASE_URL}/knowledge/shared`, {
                cache: "no-store",
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();
            setKbs(data.items || []);
            setError(null);
        } catch (e: any) {
            setError(e?.message || "Failed to load shared knowledge");
        } finally {
            setLoading(false);
            setReloading(false);
        }
    };

    useEffect(() => {
        loadKbs();
    }, []);

    const summary = useMemo(() => {
        const active = kbs.filter((k) => k.status === "active").length;
        const fallback = kbs.filter((k) => k.status === "active_fallback").length;
        const degraded = kbs.filter((k) => k.status === "degraded").length;
        const pending = kbs.filter((k) => k.status === "pending_data").length;
        const totalRecords = kbs.reduce(
            (acc, k) => acc + Object.values(k.counts).reduce((a, b) => a + b, 0),
            0,
        );
        return { active, fallback, degraded, pending, totalRecords };
    }, [kbs]);

    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* ── Header ────────────────────────────────────────────── */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold flex items-center gap-2">
                        <Boxes className="w-6 h-6 text-purple-600" />
                        Shared Knowledge Bases
                    </h1>
                    <p className="text-muted-foreground text-sm mt-1">
                        Universal reference data, shared across every tenant on this deployment.
                        Updated by Asgard operators, never by tenant users.
                    </p>
                </div>
                <button
                    onClick={loadKbs}
                    disabled={reloading}
                    className="px-3 py-1.5 text-sm rounded-md border bg-card hover:bg-accent transition-colors flex items-center gap-1.5 disabled:opacity-50"
                >
                    <RefreshCw className={`w-3.5 h-3.5 ${reloading ? "animate-spin" : ""}`} />
                    Refresh
                </button>
            </div>

            {/* ── Summary strip ─────────────────────────────────────── */}
            {!loading && kbs.length > 0 && (
                <Card>
                    <CardContent className="py-4 grid grid-cols-2 md:grid-cols-5 gap-4">
                        <div>
                            <div className="text-xs text-muted-foreground">Total KBs</div>
                            <div className="text-2xl font-bold">{kbs.length}</div>
                        </div>
                        <div>
                            <div className="text-xs text-muted-foreground">Active</div>
                            <div className="text-2xl font-bold text-green-600 dark:text-green-400">
                                {summary.active}
                            </div>
                        </div>
                        <div>
                            <div className="text-xs text-muted-foreground">Fallback</div>
                            <div className="text-2xl font-bold text-amber-600 dark:text-amber-400">
                                {summary.fallback}
                            </div>
                        </div>
                        <div>
                            <div className="text-xs text-muted-foreground">Pending</div>
                            <div className="text-2xl font-bold text-zinc-500">{summary.pending}</div>
                        </div>
                        <div>
                            <div className="text-xs text-muted-foreground">Total records</div>
                            <div className="text-2xl font-bold">
                                {formatCount(summary.totalRecords)}
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* ── Error ─────────────────────────────────────────────── */}
            {error && (
                <Card className="border-red-300 bg-red-50 dark:bg-red-900/20">
                    <CardContent className="py-4 flex items-center gap-3">
                        <AlertCircle className="w-5 h-5 text-red-600" />
                        <span className="text-sm">Failed to load: {error}</span>
                    </CardContent>
                </Card>
            )}

            {/* ── KB cards ──────────────────────────────────────────── */}
            {loading ? (
                <div className="flex items-center justify-center py-16 text-muted-foreground">
                    <Loader2 className="w-5 h-5 animate-spin mr-2" />
                    Loading shared knowledge bases...
                </div>
            ) : (
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                    {kbs.map((kb) => (
                        <Card key={kb.id} className="overflow-hidden">
                            <CardHeader className="pb-3 space-y-2">
                                <div className="flex items-start justify-between gap-3">
                                    <div className="flex items-start gap-2 min-w-0">
                                        <KindIcon kind={kb.kind} />
                                        <div className="min-w-0">
                                            <CardTitle className="text-base flex items-center gap-2 flex-wrap">
                                                {kb.name}
                                                <RegionBadge region={kb.region} />
                                            </CardTitle>
                                            <CardDescription className="text-xs mt-1">
                                                {kb.description}
                                            </CardDescription>
                                        </div>
                                    </div>
                                    <StatusBadge status={kb.status} />
                                </div>
                            </CardHeader>

                            <CardContent className="space-y-4">
                                {/* Counts grid */}
                                {Object.keys(kb.counts).length > 0 && (
                                    <div className="grid grid-cols-3 gap-2">
                                        {Object.entries(kb.counts).map(([k, v]) => (
                                            <div
                                                key={k}
                                                className="rounded-md bg-muted/40 p-2"
                                            >
                                                <div className="text-[10px] uppercase tracking-wide text-muted-foreground truncate">
                                                    {k.replace(/_/g, " ")}
                                                </div>
                                                <div className="font-mono font-bold text-base">
                                                    {formatCount(v)}
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                )}

                                {/* Metadata grid (2 col on md+) */}
                                <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-3 border-t pt-3">
                                    <MetaRow
                                        icon={Building2}
                                        label="Maintainer"
                                        value={kb.maintainer}
                                    />
                                    <MetaRow
                                        icon={Calendar}
                                        label="Vintage"
                                        value={
                                            kb.vintage_year
                                                ? `${kb.vintage_year}${
                                                      kb.source_version
                                                          ? ` (${kb.source_version})`
                                                          : ""
                                                  }`
                                                : kb.source_version || "—"
                                        }
                                    />
                                    <MetaRow
                                        icon={Languages}
                                        label="Languages"
                                        value={
                                            <span className="flex gap-1">
                                                {kb.languages.map((l) => (
                                                    <span
                                                        key={l}
                                                        className="px-1.5 py-0.5 rounded bg-zinc-100 dark:bg-zinc-800 text-[10px] font-mono uppercase"
                                                    >
                                                        {l}
                                                    </span>
                                                ))}
                                            </span>
                                        }
                                    />
                                    <MetaRow
                                        icon={ShieldCheck}
                                        label="License"
                                        value={kb.license}
                                    />
                                    {kb.fhir_binding && (
                                        <MetaRow
                                            icon={Tag}
                                            label="FHIR binding"
                                            value={
                                                <code className="text-[11px]">
                                                    {kb.fhir_binding}
                                                </code>
                                            }
                                        />
                                    )}
                                    <MetaRow
                                        icon={RefreshCw}
                                        label="Update cadence"
                                        value={kb.update_cadence}
                                    />
                                    <MetaRow
                                        icon={Calendar}
                                        label="Last local refresh"
                                        value={formatRefresh(kb.last_local_refresh)}
                                    />
                                    <MetaRow
                                        icon={Database}
                                        label="Stores"
                                        value={
                                            <span className="flex gap-1 flex-wrap">
                                                {kb.stores.length === 0 ? (
                                                    <span className="text-muted-foreground italic">
                                                        none
                                                    </span>
                                                ) : (
                                                    kb.stores.map((s) => (
                                                        <span
                                                            key={s}
                                                            className="px-1.5 py-0.5 rounded bg-zinc-100 dark:bg-zinc-800 text-[10px] font-mono uppercase"
                                                        >
                                                            {s}
                                                        </span>
                                                    ))
                                                )}
                                                <span className="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 text-[10px] font-mono">
                                                    {kb.schema_version}
                                                </span>
                                            </span>
                                        }
                                    />
                                </div>

                                {/* Notes */}
                                {kb.notes && (
                                    <div className="flex items-start gap-2 text-xs text-muted-foreground italic border-l-2 border-zinc-300 dark:border-zinc-700 pl-2">
                                        <FileText className="w-3 h-3 mt-0.5 shrink-0" />
                                        <span>{kb.notes}</span>
                                    </div>
                                )}

                                {/* Source link */}
                                {kb.source_url && (
                                    <a
                                        href={kb.source_url}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="inline-flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400 hover:underline"
                                    >
                                        <Globe className="w-3 h-3" />
                                        <span className="truncate">Source</span>
                                        <ExternalLink className="w-3 h-3" />
                                    </a>
                                )}
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            {/* ── Footnote ──────────────────────────────────────────── */}
            <Card className="bg-zinc-50 dark:bg-zinc-900/60 border-dashed">
                <CardContent className="py-4 text-xs text-muted-foreground space-y-1">
                    <p>
                        <strong>What this page shows:</strong> the universal master/reference
                        knowledge bases — ICD codes, biomedical graphs, lab/drug terminologies —
                        loaded into this deployment with{" "}
                        <code className="text-[11px]">tenant_id=NULL</code>. Per-tenant ingest
                        (uploaded PDFs, FAQs) lives under{" "}
                        <a href="/sources" className="underline">
                            Sources
                        </a>{" "}
                        and{" "}
                        <a href="/knowledge" className="underline">
                            Knowledge
                        </a>{" "}
                        with a specific tenant scope.
                    </p>
                    <p>
                        Status legend: <strong className="text-green-600">active</strong> =
                        populated and queryable ·{" "}
                        <strong className="text-amber-600">active_fallback</strong> = serving
                        traffic but with known coverage gaps (typically waiting on official
                        licensed data) ·{" "}
                        <strong className="text-amber-600">degraded</strong> = one or more stores
                        unreachable · <strong className="text-zinc-500">pending_data</strong> =
                        schema ready, data not loaded.
                    </p>
                </CardContent>
            </Card>
        </div>
    );
}
