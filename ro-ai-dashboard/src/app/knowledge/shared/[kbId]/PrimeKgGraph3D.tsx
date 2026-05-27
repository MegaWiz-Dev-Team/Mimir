"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import {
    searchPrimekgEntity,
    fetchPrimekgNeighbors,
    askPrimekgAssistantStream,
    type PrimekgEntity,
} from "@/lib/api";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Loader2, Search, X, Sparkles, Send, PanelRightClose, PanelRightOpen } from "lucide-react";

type ChatTurn = {
    role: "user" | "assistant" | "system";
    content: string;
    turn: number;
};

const ForceGraph3D = dynamic(() => import("react-force-graph-3d"), {
    ssr: false,
    loading: () => (
        <div className="flex items-center justify-center h-full">
            <Loader2 className="w-8 h-8 animate-spin text-indigo-500" />
        </div>
    ),
});

// PrimeKG node types → colours (white-background friendly).
const TYPE_COLORS: Record<string, string> = {
    disease: "#e11d48",
    drug: "#2563eb",
    "gene/protein": "#16a34a",
    "effect/phenotype": "#f59e0b",
    exposure: "#9333ea",
    anatomy: "#0d9488",
    biological_process: "#0891b2",
    molecular_function: "#7c3aed",
    pathway: "#db2777",
    cellular_component: "#ca8a04",
};
const colorFor = (t: string) => TYPE_COLORS[t] || "#64748b";

type GNode = { id: string; label: string; type: string; color: string; val: number };
type GLink = { id: string; source: string; target: string; label: string };

/**
 * PrimeKG 3D browser — replaces the "not implemented" table for the PrimeKG KB
 * (PrimeKG lives in Neo4j). Search an entity to seed the graph, click any node
 * to pull in its neighbours. Auto-rotating, white background.
 */
export default function PrimeKgGraph3D() {
    const fgRef = useRef<any>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const [nodes, setNodes] = useState<Map<string, GNode>>(new Map());
    const [links, setLinks] = useState<Map<string, GLink>>(new Map());
    const [expanded, setExpanded] = useState<Set<string>>(new Set());
    const [selected, setSelected] = useState<GNode | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const [search, setSearch] = useState("");
    const [results, setResults] = useState<PrimekgEntity[]>([]);
    const [searching, setSearching] = useState(false);

    const [dim, setDim] = useState({ w: 800, h: 600 });

    // ── Medical Knowledge Assistant chat panel (restored 2026-05-27) ──────
    // Right-side aside. Streams replies from Bifrost's PrimeKG Graph Agent
    // (id=7) via `/api/v1/knowledge/primekg/assistant/stream`. Lost between
    // dashboard v2.3.36 (May 22, deployed) and v2.3.42 (current, rebuilt
    // without the WIP that never got committed) — see memory
    // `iris_swarm_chat_bifrost_gaps`.
    const [chatOpen, setChatOpen] = useState(true);
    const [chatTurns, setChatTurns] = useState<ChatTurn[]>([]);
    const [chatInput, setChatInput] = useState("");
    const [chatStreaming, setChatStreaming] = useState(false);
    const [chatStatus, setChatStatus] = useState<string | null>(null);
    const chatSessionId = useRef<string>(
        `primekg-${Math.random().toString(36).slice(2)}`,
    );
    const chatTurnRef = useRef(0);
    // The user's currently-selected entity becomes the "topic" prefix for
    // the next question — disambiguates "what causes this?" when the
    // graph has many nodes.
    const topicLabel = selected?.label;

    const sendQuestion = useCallback(
        async (rawText?: string) => {
            const text = (rawText ?? chatInput).trim();
            if (!text || chatStreaming) return;
            const turn = ++chatTurnRef.current;
            setChatInput("");
            setChatTurns((prev) => [...prev, { role: "user", content: text, turn }]);
            setChatStreaming(true);
            setChatStatus("consulting");
            // Anchor the query on the currently-selected entity so the
            // agent grounds its answer there (matches v2.3.36 behaviour).
            const query = topicLabel
                ? `Topic: ${topicLabel}\n\nQuestion: ${text}`
                : text;
            let accum = "";
            try {
                await askPrimekgAssistantStream(query, chatSessionId.current, {
                    onStatus: () => setChatStatus("consulting"),
                    onAnswer: (answer) => {
                        accum = answer;
                    },
                    onError: (msg) => {
                        setChatTurns((prev) => [
                            ...prev,
                            { role: "system", content: `Error: ${msg}`, turn },
                        ]);
                    },
                });
                if (accum) {
                    setChatTurns((prev) => [
                        ...prev,
                        { role: "assistant", content: accum, turn },
                    ]);
                }
            } catch (e: any) {
                setChatTurns((prev) => [
                    ...prev,
                    {
                        role: "system",
                        content: `Failed: ${e?.message || "unknown"}`,
                        turn,
                    },
                ]);
            } finally {
                setChatStreaming(false);
                setChatStatus(null);
            }
        },
        [chatInput, chatStreaming, topicLabel],
    );

    // Pull a node's neighbours into the graph.
    const expand = useCallback(
        async (idx: number, name: string, type: string) => {
            const centerId = String(idx);
            if (expanded.has(centerId)) return;
            setBusy(true);
            try {
                const d = await fetchPrimekgNeighbors(idx);
                setNodes((prev) => {
                    const m = new Map(prev);
                    if (!m.has(centerId)) {
                        m.set(centerId, { id: centerId, label: name, type, color: colorFor(type), val: 8 });
                    }
                    for (const n of d.neighbors) {
                        const id = String(n.neighbor_index);
                        if (!m.has(id)) {
                            m.set(id, {
                                id,
                                label: n.neighbor_name,
                                type: n.neighbor_type || "",
                                color: colorFor(n.neighbor_type || ""),
                                val: 4,
                            });
                        }
                    }
                    return m;
                });
                setLinks((prev) => {
                    const m = new Map(prev);
                    for (const n of d.neighbors) {
                        const nid = String(n.neighbor_index);
                        const [s, t] = n.direction === "incoming" ? [nid, centerId] : [centerId, nid];
                        const id = `${s}-${t}-${n.relation_type}`;
                        if (!m.has(id)) m.set(id, { id, source: s, target: t, label: n.relation_type });
                    }
                    return m;
                });
                setExpanded((prev) => new Set(prev).add(centerId));
                setError(null);
            } catch (e: any) {
                setError(e?.message || "Failed to load neighbours");
            } finally {
                setBusy(false);
            }
        },
        [expanded],
    );

    // Seed with a recognisable hub on first mount.
    useEffect(() => {
        (async () => {
            try {
                const hits = await searchPrimekgEntity("diabetes mellitus", 1);
                if (hits[0]) {
                    setSelected({
                        id: String(hits[0].entity_index),
                        label: hits[0].name,
                        type: hits[0].type,
                        color: colorFor(hits[0].type),
                        val: 8,
                    });
                    await expand(hits[0].entity_index, hits[0].name, hits[0].type);
                }
            } catch {
                /* seed best-effort */
            }
        })();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Track container size.
    useEffect(() => {
        const onResize = () => {
            if (containerRef.current) {
                setDim({ w: containerRef.current.clientWidth, h: containerRef.current.clientHeight });
            }
        };
        onResize();
        window.addEventListener("resize", onResize);
        return () => window.removeEventListener("resize", onResize);
    }, []);

    // Slow auto-rotation once the instance exists.
    useEffect(() => {
        const id = setInterval(() => {
            const controls = fgRef.current?.controls?.();
            if (controls) {
                controls.autoRotate = true;
                controls.autoRotateSpeed = 0.55;
                clearInterval(id);
            }
        }, 300);
        return () => clearInterval(id);
    }, []);

    // Debounced entity search.
    useEffect(() => {
        const q = search.trim();
        if (q.length < 2) {
            setResults([]);
            return;
        }
        const t = setTimeout(async () => {
            setSearching(true);
            try {
                setResults(await searchPrimekgEntity(q, 8));
            } catch {
                setResults([]);
            } finally {
                setSearching(false);
            }
        }, 350);
        return () => clearTimeout(t);
    }, [search]);

    const graphData = useMemo(
        () => ({ nodes: Array.from(nodes.values()), links: Array.from(links.values()) }),
        [nodes, links],
    );

    const focusNode = useCallback((node: any) => {
        setSelected(node);
        if (fgRef.current && node) {
            const hyp = Math.hypot(node.x || 1, node.y || 1, node.z || 1);
            const ratio = 1 + 90 / hyp;
            fgRef.current.cameraPosition(
                { x: (node.x || 0) * ratio, y: (node.y || 0) * ratio, z: (node.z || 0) * ratio },
                node,
                1200,
            );
        }
    }, []);

    const onNodeClick = useCallback(
        (node: any) => {
            focusNode(node);
            const idx = Number(node.id);
            if (!Number.isNaN(idx)) expand(idx, node.label, node.type);
        },
        [focusNode, expand],
    );

    const pickResult = useCallback(
        (e: PrimekgEntity) => {
            setSearch("");
            setResults([]);
            setSelected({
                id: String(e.entity_index),
                label: e.name,
                type: e.type,
                color: colorFor(e.type),
                val: 8,
            });
            expand(e.entity_index, e.name, e.type);
        },
        [expand],
    );

    return (
        <Card className="overflow-hidden">
            <CardContent className="p-0">
                <div className="flex" style={{ height: "70vh", background: "#ffffff" }}>
                <div className="relative flex-1 min-w-0" ref={containerRef} style={{ background: "#ffffff" }}>
                    {/* Search / entity picker */}
                    <div className="absolute top-3 left-3 z-10 w-[300px]">
                        <div className="relative">
                            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                            <Input
                                placeholder="ค้นหา entity (เช่น diabetes, aspirin)…"
                                value={search}
                                onChange={(e) => setSearch(e.target.value)}
                                className="pl-10 bg-white/90 border-slate-300 text-slate-800"
                            />
                            {searching && (
                                <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 animate-spin text-slate-400" />
                            )}
                        </div>
                        {results.length > 0 && (
                            <div className="mt-1 rounded-md border border-slate-200 bg-white shadow-lg overflow-hidden max-h-[320px] overflow-y-auto">
                                {results.map((r) => (
                                    <button
                                        key={r.entity_index}
                                        onClick={() => pickResult(r)}
                                        className="w-full text-left px-3 py-2 text-sm hover:bg-slate-100 flex items-center gap-2"
                                    >
                                        <span
                                            className="inline-block w-2.5 h-2.5 rounded-full shrink-0"
                                            style={{ background: colorFor(r.type) }}
                                        />
                                        <span className="truncate text-slate-700">{r.name}</span>
                                        <span className="ml-auto text-[10px] text-slate-400 shrink-0">{r.type}</span>
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    {/* Selected entity panel */}
                    {selected && (
                        <div className="absolute top-3 right-3 z-10 w-[260px] rounded-md border border-slate-200 bg-white/95 shadow-lg p-3">
                            <div className="flex items-start justify-between">
                                <div className="flex items-center gap-2">
                                    <span className="inline-block w-3 h-3 rounded-full" style={{ background: selected.color }} />
                                    <span className="text-xs font-medium uppercase tracking-wide text-slate-500">
                                        {selected.type || "entity"}
                                    </span>
                                </div>
                                <button onClick={() => setSelected(null)} className="text-slate-400 hover:text-slate-700">
                                    <X className="w-4 h-4" />
                                </button>
                            </div>
                            <p className="mt-2 text-sm font-semibold text-slate-800 break-words">{selected.label}</p>
                            <p className="mt-1 text-[11px] text-slate-400 font-mono">idx {selected.id}</p>
                            <p className="mt-2 text-[11px] text-slate-500">คลิกโหนดในกราฟเพื่อดึงเพื่อนบ้านเพิ่ม</p>
                        </div>
                    )}

                    {/* Counts + busy */}
                    <div className="absolute bottom-3 left-3 z-10 text-[11px] text-slate-400 flex items-center gap-2">
                        {busy && <Loader2 className="w-3 h-3 animate-spin" />}
                        {graphData.nodes.length} nodes · {graphData.links.length} edges
                    </div>

                    {error && (
                        <div className="absolute bottom-3 right-3 z-10 text-[11px] text-red-500">{error}</div>
                    )}

                    {graphData.nodes.length === 0 && !busy ? (
                        <div className="flex items-center justify-center h-full text-slate-400 text-sm">
                            ค้นหา entity ด้านบนเพื่อเริ่มสำรวจกราฟ PrimeKG
                        </div>
                    ) : (
                        <ForceGraph3D
                            ref={fgRef}
                            graphData={graphData as any}
                            width={dim.w}
                            height={dim.h}
                            backgroundColor="#ffffff"
                            showNavInfo={false}
                            nodeColor={(n: any) => n.color || "#64748b"}
                            nodeVal={(n: any) => n.val || 4}
                            nodeOpacity={0.95}
                            nodeLabel={(n: any) => `${n.label} · ${n.type || "entity"}`}
                            linkColor={() => "rgba(100, 116, 139, 0.25)"}
                            linkWidth={0.5}
                            linkLabel={(l: any) => l.label}
                            linkDirectionalParticles={2}
                            linkDirectionalParticleWidth={1.1}
                            linkDirectionalParticleSpeed={0.005}
                            linkDirectionalParticleColor={() => "#94a3b8"}
                            onNodeClick={onNodeClick}
                            onBackgroundClick={() => setSelected(null)}
                            enableNodeDrag={false}
                            cooldownTicks={120}
                        />
                    )}
                    {/* Collapsed-chat re-open button (only when chat aside is hidden) */}
                    {!chatOpen && (
                        <button
                            onClick={() => setChatOpen(true)}
                            className="absolute top-3 right-3 z-20 flex items-center gap-1.5 rounded-md bg-indigo-600 text-white text-xs font-medium px-3 py-2 shadow hover:bg-indigo-700"
                        >
                            <PanelRightOpen className="w-4 h-4" /> Assistant
                        </button>
                    )}
                </div>

                {/* ── Medical Knowledge Assistant aside ──────────────────── */}
                {chatOpen && (
                    <aside className="w-[380px] shrink-0 border-l border-slate-200 bg-white flex flex-col">
                        <div className="px-3 py-2 border-b border-slate-100">
                            <div className="flex items-center gap-2">
                                <Sparkles className="w-4 h-4 text-indigo-600" />
                                <span className="text-sm font-semibold text-slate-800">
                                    Medical Knowledge Assistant
                                </span>
                                <button
                                    onClick={() => setChatOpen(false)}
                                    className="ml-auto text-slate-400 hover:text-slate-700"
                                    title="ย่อ"
                                >
                                    <PanelRightClose className="w-4 h-4" />
                                </button>
                            </div>
                            {topicLabel && (
                                <div className="mt-1.5 inline-flex items-center gap-1.5 rounded-full bg-indigo-50 px-2 py-0.5 text-[11px] text-indigo-700">
                                    <span className="text-slate-500">หัวข้อ:</span>
                                    <span className="font-medium">{topicLabel}</span>
                                    <button
                                        onClick={() => setSelected(null)}
                                        className="text-indigo-400 hover:text-indigo-600"
                                        title="ลบ topic"
                                    >
                                        <X className="w-3 h-3" />
                                    </button>
                                </div>
                            )}
                        </div>

                        {/* Messages */}
                        <div className="flex-1 overflow-y-auto p-3 space-y-3">
                            {chatTurns.length === 0 && !chatStreaming && (
                                <div className="text-[12px] text-slate-400 text-center mt-6">
                                    ถามเรื่องความสัมพันธ์ของโรค, ยา, อาการ ได้เลย
                                    <br />
                                    คลิก entity ในกราฟเพื่อตั้งหัวข้อ
                                </div>
                            )}
                            {chatTurns.map((turn, idx) => (
                                <div
                                    key={`${turn.turn}-${idx}`}
                                    className={
                                        turn.role === "user"
                                            ? "ml-6 rounded-lg bg-indigo-50 border border-indigo-100 px-3 py-2 text-sm text-slate-800"
                                            : turn.role === "assistant"
                                              ? "rounded-lg bg-slate-50 border border-slate-200 px-3 py-2 text-sm text-slate-800 whitespace-pre-wrap"
                                              : "rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-xs text-red-700"
                                    }
                                >
                                    {turn.content}
                                </div>
                            ))}
                            {chatStreaming && (
                                <div className="rounded-lg bg-slate-50 border border-slate-200 px-3 py-2 text-sm text-slate-500 flex items-center gap-2">
                                    <Loader2 className="w-3 h-3 animate-spin" />
                                    {chatStatus === "consulting"
                                        ? "กำลังถาม PrimeKG agent…"
                                        : "รอ Bifrost…"}
                                </div>
                            )}
                        </div>

                        {/* Input */}
                        <form
                            onSubmit={(e) => {
                                e.preventDefault();
                                sendQuestion();
                            }}
                            className="border-t border-slate-100 p-2 flex items-center gap-2"
                        >
                            <Input
                                value={chatInput}
                                onChange={(e) => setChatInput(e.target.value)}
                                placeholder="ถามเรื่องความสัมพันธ์ของโรค..."
                                className="flex-1 text-sm"
                                disabled={chatStreaming}
                            />
                            <button
                                type="submit"
                                disabled={!chatInput.trim() || chatStreaming}
                                className="rounded-md bg-indigo-600 text-white px-3 py-2 hover:bg-indigo-700 disabled:opacity-40 disabled:cursor-not-allowed"
                                aria-label="Send"
                            >
                                <Send className="w-4 h-4" />
                            </button>
                        </form>

                        {/* Audit footer — explains the agent's data source */}
                        <div className="border-t border-slate-100 px-3 py-1.5 text-[10px] text-slate-400 flex items-center gap-1.5">
                            📊 หลักฐานจากกราฟ (PrimeKG) · agent id=7 · ตรวจสอบได้
                        </div>
                    </aside>
                )}
                </div>
            </CardContent>
        </Card>
    );
}
