"use client";

// Sprint 50 B-50i — Syn OCR audit + policy dashboard.
//
// Surfaces the four /api/v1/syn/ocr/* endpoints:
//   - GET /health     — 4-tier engine status board
//   - GET /policy     — current tenant cloud opt-in flags + budget
//   - GET /documents  — paginated audit history (engine, status, cost, latency)
//
// Read-only Day-1: editing tenant opt-in flags + budget cap is admin-only and
// arrives in B-50l. This page is the visibility layer — clinicians, ops, and
// curators all use it to inspect what's been OCR'd, by which engine, and how
// much it cost.

import { useEffect, useState } from "react";
import { authFetch, SYN_API_BASE_URL as API_BASE_URL } from "@/lib/api";

interface EngineHealth {
    engine: string;
    tier: number;
    status: string;
    detail: string | null;
    latency_ms: number | null;
}
interface HealthResponse {
    overall: string;
    engines: EngineHealth[];
}
interface TenantPolicy {
    tenant_id: string;
    ocr_cloud_flash_enabled: boolean;
    ocr_cloud_pro_enabled: boolean;
    ocr_phi_strict: boolean;
    ocr_monthly_cloud_budget_usd: number;
    pii_mode: string;
    pii_custom_patterns: string | null;
}
interface OcrDocument {
    id: string;
    tenant_id: string;
    image_sha256: string;
    engine_used: string;
    router_reason: string | null;
    confidence: number | null;
    bbox_count: number | null;
    cost_usd: number;
    latency_ms: number | null;
    pii_redacted: boolean;
    status: string;
    status_message: string | null;
    created_at: string;
}
interface DocumentsResponse {
    tenant_id: string;
    limit: number;
    offset: number;
    rows: OcrDocument[];
}

function statusBadge(status: string): string {
    switch (status) {
        case "succeeded":
            return "bg-green-100 text-green-800";
        case "engine_failed":
            return "bg-red-100 text-red-800";
        case "pii_strict_block":
            return "bg-orange-100 text-orange-800";
        case "budget_exceeded":
            return "bg-yellow-100 text-yellow-800";
        case "pii_blocked":
            return "bg-purple-100 text-purple-800";
        default:
            return "bg-slate-100 text-slate-800";
    }
}

function engineBadge(engine: string): string {
    if (engine.startsWith("gemini")) return "bg-sky-100 text-sky-800";
    if (engine === "chandra-local") return "bg-indigo-100 text-indigo-800";
    if (engine === "paddleocr-local") return "bg-emerald-100 text-emerald-800";
    return "bg-slate-100 text-slate-800";
}

export default function SynOcrPage() {
    const [health, setHealth] = useState<HealthResponse | null>(null);
    const [policy, setPolicy] = useState<TenantPolicy | null>(null);
    const [docs, setDocs] = useState<DocumentsResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const load = async () => {
        setLoading(true);
        setError(null);
        try {
            const [h, p, d] = await Promise.all([
                authFetch(`${API_BASE_URL}/syn/ocr/health`, { cache: "no-store" }),
                authFetch(`${API_BASE_URL}/syn/ocr/policy`, { cache: "no-store" }),
                authFetch(`${API_BASE_URL}/syn/ocr/documents?limit=50`, { cache: "no-store" }),
            ]);
            if (h.ok) setHealth(await h.json());
            if (p.ok) setPolicy(await p.json());
            if (d.ok) setDocs(await d.json());
            if (!p.ok && p.status !== 404) {
                setError(`policy: ${p.status} ${p.statusText}`);
            }
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "fetch failed");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        load();
    }, []);

    const monthlySpent = (docs?.rows ?? [])
        .filter((r) => {
            const d = new Date(r.created_at);
            const now = new Date();
            return d.getUTCFullYear() === now.getUTCFullYear() && d.getUTCMonth() === now.getUTCMonth();
        })
        .reduce((acc, r) => acc + (r.cost_usd ?? 0), 0);

    return (
        <div className="p-6 space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold">Syn OCR</h1>
                    <p className="text-sm text-slate-600">
                        4-tier hybrid OCR audit + policy view. Sprint 50 (B-50i).
                    </p>
                </div>
                <div className="flex items-center gap-2">
                    <a
                        href="/syn-ocr/review"
                        className="px-3 py-1.5 text-sm bg-amber-500 text-white hover:bg-amber-600 rounded"
                    >
                        Review queue
                    </a>
                    <a
                        href="/syn-ocr/admin"
                        className="px-3 py-1.5 text-sm bg-blue-600 text-white hover:bg-blue-700 rounded"
                    >
                        Edit policy
                    </a>
                    <button
                        onClick={load}
                        className="px-3 py-1.5 text-sm bg-slate-200 hover:bg-slate-300 rounded"
                    >
                        {loading ? "…" : "Refresh"}
                    </button>
                </div>
            </div>

            {error && (
                <div className="p-3 bg-red-50 text-red-800 border border-red-200 rounded">
                    {error}
                </div>
            )}

            {/* ─── Engine health ─── */}
            <section>
                <h2 className="text-lg font-semibold mb-2">Engines</h2>
                {health ? (
                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
                        {health.engines.map((e) => (
                            <div
                                key={e.engine}
                                className="p-3 border rounded bg-white shadow-sm"
                            >
                                <div className="flex items-center justify-between mb-1">
                                    <span className={`text-xs px-2 py-0.5 rounded ${engineBadge(e.engine)}`}>
                                        Tier {e.tier}
                                    </span>
                                    <span className="text-xs text-slate-500">
                                        {e.latency_ms != null ? `${e.latency_ms}ms` : "—"}
                                    </span>
                                </div>
                                <div className="font-mono text-sm">{e.engine}</div>
                                <div className="mt-1 text-xs text-slate-700">
                                    status: <span className="font-medium">{e.status}</span>
                                </div>
                                {e.detail && (
                                    <div className="mt-0.5 text-xs text-slate-500 truncate">
                                        {e.detail}
                                    </div>
                                )}
                            </div>
                        ))}
                    </div>
                ) : (
                    <div className="text-sm text-slate-500">{loading ? "Loading…" : "no data"}</div>
                )}
            </section>

            {/* ─── Tenant policy ─── */}
            <section>
                <h2 className="text-lg font-semibold mb-2">Tenant Policy</h2>
                {policy ? (
                    <div className="p-4 border rounded bg-white shadow-sm grid grid-cols-2 sm:grid-cols-3 gap-4 text-sm">
                        <div>
                            <div className="text-xs text-slate-500">tenant_id</div>
                            <div className="font-mono">{policy.tenant_id}</div>
                        </div>
                        <div>
                            <div className="text-xs text-slate-500">PHI strict</div>
                            <div className={policy.ocr_phi_strict ? "text-orange-700 font-semibold" : "text-slate-700"}>
                                {policy.ocr_phi_strict ? "ON — cloud OCR refused" : "off"}
                            </div>
                        </div>
                        <div>
                            <div className="text-xs text-slate-500">Cloud Flash</div>
                            <div>{policy.ocr_cloud_flash_enabled ? "enabled" : "disabled"}</div>
                        </div>
                        <div>
                            <div className="text-xs text-slate-500">Cloud Pro</div>
                            <div>{policy.ocr_cloud_pro_enabled ? "enabled" : "disabled"}</div>
                        </div>
                        <div>
                            <div className="text-xs text-slate-500">Monthly budget cap</div>
                            <div>
                                {policy.ocr_monthly_cloud_budget_usd > 0
                                    ? `$${policy.ocr_monthly_cloud_budget_usd.toFixed(2)}`
                                    : "no cap"}
                            </div>
                        </div>
                        <div>
                            <div className="text-xs text-slate-500">PII mode (Skuggi)</div>
                            <div className="font-mono">{policy.pii_mode}</div>
                        </div>
                        <div className="col-span-2 sm:col-span-3 pt-2 border-t">
                            <div className="text-xs text-slate-500">Month-to-date cloud spend (visible rows only)</div>
                            <div>
                                ${monthlySpent.toFixed(5)}
                                {policy.ocr_monthly_cloud_budget_usd > 0 && (
                                    <span className="text-slate-500 ml-2">
                                        / ${policy.ocr_monthly_cloud_budget_usd.toFixed(2)} cap
                                        {monthlySpent / policy.ocr_monthly_cloud_budget_usd > 0.8 && (
                                            <span className="ml-2 text-yellow-700">
                                                ⚠ {((monthlySpent / policy.ocr_monthly_cloud_budget_usd) * 100).toFixed(0)}% used
                                            </span>
                                        )}
                                    </span>
                                )}
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="text-sm text-slate-500">
                        {loading ? "Loading…" : "no tenant_configs row for this tenant — admin must seed it"}
                    </div>
                )}
            </section>

            {/* ─── Audit log ─── */}
            <section>
                <h2 className="text-lg font-semibold mb-2">
                    Audit log <span className="text-sm text-slate-500 font-normal">(latest 50)</span>
                </h2>
                {docs && docs.rows.length > 0 ? (
                    <div className="overflow-x-auto border rounded bg-white shadow-sm">
                        <table className="min-w-full text-sm">
                            <thead className="bg-slate-50 text-xs text-slate-600">
                                <tr>
                                    <th className="text-left p-2">When</th>
                                    <th className="text-left p-2">Engine</th>
                                    <th className="text-left p-2">Reason</th>
                                    <th className="text-left p-2">Status</th>
                                    <th className="text-right p-2">Cost</th>
                                    <th className="text-right p-2">Latency</th>
                                    <th className="text-left p-2">Image SHA</th>
                                </tr>
                            </thead>
                            <tbody>
                                {docs.rows.map((r) => (
                                    <tr key={r.id} className="border-t hover:bg-slate-50">
                                        <td className="p-2 whitespace-nowrap text-slate-600">
                                            {new Date(r.created_at).toLocaleString()}
                                        </td>
                                        <td className="p-2">
                                            <span className={`px-2 py-0.5 rounded text-xs ${engineBadge(r.engine_used)}`}>
                                                {r.engine_used}
                                            </span>
                                        </td>
                                        <td className="p-2 font-mono text-xs">{r.router_reason ?? "—"}</td>
                                        <td className="p-2">
                                            <span className={`px-2 py-0.5 rounded text-xs ${statusBadge(r.status)}`}>
                                                {r.status}
                                            </span>
                                        </td>
                                        <td className="p-2 text-right font-mono">
                                            {r.cost_usd > 0 ? `$${r.cost_usd.toFixed(5)}` : "—"}
                                        </td>
                                        <td className="p-2 text-right font-mono text-slate-600">
                                            {r.latency_ms != null ? `${r.latency_ms}ms` : "—"}
                                        </td>
                                        <td className="p-2 font-mono text-xs text-slate-500">
                                            {r.image_sha256.slice(0, 12)}…
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                ) : (
                    <div className="text-sm text-slate-500">
                        {loading ? "Loading…" : "no audit rows yet"}
                    </div>
                )}
            </section>
        </div>
    );
}
