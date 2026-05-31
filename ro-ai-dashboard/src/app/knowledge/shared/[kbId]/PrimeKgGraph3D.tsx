"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import {
    searchPrimekgEntity,
    resolvePrimekgQuery,
    fetchPrimekgRelations,
    fetchPrimekgNeighbors,
    askPrimekgAssistantStream,
    type PrimekgEntity,
    type PrimekgRelation,
} from "@/lib/api";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Loader2, Search, X, Sparkles, Send, PanelRightClose, PanelRightOpen } from "lucide-react";

// Deterministic "Graph Evidence" shown alongside the LLM prose: the
// resolved topic + its first-hop relations, straight from PrimeKG.
type Evidence = {
    topic: string;
    topicType: string;
    topicIndex: number;
    relations: PrimekgRelation[];
};

type ChatTurn = {
    role: "user" | "assistant" | "system";
    content: string;
    turn: number;
    evidence?: Evidence;
};

// How each PrimeKG relation type is presented in the evidence card.
// Order = clinical priority (treatment & contraindications first, then
// presentation, then related diseases, then the rest). Safety-distinct
// styling: ⚠️ CONTRAINDICATION (rose) must never read like 💊 INDICATION.
// `cap` = how many chips to show before a "+N เพิ่ม" expander. Clinically
// salient groups (drugs/diseases/phenotypes) are small and shown in full;
// the high-volume research groups (genes/proteins, exposures, other) are
// capped so they don't bury the actionable relations.
type RelGroup = { label: string; icon: string; chip: string; order: number; cap: number };
const SHOW_ALL = 999;
const REL_GROUPS: Record<string, RelGroup> = {
    INDICATION: { label: "ยาที่ใช้รักษา", icon: "💊", chip: "bg-emerald-50 border-emerald-200 text-emerald-800 hover:bg-emerald-100", order: 0, cap: SHOW_ALL },
    "OFF-LABEL USE": { label: "ยา (off-label)", icon: "💊", chip: "bg-teal-50 border-teal-200 text-teal-800 hover:bg-teal-100", order: 1, cap: SHOW_ALL },
    CONTRAINDICATION: { label: "ยาที่ห้าม/ระวัง", icon: "⚠️", chip: "bg-rose-50 border-rose-200 text-rose-800 hover:bg-rose-100", order: 2, cap: SHOW_ALL },
    DISEASE_PHENOTYPE_POSITIVE: { label: "อาการ/ลักษณะที่พบ", icon: "🩺", chip: "bg-amber-50 border-amber-200 text-amber-800 hover:bg-amber-100", order: 3, cap: SHOW_ALL },
    DISEASE_PHENOTYPE_NEGATIVE: { label: "อาการที่มักไม่พบ", icon: "🚫", chip: "bg-slate-50 border-slate-200 text-slate-600 hover:bg-slate-100", order: 4, cap: SHOW_ALL },
    DISEASE_DISEASE: { label: "โรคที่เกี่ยวข้อง", icon: "🔗", chip: "bg-indigo-50 border-indigo-200 text-indigo-800 hover:bg-indigo-100", order: 5, cap: SHOW_ALL },
    DISEASE_PROTEIN: { label: "ยีน/โปรตีนที่เกี่ยวข้อง", icon: "🧬", chip: "bg-green-50 border-green-200 text-green-800 hover:bg-green-100", order: 6, cap: 8 },
    EXPOSURE_DISEASE: { label: "ปัจจัย/สารที่สัมพันธ์", icon: "🌫️", chip: "bg-purple-50 border-purple-200 text-purple-800 hover:bg-purple-100", order: 7, cap: 8 },
};
const REL_FALLBACK: RelGroup = { label: "ความสัมพันธ์อื่นๆ", icon: "🔬", chip: "bg-slate-50 border-slate-200 text-slate-700 hover:bg-slate-100", order: 9, cap: 8 };
const relGroup = (rel: string) => REL_GROUPS[rel] || REL_FALLBACK;

// Compact, grouped summary of the REAL PrimeKG relations, injected into the
// assistant prompt so the local LLM grounds its prose on the graph instead
// of punting to general knowledge ("ไม่พบข้อมูล… จากความรู้ทั่วไป…"). The
// Bifrost agent (id=7) doesn't reliably fire the PrimeKG MCP tools for local
// models, so we hand it the evidence directly. Capped per group to keep the
// prompt small.
function relationsContext(topic: string, rels: PrimekgRelation[]): string {
    const byRel = new Map<string, string[]>();
    for (const r of rels) {
        const arr = byRel.get(r.relation) || [];
        if (arr.length < 30) arr.push(r.name);
        byRel.set(r.relation, arr);
    }
    const lines = Array.from(byRel.entries())
        .sort((a, b) => relGroup(a[0]).order - relGroup(b[0]).order)
        .map(([rel, names]) => `- ${relGroup(rel).label} (${rel}): ${names.join(", ")}`);
    return (
        `ข้อมูลความสัมพันธ์จริงจากกราฟ PrimeKG สำหรับ "${topic}" ` +
        `(ตอบโดยอ้างอิงข้อมูลนี้เท่านั้น ห้ามตอบว่า "ไม่พบข้อมูล"):\n` +
        lines.join("\n")
    );
}

/**
 * Deterministic "Graph Evidence" card — the verifiable half of an answer.
 * Built straight from PrimeKG relations (NOT LLM text), so safety-critical
 * grouping (⚠️ contraindication vs 💊 indication) can't be garbled by
 * phrasing. Every related entity is a chip that jumps the 3D graph to it.
 */
function EvidenceCard({
    ev,
    onGo,
}: {
    ev: Evidence;
    onGo: (idx: number, name: string, type: string) => void;
}) {
    const groups = new Map<string, PrimekgRelation[]>();
    for (const r of ev.relations) {
        const arr = groups.get(r.relation) || [];
        arr.push(r);
        groups.set(r.relation, arr);
    }
    const ordered = Array.from(groups.entries()).sort(
        (a, b) => relGroup(a[0]).order - relGroup(b[0]).order,
    );
    const hasTreatment = groups.has("INDICATION") || groups.has("OFF-LABEL USE");
    const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
    const toggleGroup = (rel: string) =>
        setExpandedGroups((prev) => {
            const next = new Set(prev);
            if (next.has(rel)) next.delete(rel);
            else next.add(rel);
            return next;
        });

    return (
        <div className="rounded-lg border border-slate-200 bg-white overflow-hidden shadow-sm">
            <div className="flex items-center gap-2 px-3 py-2 bg-slate-50 border-b border-slate-100">
                <span
                    className="inline-block w-2.5 h-2.5 rounded-full shrink-0"
                    style={{ background: colorFor(ev.topicType) }}
                />
                <span className="text-sm font-semibold text-slate-800 truncate">{ev.topic}</span>
                <span className="ml-auto text-[10px] uppercase tracking-wide text-slate-400 shrink-0">
                    หลักฐานจากกราฟ
                </span>
            </div>
            <div className="p-2.5 space-y-2.5">
                {ordered.map(([rel, items]) => {
                    const g = relGroup(rel);
                    const isExpanded = expandedGroups.has(rel);
                    const shown = isExpanded ? items : items.slice(0, g.cap);
                    const hidden = items.length - shown.length;
                    return (
                        <div key={rel}>
                            <div className="flex items-center gap-1.5 mb-1 text-[11px] font-medium text-slate-500">
                                <span>{g.icon}</span>
                                <span>{g.label}</span>
                                <span className="text-slate-300">·</span>
                                <span className="text-slate-400">{items.length}</span>
                            </div>
                            <div className="flex flex-wrap gap-1.5">
                                {shown.map((r) => (
                                    <button
                                        key={r.entity_index}
                                        onClick={() => onGo(r.entity_index, r.name, r.type)}
                                        title={`ดู “${r.name}” ในกราฟ 3D`}
                                        className={`rounded-full border px-2 py-0.5 text-[11px] transition-colors ${g.chip}`}
                                    >
                                        {r.name}
                                    </button>
                                ))}
                                {(hidden > 0 || isExpanded) && (
                                    <button
                                        onClick={() => toggleGroup(rel)}
                                        className="rounded-full border border-dashed border-slate-300 px-2 py-0.5 text-[11px] text-slate-500 hover:bg-slate-100 transition-colors"
                                    >
                                        {isExpanded ? "ย่อ" : `+${hidden} เพิ่ม`}
                                    </button>
                                )}
                            </div>
                        </div>
                    );
                })}
                {!hasTreatment && (
                    <div className="rounded-md bg-amber-50 border border-amber-100 px-2 py-1.5 text-[10px] text-amber-700 leading-relaxed">
                        ℹ️ ไม่มี “ยาที่ใช้รักษา” ในกราฟ — ไม่ได้แปลว่ารักษาไม่ได้
                        (เช่น OSA รักษาด้วย CPAP/ปรับพฤติกรรมเป็นหลัก)
                    </div>
                )}
            </div>
            <div className="px-3 py-1.5 bg-slate-50 border-t border-slate-100 text-[10px] text-slate-400 leading-relaxed">
                👆 คลิกชิปเพื่อดูใน 3D · ข้อมูลจากกราฟวิจัย PrimeKG ไม่ใช่แนวทางเวชปฏิบัติ
            </div>
        </div>
    );
}

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
    // Relations of the currently-selected node, for the detail panel.
    const [selRelations, setSelRelations] = useState<PrimekgRelation[]>([]);
    const [selRelLoading, setSelRelLoading] = useState(false);
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

    // Pull a node's neighbours into the graph. Declared before
    // `sendQuestion` because that callback lists `expand` in its deps.
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

    const sendQuestion = useCallback(
        async (rawText?: string) => {
            const text = (rawText ?? chatInput).trim();
            if (!text || chatStreaming) return;
            const turn = ++chatTurnRef.current;
            setChatInput("");
            setChatTurns((prev) => [...prev, { role: "user", content: text, turn }]);
            setChatStreaming(true);
            setChatStatus("consulting");
            // Re-center the graph on whatever disease THIS question names
            // (e.g. OSA) and use it as the topic. A question that names its
            // own entity wins; a bare follow-up ("รักษายังไง") resolves
            // nothing → we keep the currently-selected node as the topic.
            let topic = topicLabel;
            let groundingCtx = "";
            try {
                const hit = await resolvePrimekgQuery(text);
                let evTopic = "";
                let evType = "";
                let evIndex = 0;
                let relations: PrimekgRelation[] = [];
                if (hit) {
                    // The question named its own disease (e.g. OSA) → re-center
                    // the graph and make it the topic.
                    topic = hit.name;
                    evTopic = hit.name;
                    evType = hit.type;
                    evIndex = hit.entity_index;
                    relations = hit.relations;
                    setSelected({
                        id: String(hit.entity_index),
                        label: hit.name,
                        type: hit.type,
                        color: colorFor(hit.type),
                        val: 8,
                    });
                    void expand(hit.entity_index, hit.name, hit.type);
                } else if (selected && Number.isFinite(Number(selected.id))) {
                    // Follow-up that names no disease ("ความสัมพันธ์กับยาอะไรบ้าง")
                    // → pull relations for the current topic (selected node).
                    evTopic = selected.label;
                    evType = selected.type;
                    evIndex = Number(selected.id);
                    relations = await fetchPrimekgRelations(evIndex);
                }
                if (relations.length > 0) {
                    // (a) Feed the real graph relations to the LLM so its prose
                    // is grounded, not generic.
                    groundingCtx = relationsContext(evTopic, relations);
                    // (b) Show the deterministic evidence card immediately —
                    // before (and independent of) the LLM prose.
                    setChatTurns((prev) => [
                        ...prev,
                        {
                            role: "assistant",
                            content: "",
                            turn,
                            evidence: {
                                topic: evTopic,
                                topicType: evType,
                                topicIndex: evIndex,
                                relations,
                            },
                        },
                    ]);
                }
            } catch {
                /* best-effort graph navigation — keep the current topic */
            }
            // Anchor the LLM answer on the resolved (or current) topic, and
            // hand it the real graph relations when we have them.
            const query = [topic ? `Topic: ${topic}` : "", groundingCtx, `Question: ${text}`]
                .filter(Boolean)
                .join("\n\n");
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
        [chatInput, chatStreaming, topicLabel, selected, expand],
    );

    // Load the selected node's relations for the detail panel (any node:
    // disease, drug, gene…). PrimeKG nodes carry no description, so their
    // relationships ARE the detail worth showing.
    const selectedId = selected?.id;
    useEffect(() => {
        const idx = Number(selectedId);
        if (!selectedId || !Number.isFinite(idx)) {
            setSelRelations([]);
            return;
        }
        let cancelled = false;
        setSelRelLoading(true);
        setSelRelations([]);
        (async () => {
            try {
                const rels = await fetchPrimekgRelations(idx);
                if (!cancelled) setSelRelations(rels);
            } catch {
                if (!cancelled) setSelRelations([]);
            } finally {
                if (!cancelled) setSelRelLoading(false);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [selectedId]);

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

    // Jump the graph to an entity referenced from the chat (evidence-card
    // chip). Select it, pull its neighbours, then fly the camera there once
    // the layout has placed the node.
    const goToEntity = useCallback(
        async (idx: number, name: string, type: string) => {
            setSelected({ id: String(idx), label: name, type, color: colorFor(type), val: 8 });
            await expand(idx, name, type);
            setTimeout(() => {
                const node: any = fgRef.current
                    ?.graphData?.()
                    ?.nodes?.find((n: any) => n.id === String(idx));
                if (node && typeof node.x === "number") focusNode(node);
            }, 450);
        },
        [expand, focusNode],
    );

    // Grouped relation counts for the selected-node detail panel.
    const selRelSummary = useMemo(() => {
        const m = new Map<string, number>();
        for (const r of selRelations) m.set(r.relation, (m.get(r.relation) || 0) + 1);
        return Array.from(m.entries()).sort(
            (a, b) => relGroup(a[0]).order - relGroup(b[0]).order,
        );
    }, [selRelations]);

    // "Ask about this node" — sends a grounded question to the chat about the
    // selected node, which re-centers the graph, shows its evidence card, and
    // grounds the LLM answer on the node's real relations.
    const askAboutSelected = useCallback(() => {
        if (!selected) return;
        setChatOpen(true);
        void sendQuestion(`${selected.label} เกี่ยวข้องกับอะไรบ้าง`);
    }, [selected, sendQuestion]);

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

                            {/* Relation summary — PrimeKG nodes have no
                                description, so their connections are the detail. */}
                            <div className="mt-2 border-t border-slate-100 pt-2">
                                {selRelLoading ? (
                                    <div className="flex items-center gap-1.5 text-[11px] text-slate-400">
                                        <Loader2 className="w-3 h-3 animate-spin" /> กำลังโหลดความสัมพันธ์…
                                    </div>
                                ) : selRelSummary.length > 0 ? (
                                    <>
                                        <div className="text-[10px] text-slate-400 mb-1">
                                            ความสัมพันธ์ในกราฟ ({selRelations.length})
                                        </div>
                                        <div className="flex flex-wrap gap-1">
                                            {selRelSummary.map(([rel, count]) => {
                                                const g = relGroup(rel);
                                                return (
                                                    <span
                                                        key={rel}
                                                        title={g.label}
                                                        className={`inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 text-[10px] ${g.chip}`}
                                                    >
                                                        <span>{g.icon}</span>
                                                        <span>{g.label}</span>
                                                        <span className="font-semibold">{count}</span>
                                                    </span>
                                                );
                                            })}
                                        </div>
                                        <button
                                            onClick={askAboutSelected}
                                            className="mt-2 w-full rounded-md bg-indigo-600 text-white text-[11px] font-medium px-2 py-1.5 hover:bg-indigo-700 flex items-center justify-center gap-1.5"
                                        >
                                            <Sparkles className="w-3 h-3" /> ดูรายละเอียด / ถามในแชต
                                        </button>
                                    </>
                                ) : (
                                    <p className="text-[11px] text-slate-500">
                                        คลิกโหนดในกราฟเพื่อดึงเพื่อนบ้านเพิ่ม
                                    </p>
                                )}
                            </div>
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
                            {chatTurns.map((turn, idx) => {
                                if (turn.evidence) {
                                    return (
                                        <EvidenceCard
                                            key={`${turn.turn}-${idx}`}
                                            ev={turn.evidence}
                                            onGo={goToEntity}
                                        />
                                    );
                                }
                                return (
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
                                );
                            })}
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
