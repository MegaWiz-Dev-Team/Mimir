// Sprint 47 B-47g — clinician-curated rag_benchmark_items API client.
// Talks to /api/v1/rag-benchmark/* in mimir-api (see Mimir/ro-ai-bridge/src/routes/rag_benchmark.rs).

import { API_BASE_URL, authFetch } from "./api";

export interface RagBenchmarkItem {
    id: string;
    benchmark_id: string;
    question_id: string;
    collection_id: string;
    relevant_chunk_ids: string[];
    required_topics: string[] | null;
    notes: string | null;
    tenant_id: string;
    curated_by: string | null;
    curated_at: string;
    updated_at: string | null;
}

export interface CandidateChunk {
    chunk_id: string;
    source: string;
    title: string;
    score: number;
    content_preview: string;
    already_gold: boolean;
}

export interface CandidatesResponse {
    question_id: string;
    candidates: CandidateChunk[];
    source_run_id: string | null;
    source_run_started_at: string | null;
}

export interface CreateRagItemPayload {
    benchmark_id: string;
    question_id: string;
    collection_id: string;
    relevant_chunk_ids: string[];
    required_topics?: string[];
    notes?: string;
}

export interface UpdateRagItemPayload {
    relevant_chunk_ids?: string[];
    required_topics?: string[];
    notes?: string;
}

export async function listRagBenchmarkItems(
    benchmarkId: string,
    opts: { limit?: number; offset?: number } = {},
): Promise<RagBenchmarkItem[]> {
    const url = new URL(`${API_BASE_URL}/rag-benchmark/items`);
    url.searchParams.set("benchmark_id", benchmarkId);
    if (opts.limit) url.searchParams.set("limit", String(opts.limit));
    if (opts.offset) url.searchParams.set("offset", String(opts.offset));
    const res = await authFetch(url.toString(), { cache: "no-store" });
    if (!res.ok) throw new Error(`list rag-benchmark items: ${res.status}`);
    const rows = (await res.json()) as RagBenchmarkItem[];
    // The backend returns relevant_chunk_ids as a JSON-encoded value; coerce.
    return rows.map((r) => ({
        ...r,
        relevant_chunk_ids: Array.isArray(r.relevant_chunk_ids)
            ? r.relevant_chunk_ids
            : (typeof r.relevant_chunk_ids === "string"
                ? safeParseArray(r.relevant_chunk_ids)
                : []),
        required_topics:
            r.required_topics === null
                ? null
                : (Array.isArray(r.required_topics)
                    ? r.required_topics
                    : safeParseArray(r.required_topics as unknown as string)),
    }));
}

export async function getCandidates(
    questionId: string,
    opts: { collection_id?: string; limit?: number } = {},
): Promise<CandidatesResponse> {
    const url = new URL(`${API_BASE_URL}/rag-benchmark/items/${encodeURIComponent(questionId)}/candidates`);
    if (opts.collection_id) url.searchParams.set("collection_id", opts.collection_id);
    if (opts.limit) url.searchParams.set("limit", String(opts.limit));
    const res = await authFetch(url.toString(), { cache: "no-store" });
    if (!res.ok) throw new Error(`get candidates: ${res.status}`);
    return (await res.json()) as CandidatesResponse;
}

export async function createRagBenchmarkItem(
    payload: CreateRagItemPayload,
): Promise<RagBenchmarkItem> {
    const res = await authFetch(`${API_BASE_URL}/rag-benchmark/items`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
    });
    if (!res.ok) {
        const body = await res.text();
        throw new Error(`create rag-benchmark item: ${res.status} ${body}`);
    }
    return (await res.json()) as RagBenchmarkItem;
}

export async function updateRagBenchmarkItem(
    id: string,
    payload: UpdateRagItemPayload,
): Promise<RagBenchmarkItem> {
    const res = await authFetch(`${API_BASE_URL}/rag-benchmark/items/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
    });
    if (!res.ok) {
        const body = await res.text();
        throw new Error(`update rag-benchmark item: ${res.status} ${body}`);
    }
    return (await res.json()) as RagBenchmarkItem;
}

function safeParseArray(s: unknown): string[] {
    if (typeof s !== "string") return [];
    try {
        const v = JSON.parse(s);
        return Array.isArray(v) ? v.map(String) : [];
    } catch {
        return [];
    }
}
