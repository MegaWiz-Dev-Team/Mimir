// Client for the unified evaluation scoreboard (evx_* layer).
//
// Backend: ro-ai-bridge/src/routes/evx.rs
//   GET /api/v1/eval/scoreboard?family=...  — one row per (target, dataset) run
//                                             with its primary metric.
//
// Tenant is taken from the X-Tenant-Id header (authFetch merges it); pass an
// explicit tenant to view another tenant's board without mutating the cookie.

import { authFetch, API_BASE_URL } from "@/lib/api";

const BASE = `${API_BASE_URL}/eval/scoreboard`;

export interface ScoreboardRow {
    family: string;
    run_id: string;
    experiment_id: string | null;
    tenant_id: string | null;
    target_kind: string;
    target_name: string;
    model_id: string | null;
    runtime: string | null;
    dataset_id: string | null;
    n_items: number;
    primary_metric: string | null;
    primary_value: number | null;
    unit: string | null;
    higher_is_better: number | null;
    ci_low: number | null;
    ci_high: number | null;
    finished_at: string | null;
}

export async function getScoreboard(
    opts: { family?: string; tenant?: string } = {},
): Promise<ScoreboardRow[]> {
    const url = new URL(BASE);
    if (opts.family) url.searchParams.set("family", opts.family);

    const headers: Record<string, string> = {};
    if (opts.tenant) headers["X-Tenant-Id"] = opts.tenant;

    const res = await authFetch(url.toString(), { headers });
    if (!res.ok) throw new Error(`scoreboard ${res.status}`);
    return (await res.json()) as ScoreboardRow[];
}
