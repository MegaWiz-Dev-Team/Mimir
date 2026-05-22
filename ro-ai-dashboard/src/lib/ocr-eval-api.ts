// Client for the OCR *layout* evaluation API (Sprint 53).
//
// Backend: ro-ai-bridge/src/routes/eval_ocr_layout.rs
//   GET /eval/ocr/layout/runs        — list (filter: eval_kind, syn_version, dataset_name)
//   GET /eval/ocr/layout/runs/{id}   — detail + per-image items
//
// Tenant: the route scopes by the X-Tenant-Id header (default asgard_platform).
// Each call here passes an explicit X-Tenant-Id so the page's tenant dropdown
// can view a chosen tenant's runs without mutating the global tenant cookie.
// (authFetch merges options.headers over the cookie-derived header.)

import { authFetch, API_BASE_URL } from "@/lib/api";

const BASE = `${API_BASE_URL}/eval/ocr/layout`;

/** Layout/geometric eval kinds the backend accepts. */
export type EvalKind = "mAP" | "parity" | "grits";

/**
 * Tenants selectable in the eval viewer. asgard_platform holds cross-cutting
 * engineering benchmarks (the default); the domain tenants hold their own
 * layout-eval runs.
 */
export const OCR_EVAL_TENANTS = [
    "asgard_platform",
    "asgard_medical",
    "asgard_insurance",
    "asgard_wellness",
] as const;
export type OcrEvalTenant = (typeof OCR_EVAL_TENANTS)[number];

export interface OcrLayoutRunSummary {
    id: string;
    eval_kind: EvalKind;
    syn_version: string;
    model_name: string;
    dataset_name: string;
    is_synthetic: boolean;
    n_images: number | null;
    n_gt_regions: number | null;
    n_predictions: number | null;
    summary: Record<string, unknown> | null;
    started_at: string | null;
    finished_at: string | null;
    duration_ms: number | null;
    created_at: string | null;
}

export interface OcrLayoutItem {
    id: string;
    image_name: string | null;
    image_hash: string | null;
    image_width: number | null;
    image_height: number | null;
    n_gt: number;
    n_pred: number;
    n_matched: number;
    metrics: Record<string, unknown> | null;
    latency_ms: number | null;
    created_at: string | null;
}

export interface OcrLayoutRunDetail extends OcrLayoutRunSummary {
    tenant: string;
    commit_sha: string | null;
    model_sha256: string | null;
    dataset_hash: string | null;
    iou_threshold: number | null;
    items: OcrLayoutItem[];
}

export interface ListRunsResponse {
    runs: OcrLayoutRunSummary[];
    tenant: string;
    limit: number;
    offset: number;
}

export interface ListRunsParams {
    tenant: string;
    eval_kind?: EvalKind | "";
    syn_version?: string;
    dataset_name?: string;
    limit?: number;
    offset?: number;
}

function tenantHeader(tenant: string): RequestInit {
    return { headers: { "X-Tenant-Id": tenant }, cache: "no-store" };
}

export async function listOcrLayoutRuns(params: ListRunsParams): Promise<ListRunsResponse> {
    const qs = new URLSearchParams();
    if (params.eval_kind) qs.set("eval_kind", params.eval_kind);
    if (params.syn_version) qs.set("syn_version", params.syn_version);
    if (params.dataset_name) qs.set("dataset_name", params.dataset_name);
    if (params.limit != null) qs.set("limit", String(params.limit));
    if (params.offset != null) qs.set("offset", String(params.offset));
    const query = qs.toString() ? `?${qs}` : "";

    const res = await authFetch(`${BASE}/runs${query}`, tenantHeader(params.tenant));
    if (!res.ok) throw new Error(`Failed to list OCR layout runs (${res.status})`);
    return res.json();
}

export async function getOcrLayoutRun(id: string, tenant: string): Promise<OcrLayoutRunDetail> {
    const res = await authFetch(`${BASE}/runs/${id}`, tenantHeader(tenant));
    if (res.status === 404) throw new Error(`Run ${id} not found for tenant ${tenant}`);
    if (!res.ok) throw new Error(`Failed to load OCR layout run (${res.status})`);
    return res.json();
}

/**
 * Pull a single headline score out of a run summary for the list view.
 * Shape depends on eval_kind (see schema comments). Returns null if unknown.
 */
export function headlineScore(run: OcrLayoutRunSummary): { label: string; value: string } | null {
    const s = run.summary as Record<string, unknown> | null;
    if (!s) return null;
    if (run.eval_kind === "mAP") {
        const ca = s.class_agnostic as Record<string, unknown> | undefined;
        const ap = ca?.ap50;
        if (typeof ap === "number") return { label: "mAP@50", value: ap.toFixed(3) };
    } else if (run.eval_kind === "parity") {
        const d = s.max_abs_diff;
        if (typeof d === "number") return { label: "max Δ", value: d.toExponential(2) };
    } else if (run.eval_kind === "grits") {
        const g = s.grits_top;
        if (typeof g === "number") return { label: "GriTS", value: g.toFixed(3) };
    }
    return null;
}
