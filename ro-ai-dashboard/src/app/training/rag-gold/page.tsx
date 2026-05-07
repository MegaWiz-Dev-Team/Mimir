"use client";

// Sprint 47 B-47g — Curator UI for rag_benchmark_items (clinician gold).
//
// Workflow:
//   1. Clinician enters benchmark_id + question_id (or picks from list).
//   2. UI fetches candidate chunks from a recent eval run for that question.
//   3. Multi-select chunks (1-9 toggle, or click). Selected = "relevant".
//   4. Optional: add required_topics tags + clinician notes.
//   5. Save → POST creates new rag_benchmark_items row OR updates existing.
//
// Keyboard shortcuts:
//   1-9     toggle candidate chunk #N (visible)
//   S       save current selection
//   N       clear selection
//   Esc     cancel any focus

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import {
    listRagBenchmarkItems,
    getCandidates,
    createRagBenchmarkItem,
    type CandidateChunk,
    type RagBenchmarkItem,
} from "../../../lib/rag-benchmark-api";

const DEFAULT_BENCHMARK = "hb-pro-asgard-001";
const DEFAULT_COLLECTION = "medical_knowledge";

export default function RagGoldPage() {
    const [benchmarkId, setBenchmarkId] = useState(DEFAULT_BENCHMARK);
    const [questionId, setQuestionId] = useState("");
    const [collectionId, setCollectionId] = useState(DEFAULT_COLLECTION);

    const [candidates, setCandidates] = useState<CandidateChunk[]>([]);
    const [sourceRun, setSourceRun] = useState<{ id: string | null; ts: string | null }>({ id: null, ts: null });
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [topicsRaw, setTopicsRaw] = useState("");
    const [notes, setNotes] = useState("");

    const [recentItems, setRecentItems] = useState<RagBenchmarkItem[]>([]);
    const [loading, setLoading] = useState(false);
    const [saveStatus, setSaveStatus] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    const loadCandidates = useCallback(async () => {
        if (!questionId.trim()) return;
        setLoading(true);
        setError(null);
        try {
            const r = await getCandidates(questionId.trim(), { limit: 32 });
            setCandidates(r.candidates);
            setSourceRun({ id: r.source_run_id, ts: r.source_run_started_at });
            // Pre-select chunks already in gold (already_gold true).
            const preselected = new Set(r.candidates.filter((c) => c.already_gold).map((c) => c.chunk_id));
            setSelected(preselected);
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "candidates fetch failed");
        } finally {
            setLoading(false);
        }
    }, [questionId]);

    const loadRecent = useCallback(async () => {
        try {
            const rows = await listRagBenchmarkItems(benchmarkId, { limit: 20 });
            setRecentItems(rows);
        } catch (e: unknown) {
            // Non-fatal: just leave list empty.
            console.warn("listRagBenchmarkItems:", e);
        }
    }, [benchmarkId]);

    useEffect(() => {
        loadRecent();
    }, [loadRecent]);

    const toggleChunk = (chunkId: string) => {
        setSelected((prev) => {
            const next = new Set(prev);
            if (next.has(chunkId)) next.delete(chunkId);
            else next.add(chunkId);
            return next;
        });
    };

    const handleSave = useCallback(async () => {
        if (!questionId.trim()) {
            setError("Question ID required");
            return;
        }
        if (selected.size === 0) {
            setError("Select at least 1 chunk before saving");
            return;
        }
        setError(null);
        setSaveStatus("Saving…");
        try {
            const topics = topicsRaw
                .split(",")
                .map((t) => t.trim())
                .filter(Boolean);
            const r = await createRagBenchmarkItem({
                benchmark_id: benchmarkId,
                question_id: questionId.trim(),
                collection_id: collectionId,
                relevant_chunk_ids: Array.from(selected),
                required_topics: topics.length > 0 ? topics : undefined,
                notes: notes.trim() || undefined,
            });
            setSaveStatus(`Saved ✓ (id: ${r.id.slice(0, 8)}…)`);
            await loadRecent();
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : "save failed");
            setSaveStatus(null);
        }
    }, [benchmarkId, questionId, collectionId, selected, topicsRaw, notes, loadRecent]);

    // Keyboard shortcuts
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            // Don't intercept inside input/textarea.
            const tag = (e.target as HTMLElement).tagName;
            if (tag === "INPUT" || tag === "TEXTAREA") return;

            if (e.key === "s" || e.key === "S") {
                e.preventDefault();
                handleSave();
            } else if (e.key === "n" || e.key === "N") {
                e.preventDefault();
                setSelected(new Set());
            } else if (/^[1-9]$/.test(e.key)) {
                const idx = parseInt(e.key, 10) - 1;
                if (idx < candidates.length) {
                    toggleChunk(candidates[idx].chunk_id);
                }
            }
        };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, [candidates, handleSave]);

    return (
        <main className="container mx-auto p-6 max-w-6xl">
            <header className="mb-6">
                <Link href="/training" className="text-sm text-blue-600 hover:underline">
                    ← Back to /training
                </Link>
                <h1 className="text-2xl font-semibold mt-2">RAG Benchmark Gold (B-47g)</h1>
                <p className="text-sm text-gray-600 mt-1">
                    Label which retrieved chunks are <em>actually relevant</em> to a benchmark question.
                    Used by Sprint 47 retrieval metrics (Recall@k, MRR, NDCG@k) and B-47e gold_only mode.
                </p>
            </header>

            <section className="border rounded p-4 mb-6 bg-gray-50">
                <h2 className="font-medium mb-3">Question setup</h2>
                <div className="grid grid-cols-3 gap-3">
                    <label className="text-sm">
                        Benchmark
                        <input
                            type="text"
                            value={benchmarkId}
                            onChange={(e) => setBenchmarkId(e.target.value)}
                            className="block w-full border rounded px-2 py-1 mt-1 font-mono text-xs"
                        />
                    </label>
                    <label className="text-sm">
                        Question ID
                        <input
                            type="text"
                            value={questionId}
                            onChange={(e) => setQuestionId(e.target.value)}
                            placeholder="e.g. 9566084de89c416408691006a6f06f9c"
                            className="block w-full border rounded px-2 py-1 mt-1 font-mono text-xs"
                        />
                    </label>
                    <label className="text-sm">
                        Collection
                        <input
                            type="text"
                            value={collectionId}
                            onChange={(e) => setCollectionId(e.target.value)}
                            className="block w-full border rounded px-2 py-1 mt-1 font-mono text-xs"
                        />
                    </label>
                </div>
                <button
                    onClick={loadCandidates}
                    disabled={loading || !questionId.trim()}
                    className="mt-3 px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:bg-gray-300"
                >
                    {loading ? "Loading…" : "Load candidates"}
                </button>
                {sourceRun.id && (
                    <p className="text-xs text-gray-500 mt-2">
                        Candidates from run <code>{sourceRun.id.slice(0, 8)}…</code>
                        {sourceRun.ts && ` started ${new Date(sourceRun.ts).toLocaleString()}`}
                    </p>
                )}
            </section>

            {candidates.length > 0 && (
                <section className="mb-6">
                    <div className="flex items-center justify-between mb-2">
                        <h2 className="font-medium">
                            Candidates ({candidates.length}) · Selected: {selected.size}
                        </h2>
                        <span className="text-xs text-gray-500">Press 1-9 to toggle · S save · N clear</span>
                    </div>
                    <div className="border rounded divide-y">
                        {candidates.map((c, i) => {
                            const isSelected = selected.has(c.chunk_id);
                            return (
                                <button
                                    key={c.chunk_id}
                                    onClick={() => toggleChunk(c.chunk_id)}
                                    className={`w-full text-left px-3 py-2 hover:bg-blue-50 transition-colors ${isSelected ? "bg-blue-100" : ""
                                        }`}
                                >
                                    <div className="flex items-center gap-3">
                                        <span className="text-xs font-mono w-6 text-gray-500">
                                            {i < 9 ? i + 1 : "·"}
                                        </span>
                                        <input
                                            type="checkbox"
                                            checked={isSelected}
                                            readOnly
                                            className="pointer-events-none"
                                        />
                                        <span className="text-xs font-mono text-purple-700 min-w-[10ch]">{c.source}</span>
                                        <span className="text-sm font-medium flex-1 truncate">{c.title}</span>
                                        <span className="text-xs text-gray-500">{c.score.toFixed(3)}</span>
                                        {c.already_gold && (
                                            <span className="text-xs text-green-700 bg-green-100 px-1 rounded">prev gold</span>
                                        )}
                                    </div>
                                    <p className="text-xs text-gray-600 mt-1 ml-12 truncate">{c.content_preview}</p>
                                </button>
                            );
                        })}
                    </div>
                </section>
            )}

            {candidates.length > 0 && (
                <section className="mb-6 grid grid-cols-2 gap-4">
                    <label className="text-sm">
                        Required topics (comma-sep)
                        <input
                            type="text"
                            value={topicsRaw}
                            onChange={(e) => setTopicsRaw(e.target.value)}
                            placeholder="STEMI inferior, reperfusion, RV-MI"
                            className="block w-full border rounded px-2 py-1 mt-1 text-sm"
                        />
                    </label>
                    <label className="text-sm">
                        Clinician notes
                        <input
                            type="text"
                            value={notes}
                            onChange={(e) => setNotes(e.target.value)}
                            placeholder="e.g. ground rationale, edge cases"
                            className="block w-full border rounded px-2 py-1 mt-1 text-sm"
                        />
                    </label>
                </section>
            )}

            {candidates.length > 0 && (
                <section className="mb-6">
                    <button
                        onClick={handleSave}
                        disabled={selected.size === 0}
                        className="px-5 py-2 bg-green-600 text-white rounded font-medium hover:bg-green-700 disabled:bg-gray-300"
                    >
                        Save gold ({selected.size} chunks)
                    </button>
                    {saveStatus && <span className="ml-3 text-sm text-green-700">{saveStatus}</span>}
                    {error && <span className="ml-3 text-sm text-red-600">{error}</span>}
                </section>
            )}

            <section>
                <h2 className="font-medium mb-2">Recent gold items in {benchmarkId}</h2>
                {recentItems.length === 0 ? (
                    <p className="text-sm text-gray-500">No gold items yet. Label the first one above.</p>
                ) : (
                    <table className="w-full text-sm border-collapse">
                        <thead className="text-xs text-gray-600 border-b">
                            <tr>
                                <th className="text-left p-2">Question ID</th>
                                <th className="text-left p-2">Collection</th>
                                <th className="text-left p-2"># chunks</th>
                                <th className="text-left p-2">Curated by</th>
                                <th className="text-left p-2">Curated at</th>
                            </tr>
                        </thead>
                        <tbody>
                            {recentItems.map((it) => (
                                <tr key={it.id} className="border-b hover:bg-gray-50">
                                    <td className="p-2 font-mono text-xs">{it.question_id.slice(0, 16)}…</td>
                                    <td className="p-2 font-mono text-xs">{it.collection_id}</td>
                                    <td className="p-2">{it.relevant_chunk_ids.length}</td>
                                    <td className="p-2 text-xs">{it.curated_by ?? "—"}</td>
                                    <td className="p-2 text-xs">{new Date(it.curated_at).toLocaleString()}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>
        </main>
    );
}
