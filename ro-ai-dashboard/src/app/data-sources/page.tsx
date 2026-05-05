"use client";

/**
 * Data Sources Registry — Wave 4
 *
 * Lists ALL data sources powering Eir's knowledge:
 *   - External KBs (PrimeKG, PubMed) — global, shared across tenants
 *   - Curated corpora (Clinical Wisdom KB)
 *   - Benchmark datasets (HealthBench Professional)
 *   - Tenant-uploaded documents (markdown, PDFs)
 *
 * Shows: source type, last sync, schedule, ingestion script, sync button.
 */

import { useEffect, useState } from "react";
import Link from "next/link";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ArrowLeft, RefreshCw, Database, FileText, BookOpen, Beaker, Globe, Loader2, ExternalLink } from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";

interface DataSource {
    id: number;
    tenant_id: string;
    name: string;
    source_type: string;
    config_json: any;
    schedule: string | null;
    last_sync_status: string | null;
    last_sync_at: string | null;
    total_chunks: number | null;
    refresh_interval_hours: number | null;
    last_refreshed_at: string | null;
}

const TYPE_ICONS: Record<string, any> = {
    external_kg: Database,
    external_corpus: Globe,
    curated_corpus: BookOpen,
    benchmark_dataset: Beaker,
    file: FileText,
    document: FileText,
    tabular: FileText,
};

const TYPE_COLORS: Record<string, string> = {
    external_kg: "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
    external_corpus: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
    curated_corpus: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
    benchmark_dataset: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
    file: "bg-gray-100 text-gray-700 dark:bg-zinc-800 dark:text-zinc-300",
    document: "bg-gray-100 text-gray-700 dark:bg-zinc-800 dark:text-zinc-300",
};

const STATUS_COLORS: Record<string, string> = {
    COMPLETED: "bg-emerald-100 text-emerald-700",
    RUNNING: "bg-blue-100 text-blue-700 animate-pulse",
    FAILED: "bg-red-100 text-red-700",
    PENDING: "bg-gray-100 text-gray-600",
};

export default function DataSourcesPage() {
    const [sources, setSources] = useState<DataSource[]>([]);
    const [loading, setLoading] = useState(true);
    const [syncing, setSyncing] = useState<Record<number, boolean>>({});
    const [syncMessages, setSyncMessages] = useState<Record<number, string>>({});

    const load = async () => {
        setLoading(true);
        try {
            const res = await authFetch(`${API_BASE_URL}/sources`, { cache: "no-store" });
            if (res.ok) setSources(await res.json());
        } catch (e) {
            console.error("Failed to load sources", e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => { load(); }, []);

    const handleSync = async (id: number) => {
        setSyncing(p => ({ ...p, [id]: true }));
        setSyncMessages(p => ({ ...p, [id]: "" }));
        try {
            const res = await authFetch(`${API_BASE_URL}/sources/${id}/sync`, { method: "POST" });
            const body = await res.json();
            setSyncMessages(p => ({ ...p, [id]: body.message || (body.error ? `Error: ${body.error}` : "Triggered") }));
            // Refresh list after a moment
            setTimeout(() => load(), 2000);
        } catch (e: any) {
            setSyncMessages(p => ({ ...p, [id]: `Error: ${e.message || e}` }));
        } finally {
            setSyncing(p => ({ ...p, [id]: false }));
        }
    };

    const fmtDate = (iso: string | null) => iso
        ? new Date(iso).toLocaleString("sv-SE", { hour12: false }).slice(0, 16).replace(" ", " · ")
        : "—";

    const globalSources = sources.filter(s => s.tenant_id === "__global__");
    const tenantSources = sources.filter(s => s.tenant_id !== "__global__");

    return (
        <div className="container mx-auto p-8 max-w-7xl">
            <div className="flex justify-between items-end mb-8">
                <div>
                    <Button asChild variant="ghost" size="sm" className="mb-1">
                        <Link href="/"><ArrowLeft className="mr-1 h-4 w-4" /> Back</Link>
                    </Button>
                    <h1 className="text-3xl font-bold tracking-tight">Data Sources</h1>
                    <p className="text-muted-foreground">All knowledge bases powering Eir — external KBs, curated corpora, benchmarks, and tenant uploads</p>
                </div>
                <Button variant="outline" size="sm" onClick={load} disabled={loading}>
                    <RefreshCw className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`} /> Refresh
                </Button>
            </div>

            {globalSources.length > 0 && (
                <div className="mb-8">
                    <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase tracking-wide">🌐 Global Sources (shared across tenants)</h2>
                    <div className="grid gap-3 md:grid-cols-2">
                        {globalSources.map(s => <SourceCard key={s.id} src={s} syncing={!!syncing[s.id]} message={syncMessages[s.id]} onSync={handleSync} fmtDate={fmtDate} />)}
                    </div>
                </div>
            )}

            {tenantSources.length > 0 && (
                <div>
                    <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase tracking-wide">🏠 Tenant Sources (this tenant only)</h2>
                    <div className="grid gap-3 md:grid-cols-2">
                        {tenantSources.map(s => <SourceCard key={s.id} src={s} syncing={!!syncing[s.id]} message={syncMessages[s.id]} onSync={handleSync} fmtDate={fmtDate} />)}
                    </div>
                </div>
            )}

            {!loading && sources.length === 0 && (
                <Card><CardContent className="p-8 text-center text-muted-foreground">
                    No data sources registered. <Link href="/sources" className="underline text-violet-600">Upload a document</Link>?
                </CardContent></Card>
            )}
        </div>
    );
}

function SourceCard({ src, syncing, message, onSync, fmtDate }: {
    src: DataSource; syncing: boolean; message: string | undefined;
    onSync: (id: number) => void; fmtDate: (iso: string | null) => string;
}) {
    const Icon = TYPE_ICONS[src.source_type] || Database;
    const cfg = typeof src.config_json === "string" ? safeJson(src.config_json) : src.config_json;
    const origin = cfg?.origin || cfg?.url || cfg?.original_filename || "—";
    const license = cfg?.license;
    const version = cfg?.version;
    const ingestionScript = cfg?.ingestion_script;
    const storageBackends = Array.isArray(cfg?.storage)
        ? cfg.storage.map((s: any) => s.backend + (s.collection ? `:${s.collection}` : "") + (s.label ? `:${s.label}` : "")).join(", ")
        : null;

    return (
        <Card>
            <CardHeader className="pb-3">
                <div className="flex items-start justify-between gap-3">
                    <div className="flex items-start gap-3">
                        <div className={`w-9 h-9 rounded-lg flex items-center justify-center ${TYPE_COLORS[src.source_type] || "bg-gray-100"}`}>
                            <Icon className="w-5 h-5" />
                        </div>
                        <div>
                            <CardTitle className="text-base flex items-center gap-2">
                                {src.name}
                                {version && <span className="text-[10px] font-mono bg-gray-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">{version}</span>}
                            </CardTitle>
                            <p className="text-xs text-muted-foreground mt-0.5">{src.source_type} · {src.total_chunks?.toLocaleString() ?? "—"} items</p>
                        </div>
                    </div>
                    {src.last_sync_status && (
                        <span className={`text-[10px] font-medium px-2 py-0.5 rounded ${STATUS_COLORS[src.last_sync_status] || "bg-gray-100"}`}>
                            {src.last_sync_status}
                        </span>
                    )}
                </div>
            </CardHeader>
            <CardContent className="space-y-2 text-xs">
                <div><span className="text-muted-foreground">Origin:</span> <code className="text-[11px]">{origin}</code></div>
                {license && <div><span className="text-muted-foreground">License:</span> {license}</div>}
                {storageBackends && <div><span className="text-muted-foreground">Storage:</span> <code className="text-[11px]">{storageBackends}</code></div>}
                <div><span className="text-muted-foreground">Last sync:</span> {fmtDate(src.last_sync_at)} · schedule: <code>{src.schedule || "manual"}</code></div>
                {ingestionScript && (
                    <div><span className="text-muted-foreground">Script:</span> <code className="text-[10px] bg-gray-100 dark:bg-zinc-800 px-1 py-0.5 rounded">{ingestionScript}</code></div>
                )}
                {message && (
                    <div className={`p-2 rounded text-[11px] ${message.startsWith("Error") ? "bg-red-50 text-red-700 dark:bg-red-900/20" : "bg-blue-50 text-blue-700 dark:bg-blue-900/20"}`}>
                        {message}
                    </div>
                )}
                <div className="flex gap-2 pt-1">
                    <Button size="sm" variant="outline" onClick={() => onSync(src.id)} disabled={syncing}>
                        {syncing ? <Loader2 className="w-3 h-3 mr-2 animate-spin" /> : <RefreshCw className="w-3 h-3 mr-2" />}
                        Sync now
                    </Button>
                    {ingestionScript && (
                        <details className="inline-block">
                            <summary className="cursor-pointer text-violet-600 hover:underline px-2 py-1 text-[11px]">CLI command</summary>
                            <pre className="mt-1 bg-gray-50 dark:bg-zinc-800 p-2 rounded text-[10px] whitespace-pre-wrap break-all">python3 {ingestionScript}</pre>
                        </details>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}

function safeJson(s: string): any {
    try { return JSON.parse(s); } catch { return {}; }
}
