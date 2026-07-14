"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Shield, ShieldAlert, ShieldCheck, ShieldX, Loader2, AlertCircle, ExternalLink, Save, History, Filter } from "lucide-react";

import {
    getSkuggiPolicy,
    saveSkuggiPolicy,
    getSkuggiRedactions,
    SkuggiPolicy,
    SkuggiPiiMode,
    SkuggiRedactionsResponse,
    SkuggiRedactionRow,
    SkuggiDetection,
} from "@/lib/api";

/**
 * B-50b-6 — Skuggi PII Guardrail config page.
 *
 * Lives at `/admin/skuggi` (not under /analytics/llm) because PII
 * guardrail config + future audit history + corpus runner deserve their
 * own surface. The OCR Cost Guard tab on /analytics/llm shows PII mode
 * as a read-only badge with a "Configure" link pointing here.
 */

interface ModeOption {
    value: SkuggiPiiMode;
    label: string;
    summary: string;
    detail: string;
    risk: "low" | "medium" | "high";
}

const MODE_OPTIONS: ModeOption[] = [
    {
        value: "off",
        label: "Off",
        summary: "No redaction. Raw PII forwarded to external LLMs.",
        detail: "Use only for non-PHI tenants or controlled tests. Audit rows are still written but no redaction happens.",
        risk: "high",
    },
    {
        value: "detect-only",
        label: "Detect Only",
        summary: "Run detection + audit. Forward ORIGINAL payload unchanged.",
        detail: "Useful during Skuggi rollout to measure detection coverage without changing payloads. PII still leaves to LLM — DO NOT use long-term on PHI tenants.",
        risk: "medium",
    },
    {
        value: "mask-and-send",
        label: "Mask and Send (recommended)",
        summary: "Detect + replace PII with [REDACTED_*] placeholders. Send REDACTED payload.",
        detail: "Default for PHI tenants. LLM sees only redacted text; structural form context (labels) preserved for anchored fields.",
        risk: "low",
    },
    {
        value: "block-on-pii",
        label: "Block on PII",
        summary: "Detect + audit. If any PII found, REJECT the call with 422.",
        detail: "Detection-gated no-cloud. Rejects only when PII is actually detected. Application must handle 422 responses gracefully.",
        risk: "low",
    },
    {
        value: "local-only",
        label: "Local Only",
        summary: "Categorically block ALL external providers with 403 — never send to any cloud, regardless of PII detection.",
        detail: "Strongest posture for no-cloud tenants (medical / PHI). Fail-closed: rejects before payload inspection and holds even if the policy DB is unreachable (env allow-list backstop). Requests must use a local model. Unlike block-on-pii, does not rely on the detector firing.",
        risk: "low",
    },
];

function riskColor(r: "low" | "medium" | "high") {
    switch (r) {
        case "low":    return "text-emerald-600";
        case "medium": return "text-amber-600";
        case "high":   return "text-red-600";
    }
}

function modeIcon(mode: string) {
    switch (mode) {
        case "off":            return <ShieldX className="w-5 h-5 text-red-500" />;
        case "detect-only":    return <ShieldAlert className="w-5 h-5 text-amber-500" />;
        case "mask-and-send":  return <ShieldCheck className="w-5 h-5 text-emerald-500" />;
        case "block-on-pii":   return <Shield className="w-5 h-5 text-indigo-500" />;
        case "local-only":     return <Shield className="w-5 h-5 text-teal-600" fill="currentColor" />;
        default:               return <Shield className="w-5 h-5 text-muted-foreground" />;
    }
}

type TimeRange = "1h" | "24h" | "7d" | "all";

function rangeToSince(r: TimeRange): string | undefined {
    if (r === "all") return undefined;
    const ms =
        r === "1h" ? 60 * 60_000 :
        r === "24h" ? 24 * 60 * 60_000 :
        7 * 24 * 60 * 60_000;
    return new Date(Date.now() - ms).toISOString();
}

function detectionList(raw: SkuggiRedactionRow["detections"]): SkuggiDetection[] {
    if (Array.isArray(raw)) return raw as SkuggiDetection[];
    return [];
}

function decisionBadgeVariant(decision: string, blocked: boolean): "default" | "destructive" | "outline" | "secondary" {
    if (blocked) return "destructive";
    if (decision === "passed") return "secondary";
    if (decision === "error") return "destructive";
    return "outline";
}

export default function SkuggiAdminPage() {
    const [policy, setPolicy] = useState<SkuggiPolicy | null>(null);
    const [selected, setSelected] = useState<SkuggiPiiMode | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [savedTs, setSavedTs] = useState<number | null>(null);

    // B-50b-8 audit history state
    const [redactions, setRedactions] = useState<SkuggiRedactionsResponse | null>(null);
    const [historyLoading, setHistoryLoading] = useState(false);
    const [historyError, setHistoryError] = useState<string | null>(null);
    const [range, setRange] = useState<TimeRange>("24h");
    const [blockedOnly, setBlockedOnly] = useState(false);

    const load = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const p = await getSkuggiPolicy();
            setPolicy(p);
            setSelected(p.pii_mode as SkuggiPiiMode);
        } catch (e) {
            setError(String((e as Error)?.message || e));
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => { load(); }, [load]);

    const loadHistory = useCallback(async () => {
        setHistoryLoading(true);
        setHistoryError(null);
        try {
            const r = await getSkuggiRedactions({
                limit: 100,
                since: rangeToSince(range),
                blockedOnly,
                surface: "text",
            });
            setRedactions(r);
        } catch (e) {
            setHistoryError(String((e as Error)?.message || e));
        } finally {
            setHistoryLoading(false);
        }
    }, [range, blockedOnly]);

    useEffect(() => { loadHistory(); }, [loadHistory]);

    const dirty = policy && selected && selected !== policy.pii_mode;

    const handleSave = async () => {
        if (!selected || !dirty) return;
        setSaving(true);
        setError(null);
        try {
            const updated = await saveSkuggiPolicy(selected);
            setPolicy(updated);
            setSavedTs(Date.now());
        } catch (e) {
            setError(String((e as Error)?.message || e));
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            <div className="flex items-start justify-between gap-4">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <Shield className="w-6 h-6 text-indigo-500" />
                        Skuggi PII Guardrail
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1 max-w-2xl">
                        Controls whether Heimdall redacts PII before forwarding LLM requests to external providers (Gemini, OpenAI, etc.). Tier 1 regex coverage: Thai national ID, phone, email, plus anchored form fields (Patient Name, Doctor Name, HN, License Number, ThaiID). Tier 2 PyThaiNLP NER for free-form Thai names runs conditionally.
                    </p>
                </div>
                {policy && (
                    <div className="flex flex-col items-end gap-1 shrink-0">
                        <div className="flex items-center gap-2">
                            {modeIcon(policy.pii_mode)}
                            <Badge variant="outline" className="font-mono text-xs">
                                current: {policy.pii_mode}
                            </Badge>
                        </div>
                        {!policy.pii_mode_valid && (
                            <Badge variant="destructive" className="text-[10px]">
                                ⚠ unknown mode (Heimdall falls back to mask-and-send)
                            </Badge>
                        )}
                        <div className="text-xs text-muted-foreground">tenant: <code className="font-mono">{policy.tenant_id}</code></div>
                    </div>
                )}
            </div>

            {error && (
                <Card className="border-red-300 bg-red-50/50">
                    <CardContent className="py-4 flex items-center gap-2 text-red-700 text-sm">
                        <AlertCircle className="w-4 h-4" />
                        <div>{error}</div>
                    </CardContent>
                </Card>
            )}

            {loading || !policy ? (
                <Card>
                    <CardContent className="py-12 flex items-center justify-center text-muted-foreground">
                        <Loader2 className="w-5 h-5 animate-spin mr-2" />
                        Loading policy…
                    </CardContent>
                </Card>
            ) : (
                <>
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">PII Mode</CardTitle>
                            <CardDescription>
                                Pick the redaction strategy. Changes take effect within ~60s (Heimdall&apos;s per-tenant cache TTL).
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            {MODE_OPTIONS.map((opt) => {
                                const isSelected = selected === opt.value;
                                const isCurrent = policy.pii_mode === opt.value;
                                return (
                                    <label
                                        key={opt.value}
                                        className={`flex items-start gap-3 rounded-lg border p-3 cursor-pointer transition-colors ${
                                            isSelected ? "border-indigo-500 bg-indigo-50/40" : "hover:bg-muted/30"
                                        }`}
                                    >
                                        <input
                                            type="radio"
                                            name="pii_mode"
                                            value={opt.value}
                                            checked={isSelected}
                                            onChange={() => setSelected(opt.value)}
                                            className="mt-1"
                                        />
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-2 flex-wrap">
                                                <span className="font-medium">{opt.label}</span>
                                                <Badge variant="outline" className="font-mono text-[10px]">{opt.value}</Badge>
                                                <span className={`text-[10px] uppercase font-semibold ${riskColor(opt.risk)}`}>
                                                    {opt.risk} risk
                                                </span>
                                                {isCurrent && (
                                                    <Badge variant="secondary" className="text-[10px]">current</Badge>
                                                )}
                                            </div>
                                            <div className="text-sm text-muted-foreground mt-1">{opt.summary}</div>
                                            <div className="text-xs text-muted-foreground mt-1 leading-snug">{opt.detail}</div>
                                        </div>
                                    </label>
                                );
                            })}
                        </CardContent>
                    </Card>

                    <div className="flex items-center justify-between">
                        <div className="text-xs text-muted-foreground">
                            {dirty ? (
                                <span className="text-amber-600">
                                    Pending change: <code>{policy.pii_mode}</code> → <code>{selected}</code>
                                </span>
                            ) : savedTs ? (
                                <span className="text-emerald-600">
                                    ✓ Saved {Math.round((Date.now() - savedTs) / 1000)}s ago
                                </span>
                            ) : (
                                <span>No pending changes.</span>
                            )}
                        </div>
                        <div className="flex items-center gap-2">
                            <Button variant="outline" size="sm" onClick={load} disabled={saving}>
                                Refresh
                            </Button>
                            <Button
                                size="sm"
                                onClick={handleSave}
                                disabled={!dirty || saving}
                            >
                                {saving ? (
                                    <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                                ) : (
                                    <Save className="w-4 h-4 mr-1" />
                                )}
                                Save
                            </Button>
                        </div>
                    </div>

                    <Card>
                        <CardHeader className="space-y-3">
                            <div className="flex items-start justify-between gap-2 flex-wrap">
                                <div>
                                    <CardTitle className="text-base flex items-center gap-2">
                                        <History className="w-4 h-4 text-indigo-500" />
                                        Recent Redactions
                                    </CardTitle>
                                    <CardDescription>
                                        Per-call audit from <code className="font-mono">pii_redactions</code>. Heimdall writes one row per cloud-bound LLM call (fire-and-forget) regardless of whether PII fired.
                                    </CardDescription>
                                </div>
                                <Button variant="ghost" size="sm" onClick={loadHistory} disabled={historyLoading}>
                                    {historyLoading ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : "Refresh"}
                                </Button>
                            </div>
                            <div className="flex items-center gap-2 flex-wrap">
                                <Filter className="w-3.5 h-3.5 text-muted-foreground" />
                                {(["1h", "24h", "7d", "all"] as const).map((r) => (
                                    <Button
                                        key={r}
                                        size="sm"
                                        variant={range === r ? "default" : "outline"}
                                        onClick={() => setRange(r)}
                                        className="h-7 text-xs"
                                    >
                                        {r === "1h" ? "Last hour" : r === "24h" ? "Last 24h" : r === "7d" ? "Last 7d" : "All"}
                                    </Button>
                                ))}
                                <Button
                                    size="sm"
                                    variant={blockedOnly ? "destructive" : "outline"}
                                    onClick={() => setBlockedOnly((b) => !b)}
                                    className="h-7 text-xs"
                                >
                                    Blocked only
                                </Button>
                            </div>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            {historyError && (
                                <div className="flex items-center gap-2 text-red-700 bg-red-50/50 border border-red-300 rounded px-3 py-2 text-xs">
                                    <AlertCircle className="w-3.5 h-3.5" />
                                    {historyError}
                                </div>
                            )}
                            {redactions && (
                                <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
                                    <div className="rounded border p-3">
                                        <div className="text-[10px] uppercase text-muted-foreground">Calls</div>
                                        <div className="text-xl font-semibold">{redactions.summary.total_calls}</div>
                                    </div>
                                    <div className="rounded border p-3">
                                        <div className="text-[10px] uppercase text-muted-foreground">With PII</div>
                                        <div className="text-xl font-semibold">{redactions.summary.calls_with_pii}</div>
                                    </div>
                                    <div className="rounded border p-3">
                                        <div className="text-[10px] uppercase text-muted-foreground">Blocked</div>
                                        <div className={`text-xl font-semibold ${redactions.summary.blocked_calls > 0 ? "text-red-600" : ""}`}>
                                            {redactions.summary.blocked_calls}
                                        </div>
                                    </div>
                                    <div className="rounded border p-3">
                                        <div className="text-[10px] uppercase text-muted-foreground">Avg latency</div>
                                        <div className="text-xl font-semibold">{redactions.summary.avg_latency_ms.toFixed(1)}<span className="text-xs font-normal ml-1">ms</span></div>
                                    </div>
                                    <div className="rounded border p-3">
                                        <div className="text-[10px] uppercase text-muted-foreground">Tier 2</div>
                                        <div className="text-xl font-semibold">{redactions.summary.tier2_count}<span className="text-xs font-normal ml-1">/ {redactions.summary.tier1_count + redactions.summary.tier2_count}</span></div>
                                    </div>
                                </div>
                            )}

                            {historyLoading && !redactions ? (
                                <div className="py-8 flex items-center justify-center text-muted-foreground text-sm">
                                    <Loader2 className="w-4 h-4 animate-spin mr-2" /> Loading audit history…
                                </div>
                            ) : redactions && redactions.items.length === 0 ? (
                                <div className="py-8 text-center text-sm text-muted-foreground">
                                    No redaction rows in this window. Cloud-bound LLM traffic will produce rows; if you expected some, check Heimdall logs.
                                </div>
                            ) : redactions ? (
                                <div className="overflow-x-auto">
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead className="text-xs">When</TableHead>
                                                <TableHead className="text-xs">Mode</TableHead>
                                                <TableHead className="text-xs">Decision</TableHead>
                                                <TableHead className="text-xs text-right">PII</TableHead>
                                                <TableHead className="text-xs">Categories</TableHead>
                                                <TableHead className="text-xs">Tier</TableHead>
                                                <TableHead className="text-xs">Provider · Model</TableHead>
                                                <TableHead className="text-xs text-right">Latency</TableHead>
                                                <TableHead className="text-xs">Trace</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {redactions.items.map((row) => {
                                                const ts = new Date(row.created_at);
                                                const cats = detectionList(row.detections);
                                                return (
                                                    <TableRow key={row.id}>
                                                        <TableCell className="text-xs whitespace-nowrap" title={row.created_at}>
                                                            {ts.toLocaleString()}
                                                        </TableCell>
                                                        <TableCell>
                                                            <Badge variant="outline" className="font-mono text-[10px]">{row.pii_mode_used}</Badge>
                                                        </TableCell>
                                                        <TableCell>
                                                            <Badge variant={decisionBadgeVariant(row.decision, row.blocked)} className="text-[10px]">
                                                                {row.blocked ? "blocked" : row.decision}
                                                            </Badge>
                                                        </TableCell>
                                                        <TableCell className="text-xs font-mono text-right">{row.pii_total_count}</TableCell>
                                                        <TableCell className="text-xs">
                                                            <div className="flex flex-wrap gap-1">
                                                                {cats.length === 0 ? (
                                                                    <span className="text-muted-foreground">—</span>
                                                                ) : (
                                                                    cats.map((d, i) => (
                                                                        <Badge key={`${row.id}-${i}`} variant="secondary" className="text-[10px] font-mono">
                                                                            {d.category}{d.count > 1 ? `×${d.count}` : ""}
                                                                        </Badge>
                                                                    ))
                                                                )}
                                                            </div>
                                                        </TableCell>
                                                        <TableCell className="text-xs font-mono">
                                                            {row.detection_tier || "—"}
                                                        </TableCell>
                                                        <TableCell className="text-xs">
                                                            <span className="font-mono">{row.provider || "?"}</span>
                                                            {row.model && <span className="text-muted-foreground"> · {row.model}</span>}
                                                        </TableCell>
                                                        <TableCell className="text-xs font-mono text-right">
                                                            {row.latency_ms != null ? `${row.latency_ms}ms` : "—"}
                                                        </TableCell>
                                                        <TableCell>
                                                            {row.request_id ? (
                                                                <a
                                                                    href={`https://laminar.asgard.internal/projects?q=${encodeURIComponent(row.request_id)}`}
                                                                    target="_blank"
                                                                    rel="noopener noreferrer"
                                                                    className="inline-flex items-center gap-1 text-xs text-indigo-600 hover:underline"
                                                                    title={`Open trace ${row.request_id} in Laminar`}
                                                                >
                                                                    <ExternalLink className="w-3 h-3" />
                                                                </a>
                                                            ) : (
                                                                <span className="text-muted-foreground">—</span>
                                                            )}
                                                        </TableCell>
                                                    </TableRow>
                                                );
                                            })}
                                        </TableBody>
                                    </Table>
                                </div>
                            ) : null}
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">Test corpus + leak runner</CardTitle>
                            <CardDescription>
                                Run the synthetic PII test corpus through your agents and verify no PII leaks back from external LLMs.
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-2 text-xs text-muted-foreground">
                            <p>
                                Corpus seeded for <code className="font-mono">asgard_insurance</code> tenant (30 synthetic rows: free-text + anchored + insurance form shapes + negative controls). Run end-to-end via:
                            </p>
                            <pre className="bg-muted/50 rounded p-3 font-mono text-[11px] overflow-x-auto">
{`cargo run --bin skuggi-leak-runner -- \\
  --mimir-url https://mimir.asgard.internal \\
  --tenant-id asgard_insurance \\
  --agent-id <AGENT_ID> \\
  --concurrency 4`}
                            </pre>
                            <p>
                                Exit 0 = clean. Exit 1 = leak detected — investigate before promoting.
                            </p>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">Trace deep-dive</CardTitle>
                            <CardDescription>Per-call redaction history in Laminar (Sága).</CardDescription>
                        </CardHeader>
                        <CardContent>
                            <Button size="sm" variant="outline" asChild>
                                <a
                                    href="https://laminar.asgard.internal/projects"
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1.5"
                                >
                                    Open Laminar
                                    <ExternalLink className="w-3 h-3" />
                                </a>
                            </Button>
                        </CardContent>
                    </Card>
                </>
            )}
        </div>
    );
}
