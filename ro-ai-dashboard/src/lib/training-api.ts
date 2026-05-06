// Sprint 39 Mimir Curator + LoRA tracking API client
// Talks to /api/v1/training/* endpoints in mimir-api (see Mimir/ro-ai-bridge/src/routes/training.rs).

import { API_BASE_URL, authFetch } from "./api";

export interface CorpusDataset {
    id: string;
    name: string;
    description: string | null;
    tenant_id: string | null; // null = shared, non-null = tenant-scoped
    source: string | null;
    metadata: string | null;
    status: string;
    total_items: number;
    approved_items: number;
    rejected_items: number;
    created_at: string;
    created_by: string | null;
}

export interface CorpusItem {
    id: number;
    dataset_id: string;
    question: string;
    ai_answer: string;
    expected_answer: string | null;
    citations: string | null;
    accuracy_score: number | null;
    completeness_score: number | null;
    relevance_score: number | null;
    safety_score: number | null;
    improved_answer: string | null;
    specialty: string | null;
    /** JSON-stringified array of cross-cutting tags. Parse via parseTags(). */
    tags: string | null;
    status: string;
    reviewer_id: string | null;
    reviewer_notes: string | null;
    reviewed_at: string | null;
    tenant_id: string | null;
    created_at: string;
}

/** Convert tags JSON string to array. Returns [] if null/invalid. */
export function parseTags(tagsJson: string | null): string[] {
    if (!tagsJson) return [];
    try {
        const parsed = JSON.parse(tagsJson);
        return Array.isArray(parsed) ? parsed.filter((t) => typeof t === "string") : [];
    } catch {
        return [];
    }
}

/** Common cross-cutting tags suggested in the UI autocomplete. */
export const COMMON_TAGS = [
    "pharmacy",
    "pregnancy",
    "pediatric",
    "geriatric",
    "urgent",
    "red-flag",
    "dosing",
    "screening",
    "monitoring",
    "differential-dx",
    "interaction",
    "contraindication",
    "anticoagulation",
    "antibiotic",
    "vaccination",
    "imaging",
    "lab-interpretation",
    "icu",
    "outpatient",
    "preoperative",
    "postoperative",
];

export interface ReviewSubmission {
    accuracy_score?: number | null;
    completeness_score?: number | null;
    relevance_score?: number | null;
    safety_score?: number | null;
    improved_answer?: string;
    specialty?: string;
    /**
     * Cross-cutting tags. Semantics:
     *   undefined / omitted → leave existing tags unchanged
     *   []                  → clear all tags
     *   ["a","b"]           → replace with this set
     */
    tags?: string[];
    notes?: string;
    status: "APPROVED" | "REJECTED" | "FLAGGED";
}

export interface CreateDatasetRequest {
    name: string;
    description?: string;
    tenant_id?: string | null; // omit = caller's tenant; null = shared
    source?: string;
    metadata?: Record<string, unknown>;
}

export interface ImportItem {
    question: string;
    ai_answer: string;
    expected_answer?: string;
    citations?: unknown;
    specialty?: string;
    /** Cross-cutting tags (Sprint 39 multi-tag). */
    tags?: string[];
}

export interface LoraRun {
    id: string;
    name: string | null;
    dataset_id: string | null;
    dataset_snapshot_hash: string | null;
    base_model_id: string;
    hyperparams: string | null;
    loss_curve: string | null;
    adapter_path: string | null;
    merged_model_id: string | null;
    status: string;
    status_message: string | null;
    started_at: string;
    finished_at: string | null;
    tenant_id: string | null;
    created_by: string | null;
    notes: string | null;
}

// ─── Datasets ────────────────────────────────────────────────────────────────

export async function listDatasets(): Promise<CorpusDataset[]> {
    const res = await authFetch(`${API_BASE_URL}/training/datasets`, { cache: "no-store" });
    if (!res.ok) throw new Error(`listDatasets ${res.status}`);
    return res.json();
}

export async function getDataset(id: string): Promise<CorpusDataset> {
    const res = await authFetch(`${API_BASE_URL}/training/datasets/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error(`getDataset ${res.status}`);
    return res.json();
}

export async function createDataset(req: CreateDatasetRequest): Promise<CorpusDataset> {
    const res = await authFetch(`${API_BASE_URL}/training/datasets`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(req),
    });
    if (!res.ok) throw new Error(`createDataset ${res.status}: ${await res.text()}`);
    return res.json();
}

export async function importItems(
    datasetId: string,
    items: ImportItem[]
): Promise<{ imported: number; dataset_id: string }> {
    const res = await authFetch(`${API_BASE_URL}/training/datasets/${datasetId}/items`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ items }),
    });
    if (!res.ok) throw new Error(`importItems ${res.status}: ${await res.text()}`);
    return res.json();
}

// ─── Review queue ────────────────────────────────────────────────────────────

export async function getNextItem(
    datasetId: string,
    opts?: { reviewer?: string; specialty?: string }
): Promise<CorpusItem | null> {
    const params = new URLSearchParams();
    if (opts?.reviewer) params.set("reviewer", opts.reviewer);
    if (opts?.specialty) params.set("specialty", opts.specialty);
    const qs = params.toString();
    const url = `${API_BASE_URL}/training/datasets/${datasetId}/queue${qs ? `?${qs}` : ""}`;
    const res = await authFetch(url, { cache: "no-store" });
    if (!res.ok) throw new Error(`getNextItem ${res.status}`);
    return res.json();
}

export async function submitReview(
    datasetId: string,
    itemId: number,
    review: ReviewSubmission
): Promise<CorpusItem> {
    const res = await authFetch(
        `${API_BASE_URL}/training/datasets/${datasetId}/items/${itemId}/review`,
        {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(review),
        }
    );
    if (!res.ok) throw new Error(`submitReview ${res.status}: ${await res.text()}`);
    return res.json();
}

export function exportDatasetUrl(datasetId: string): string {
    return `${API_BASE_URL}/training/datasets/${datasetId}/export.jsonl`;
}

// ─── LoRA runs ───────────────────────────────────────────────────────────────

export async function listLoraRuns(): Promise<LoraRun[]> {
    const res = await authFetch(`${API_BASE_URL}/training/runs`, { cache: "no-store" });
    if (!res.ok) throw new Error(`listLoraRuns ${res.status}`);
    return res.json();
}

export async function getLoraRun(id: string): Promise<LoraRun> {
    const res = await authFetch(`${API_BASE_URL}/training/runs/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error(`getLoraRun ${res.status}`);
    return res.json();
}
