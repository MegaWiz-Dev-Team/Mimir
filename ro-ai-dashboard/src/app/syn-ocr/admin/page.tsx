"use client";

// Sprint 50 B-50l — Tenant policy admin form for the current X-Tenant-Id.
//
// PATCHes /api/v1/syn/ocr/policy on syn-api with whatever fields were
// changed in the form. The tenant scope comes from the cookie/header so a
// caller can only edit their own tenant's row — cross-tenant admin needs a
// different surface.

import { useEffect, useState, FormEvent } from "react";
import Link from "next/link";
import { authFetch, SYN_API_BASE_URL } from "@/lib/api";

interface TenantPolicy {
    tenant_id: string;
    ocr_cloud_flash_enabled: boolean;
    ocr_cloud_pro_enabled: boolean;
    ocr_phi_strict: boolean;
    ocr_monthly_cloud_budget_usd: number;
    pii_mode: string;
    pii_custom_patterns: string | null;
}

const PII_MODES = ["off", "detect-only", "mask-and-send", "block-on-pii"];

export default function SynOcrAdminPage() {
    const [policy, setPolicy] = useState<TenantPolicy | null>(null);
    const [draft, setDraft] = useState<TenantPolicy | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [savedAt, setSavedAt] = useState<string | null>(null);

    const load = async () => {
        setLoading(true);
        setError(null);
        try {
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/policy`, {
                cache: "no-store",
            });
            if (!r.ok) {
                setError(`${r.status} ${r.statusText}`);
                return;
            }
            const j = (await r.json()) as TenantPolicy;
            setPolicy(j);
            setDraft(j);
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "fetch failed");
        } finally {
            setLoading(false);
        }
    };
    useEffect(() => {
        load();
    }, []);

    if (!draft) {
        return (
            <div className="p-6">
                <h1 className="text-2xl font-bold mb-2">Tenant OCR Policy</h1>
                {loading ? <div>Loading…</div> : <div className="text-red-700">{error || "no policy"}</div>}
            </div>
        );
    }

    const dirty = policy
        ? JSON.stringify(policy) !== JSON.stringify(draft)
        : false;

    // Compute a structural diff so the user knows exactly what gets PATCHed.
    const diff: Partial<TenantPolicy> = {};
    if (policy) {
        (Object.keys(draft) as (keyof TenantPolicy)[]).forEach((k) => {
            if (k === "tenant_id") return;
            if (draft[k] !== policy[k]) {
                // @ts-expect-error narrow at runtime
                diff[k] = draft[k];
            }
        });
    }

    const validProEscalation =
        !draft.ocr_cloud_pro_enabled || draft.ocr_cloud_flash_enabled;

    const submit = async (ev: FormEvent) => {
        ev.preventDefault();
        if (!dirty) return;
        if (!validProEscalation) {
            setError(
                "ocr_cloud_pro_enabled requires ocr_cloud_flash_enabled (Pro is an escalation of Flash)"
            );
            return;
        }
        setSaving(true);
        setError(null);
        try {
            const r = await authFetch(`${SYN_API_BASE_URL}/syn/ocr/policy`, {
                method: "PATCH",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(diff),
            });
            const text = await r.text();
            if (!r.ok) {
                setError(`${r.status} — ${text}`);
                return;
            }
            const j = JSON.parse(text) as TenantPolicy;
            setPolicy(j);
            setDraft(j);
            setSavedAt(new Date().toLocaleTimeString());
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "save failed");
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="p-6 max-w-3xl">
            <div className="flex items-center justify-between mb-2">
                <h1 className="text-2xl font-bold">Tenant OCR Policy</h1>
                <Link href="/syn-ocr" className="text-sm text-blue-700 hover:underline">
                    ← back to audit
                </Link>
            </div>
            <p className="text-sm text-slate-600 mb-6">
                Editing <code className="bg-slate-100 px-1 rounded">{draft.tenant_id}</code> — changes apply
                immediately to all subsequent OCR calls. Read-only to non-admins; the syn-api endpoint trusts
                the X-Tenant-Id header today (proper RBAC arrives in B-50l.2).
            </p>

            {error && (
                <div className="mb-4 p-3 bg-red-50 border border-red-200 text-red-800 rounded text-sm">
                    {error}
                </div>
            )}
            {savedAt && !dirty && (
                <div className="mb-4 p-3 bg-green-50 border border-green-200 text-green-800 rounded text-sm">
                    Saved at {savedAt}
                </div>
            )}

            <form onSubmit={submit} className="space-y-5 bg-white border rounded p-5 shadow-sm">
                <Toggle
                    label="PHI strict"
                    description="Hard block — never send images to cloud regardless of opt-in flags. Override at the call layer kicks in even on manual_override."
                    value={draft.ocr_phi_strict}
                    onChange={(v) => setDraft({ ...draft, ocr_phi_strict: v })}
                    accent={draft.ocr_phi_strict ? "warn" : "neutral"}
                />
                <Toggle
                    label="Cloud Flash enabled"
                    description="Enable Tier 2 Gemini 3 Flash for cloud OCR fallback when local engines fail or low-confidence."
                    value={draft.ocr_cloud_flash_enabled}
                    onChange={(v) =>
                        setDraft({
                            ...draft,
                            ocr_cloud_flash_enabled: v,
                            // turning Flash off forces Pro off too
                            ocr_cloud_pro_enabled: v ? draft.ocr_cloud_pro_enabled : false,
                        })
                    }
                />
                <Toggle
                    label="Cloud Pro enabled"
                    description="Enable Tier 3 Gemini 3.1 Pro for high-stakes / Curator-flagged calls. Requires Flash also enabled."
                    value={draft.ocr_cloud_pro_enabled}
                    onChange={(v) => setDraft({ ...draft, ocr_cloud_pro_enabled: v })}
                    disabled={!draft.ocr_cloud_flash_enabled}
                />

                <div>
                    <label className="block text-sm font-medium mb-1">
                        Monthly cloud budget cap (USD)
                    </label>
                    <input
                        type="number"
                        step="0.01"
                        min="0"
                        value={draft.ocr_monthly_cloud_budget_usd}
                        onChange={(e) =>
                            setDraft({
                                ...draft,
                                ocr_monthly_cloud_budget_usd: parseFloat(e.target.value || "0"),
                            })
                        }
                        className="w-48 border rounded px-2 py-1 text-sm"
                    />
                    <p className="text-xs text-slate-500 mt-1">
                        0 = no cap. Cloud calls reject with <code>budget_exceeded</code> when month-to-date
                        sum + projected call cost would exceed this. DECIMAL(10,2) — minimum representable
                        cap is $0.01.
                    </p>
                </div>

                <div>
                    <label className="block text-sm font-medium mb-1">PII mode (Skuggi)</label>
                    <select
                        value={draft.pii_mode}
                        onChange={(e) => setDraft({ ...draft, pii_mode: e.target.value })}
                        className="w-64 border rounded px-2 py-1 text-sm"
                    >
                        {PII_MODES.map((m) => (
                            <option key={m} value={m}>
                                {m}
                            </option>
                        ))}
                    </select>
                    <p className="text-xs text-slate-500 mt-1">
                        See ADR-007. <code>off</code> only for legacy; <code>mask-and-send</code> is the
                        recommended default; <code>block-on-pii</code> is strictest.
                    </p>
                </div>

                <div>
                    <label className="block text-sm font-medium mb-1">
                        PII custom patterns (JSON)
                    </label>
                    <textarea
                        rows={3}
                        value={draft.pii_custom_patterns ?? ""}
                        onChange={(e) =>
                            setDraft({
                                ...draft,
                                pii_custom_patterns: e.target.value || null,
                            })
                        }
                        placeholder={`[{"name":"my_hospital_mrn","regex":"^MR\\\\d{8}$"}]`}
                        className="w-full border rounded px-2 py-1 text-sm font-mono"
                    />
                </div>

                {!validProEscalation && (
                    <div className="p-2 bg-yellow-50 border border-yellow-200 text-yellow-800 rounded text-sm">
                        Cloud Pro requires Cloud Flash to also be on.
                    </div>
                )}

                <div className="flex items-center gap-3 pt-2 border-t">
                    <button
                        type="submit"
                        disabled={!dirty || saving || !validProEscalation}
                        className="px-4 py-2 bg-slate-900 text-white rounded text-sm font-semibold disabled:opacity-50"
                    >
                        {saving ? "Saving…" : dirty ? `Save ${Object.keys(diff).length} change(s)` : "No changes"}
                    </button>
                    <button
                        type="button"
                        onClick={() => policy && setDraft(policy)}
                        disabled={!dirty || saving}
                        className="px-4 py-2 bg-slate-100 hover:bg-slate-200 rounded text-sm disabled:opacity-50"
                    >
                        Revert
                    </button>
                    {dirty && (
                        <details className="ml-auto text-xs text-slate-600">
                            <summary className="cursor-pointer">PATCH body</summary>
                            <pre className="mt-1 p-2 bg-slate-50 rounded">{JSON.stringify(diff, null, 2)}</pre>
                        </details>
                    )}
                </div>
            </form>
        </div>
    );
}

function Toggle({
    label,
    description,
    value,
    onChange,
    disabled,
    accent,
}: {
    label: string;
    description?: string;
    value: boolean;
    onChange: (v: boolean) => void;
    disabled?: boolean;
    accent?: "warn" | "neutral";
}) {
    return (
        <div className="flex items-start gap-3">
            <button
                type="button"
                onClick={() => !disabled && onChange(!value)}
                disabled={disabled}
                className={`mt-1 relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                    disabled
                        ? "bg-slate-200 cursor-not-allowed"
                        : value
                          ? accent === "warn"
                              ? "bg-orange-600"
                              : "bg-blue-600"
                          : "bg-slate-300"
                }`}
            >
                <span
                    className={`inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${
                        value ? "translate-x-5" : "translate-x-0.5"
                    }`}
                />
            </button>
            <div className="flex-1">
                <div className="text-sm font-medium">
                    {label} <span className="text-xs text-slate-500">({value ? "on" : "off"})</span>
                </div>
                {description && <div className="text-xs text-slate-600 mt-0.5">{description}</div>}
            </div>
        </div>
    );
}
