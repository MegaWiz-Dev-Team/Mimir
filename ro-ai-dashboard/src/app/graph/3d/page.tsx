"use client";

import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import dynamic from "next/dynamic";
import Link from "next/link";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
    fetchGraphStats,
    fetchGraphVisualization,
    fetchEntityNeighbors,
    fetchMyTenants,
    GraphStats,
    Tenant,
    VisualizationNode,
    VisualizationEdge,
} from "@/lib/api";
import {
    RefreshCw,
    Info,
    X,
    Share2,
    Database,
    Circle,
    Sparkles,
    Loader2,
    ArrowLeft,
    Eye,
    EyeOff,
    Layers,
    Building2,
} from "lucide-react";

const ForceGraph3D = dynamic(() => import("react-force-graph-3d"), {
    ssr: false,
    loading: () => (
        <div className="flex items-center justify-center h-full">
            <Loader2 className="w-8 h-8 animate-spin text-purple-500" />
        </div>
    ),
});

const ENTITY_COLORS: Record<string, string> = {
    Person: "#4A90D9",
    Organization: "#27AE60",
    Location: "#E67E22",
    Concept: "#9B59B6",
    Event: "#E74C3C",
    Product: "#1ABC9C",
    Drug: "#F39C12",
    Symptom: "#E91E63",
    Item: "#00BCD4",
    Monster: "#795548",
    Disease: "#C0392B",
    GeneProtein: "#2980B9",
    BiologicalProcess: "#27AE60",
    Pathway: "#16A085",
    Anatomy: "#8E44AD",
    MolecularFunction: "#D4AC0D",
    CellularComponent: "#CA6F1E",
    EffectPhenotype: "#CB4335",
    Exposure: "#7F8C8D",
    Other: "#95A5A6",
};

type GraphData = {
    nodes: (VisualizationNode & { val?: number; degree?: number })[];
    links: (VisualizationEdge & { source: string; target: string })[];
};

export default function Graph3DPage() {
    const fgRef = useRef<any>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const [stats, setStats] = useState<GraphStats | null>(null);
    const [rawNodes, setRawNodes] = useState<VisualizationNode[]>([]);
    const [rawEdges, setRawEdges] = useState<VisualizationEdge[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const [nodeLimit, setNodeLimit] = useState(300);
    const [filterType, setFilterType] = useState<string>("");
    const [bloomEnabled, setBloomEnabled] = useState(true);
    const [particlesEnabled, setParticlesEnabled] = useState(true);
    const [showLabels, setShowLabels] = useState(true);
    const [includePrimekg, setIncludePrimekg] = useState(false);

    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [selectedTenant, setSelectedTenant] = useState<string>("");

    const [selectedNode, setSelectedNode] = useState<VisualizationNode | null>(null);
    const [highlightNodes, setHighlightNodes] = useState<Set<string>>(new Set());
    const [highlightLinks, setHighlightLinks] = useState<Set<string>>(new Set());

    const [dim, setDim] = useState({ w: 800, h: 600 });
    const [isDark, setIsDark] = useState(false);

    useEffect(() => {
        const onResize = () => {
            if (containerRef.current) {
                setDim({
                    w: containerRef.current.clientWidth,
                    h: containerRef.current.clientHeight,
                });
            }
        };
        onResize();
        window.addEventListener("resize", onResize);
        return () => window.removeEventListener("resize", onResize);
    }, []);

    useEffect(() => {
        const root = document.documentElement;
        const update = () => setIsDark(root.classList.contains("dark"));
        update();
        const obs = new MutationObserver(update);
        obs.observe(root, { attributes: true, attributeFilter: ["class"] });
        return () => obs.disconnect();
    }, []);

    const sceneBg = isDark ? "#0f172a" : "#1e293b";
    const labelColor = isDark ? "#e5e7eb" : "#f1f5f9";
    const labelBg = isDark ? "rgba(15, 23, 42, 0.65)" : "rgba(30, 41, 59, 0.55)";

    useEffect(() => {
        fetchMyTenants()
            .then((list) => setTenants(list))
            .catch(() => setTenants([]));
    }, []);

    const loadData = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const tenantOverride = selectedTenant || undefined;
            const [statsData, vizData] = await Promise.all([
                fetchGraphStats({ tenantOverride }),
                fetchGraphVisualization({
                    limit: nodeLimit,
                    type: filterType || undefined,
                    includePrimekg,
                    tenantOverride,
                }),
            ]);
            setStats(statsData);
            setRawNodes(vizData.nodes);
            setRawEdges(vizData.edges);
        } catch (err: any) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    }, [nodeLimit, filterType, includePrimekg, selectedTenant]);

    useEffect(() => {
        loadData();
    }, [loadData]);

    const graphData: GraphData = useMemo(() => {
        const degree = new Map<string, number>();
        rawEdges.forEach((e) => {
            degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
            degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
        });
        return {
            nodes: rawNodes.map((n) => ({
                ...n,
                degree: degree.get(n.id) ?? 0,
                val: Math.max(2, (n.size ?? 6) * 0.6 + (degree.get(n.id) ?? 0) * 0.5),
                color: n.color || ENTITY_COLORS[n.entity_type] || "#95A5A6",
            })),
            links: rawEdges.map((e) => ({ ...e, source: e.source, target: e.target })),
        };
    }, [rawNodes, rawEdges]);

    useEffect(() => {
        if (!fgRef.current || loading || rawNodes.length === 0) return;
        let cancelled = false;
        (async () => {
            try {
                const { UnrealBloomPass } = await import(
                    "three/examples/jsm/postprocessing/UnrealBloomPass.js"
                );
                if (cancelled || !fgRef.current?.postProcessingComposer) return;
                const composer = fgRef.current.postProcessingComposer();
                composer.passes = composer.passes.filter(
                    (p: any) => p?.constructor?.name !== "UnrealBloomPass",
                );
                if (bloomEnabled) {
                    const bloom = new UnrealBloomPass(
                        undefined as any,
                        0.9,
                        0.8,
                        0.15,
                    );
                    composer.addPass(bloom);
                }
            } catch {
                /* bloom optional */
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [bloomEnabled, loading, rawNodes.length]);

    const handleNodeClick = useCallback(async (node: any) => {
        setSelectedNode(node);
        if (fgRef.current) {
            const distance = 80;
            const distRatio = 1 + distance / Math.hypot(node.x || 1, node.y || 1, node.z || 1);
            fgRef.current.cameraPosition(
                {
                    x: (node.x || 0) * distRatio,
                    y: (node.y || 0) * distRatio,
                    z: (node.z || 0) * distRatio,
                },
                node,
                1500,
            );
        }
        try {
            const data = await fetchEntityNeighbors(node.id, 1);
            const ids = new Set<string>([node.id, ...data.nodes.map((n) => n.id)]);
            const linkIds = new Set<string>(data.edges.map((e) => e.id));
            setHighlightNodes(ids);
            setHighlightLinks(linkIds);
        } catch {
            setHighlightNodes(new Set([node.id]));
            setHighlightLinks(new Set());
        }
    }, []);

    const handleBackgroundClick = useCallback(() => {
        setSelectedNode(null);
        setHighlightNodes(new Set());
        setHighlightLinks(new Set());
    }, []);

    const nodeThreeObject = useCallback(
        (node: any) => {
            const buildObj = async () => {
                const THREE = await import("three");
                const SpriteText = (await import("three-spritetext")).default;

                const group = new THREE.Group();
                const r = Math.max(2, node.val ?? 4);

                const sphere = new THREE.Mesh(
                    new THREE.SphereGeometry(r, 16, 16),
                    new THREE.MeshBasicMaterial({
                        color: node.color || "#95A5A6",
                        transparent: true,
                        opacity: 0.95,
                    }),
                );
                group.add(sphere);

                if ((node.degree ?? 0) >= 5) {
                    const halo = new THREE.Mesh(
                        new THREE.SphereGeometry(r * 1.6, 16, 16),
                        new THREE.MeshBasicMaterial({
                            color: node.color || "#95A5A6",
                            transparent: true,
                            opacity: 0.18,
                        }),
                    );
                    group.add(halo);
                }

                if (showLabels) {
                    const label = node.label?.length > 22 ? node.label.slice(0, 21) + "…" : node.label || "";
                    const sprite = new SpriteText(label) as any;
                    sprite.color = labelColor;
                    sprite.textHeight = 3;
                    sprite.backgroundColor = labelBg;
                    sprite.padding = 1.5;
                    sprite.borderRadius = 2;
                    sprite.position.set(0, r + 4, 0);
                    group.add(sprite);
                }
                return group;
            };

            if (!(node as any)._cachedObj) {
                (node as any)._cachedObj = null;
                buildObj().then((obj) => {
                    (node as any)._cachedObj = obj;
                    fgRef.current?.refresh?.();
                });
            }
            return (node as any)._cachedObj;
        },
        [showLabels],
    );

    useEffect(() => {
        graphData.nodes.forEach((n: any) => {
            n._cachedObj = null;
        });
        fgRef.current?.refresh?.();
    }, [showLabels, isDark, graphData.nodes]);

    const linkId = (l: any) =>
        (l as any).id ?? `${l.source?.id ?? l.source}-${l.target?.id ?? l.target}`;

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            <div className="flex items-center justify-between flex-wrap gap-3">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <Sparkles className="w-6 h-6 text-purple-500" />
                        Knowledge Graph
                        <span className="text-xs font-medium px-2 py-0.5 rounded-full bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300">
                            3D
                        </span>
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1">
                        Immersive WebGL view · drag to orbit · scroll to zoom
                    </p>
                </div>
                <div className="flex items-center gap-2">
                    <Link href="/graph">
                        <Button variant="outline" size="sm">
                            <ArrowLeft className="w-4 h-4 mr-2" /> 2D view
                        </Button>
                    </Link>
                    <Button variant="outline" size="sm" onClick={loadData} disabled={loading}>
                        <RefreshCw className={`w-4 h-4 mr-2 ${loading ? "animate-spin" : ""}`} />
                        Refresh
                    </Button>
                </div>
            </div>

            {error && (
                <div className="p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 flex items-center gap-2 text-sm">
                    <Info className="w-5 h-5 flex-shrink-0" />
                    <span>{error}</span>
                </div>
            )}

            <div className="grid grid-cols-12 gap-6">
                <div className="col-span-12 lg:col-span-3 space-y-4">
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Database className="w-4 h-4" /> Stats
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            {stats ? (
                                <div className="grid grid-cols-2 gap-3">
                                    <div className="p-3 rounded-lg bg-purple-50 dark:bg-purple-950/30">
                                        <div className="text-2xl font-bold text-purple-600 dark:text-purple-400">
                                            {stats.total_entities}
                                        </div>
                                        <div className="text-xs text-muted-foreground">Entities</div>
                                    </div>
                                    <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-950/30">
                                        <div className="text-2xl font-bold text-blue-600 dark:text-blue-400">
                                            {stats.total_relations}
                                        </div>
                                        <div className="text-xs text-muted-foreground">Relations</div>
                                    </div>
                                </div>
                            ) : (
                                <div className="text-center py-6 text-muted-foreground text-sm">
                                    {loading ? <Loader2 className="w-5 h-5 animate-spin mx-auto" /> : "No data"}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Layers className="w-4 h-4" /> Graph Scope
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            <div>
                                <label className="text-xs text-muted-foreground flex items-center gap-1.5">
                                    <Building2 className="w-3.5 h-3.5" /> Tenant
                                </label>
                                <select
                                    value={selectedTenant}
                                    onChange={(e) => setSelectedTenant(e.target.value)}
                                    className="w-full mt-1 px-3 py-2 rounded-md border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                                >
                                    <option value="">Current session</option>
                                    {tenants.map((t) => (
                                        <option key={t.id} value={t.id}>
                                            {t.name || t.id}
                                        </option>
                                    ))}
                                </select>
                                {tenants.length === 0 && (
                                    <div className="text-[10px] text-muted-foreground mt-1">
                                        No alternate tenants accessible
                                    </div>
                                )}
                            </div>
                            <label className="flex items-start justify-between gap-3 text-xs cursor-pointer">
                                <span className="flex flex-col">
                                    <span className="flex items-center gap-2 font-medium">
                                        <Sparkles className="w-3.5 h-3.5 text-blue-500" />
                                        Include PrimeKG
                                    </span>
                                    <span className="text-[10px] text-muted-foreground mt-0.5">
                                        Bridges tenant entities to biomedical KG via SAME_AS
                                    </span>
                                </span>
                                <input
                                    type="checkbox"
                                    checked={includePrimekg}
                                    onChange={(e) => setIncludePrimekg(e.target.checked)}
                                    className="mt-1 accent-purple-500"
                                />
                            </label>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm">Rendering</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            <label className="flex items-center justify-between text-xs cursor-pointer">
                                <span className="flex items-center gap-2">
                                    <Sparkles className="w-3.5 h-3.5" /> Bloom glow
                                </span>
                                <input
                                    type="checkbox"
                                    checked={bloomEnabled}
                                    onChange={(e) => setBloomEnabled(e.target.checked)}
                                    className="accent-purple-500"
                                />
                            </label>
                            <label className="flex items-center justify-between text-xs cursor-pointer">
                                <span className="flex items-center gap-2">
                                    <Circle className="w-3.5 h-3.5" /> Edge particles
                                </span>
                                <input
                                    type="checkbox"
                                    checked={particlesEnabled}
                                    onChange={(e) => setParticlesEnabled(e.target.checked)}
                                    className="accent-purple-500"
                                />
                            </label>
                            <label className="flex items-center justify-between text-xs cursor-pointer">
                                <span className="flex items-center gap-2">
                                    {showLabels ? <Eye className="w-3.5 h-3.5" /> : <EyeOff className="w-3.5 h-3.5" />}
                                    Labels
                                </span>
                                <input
                                    type="checkbox"
                                    checked={showLabels}
                                    onChange={(e) => setShowLabels(e.target.checked)}
                                    className="accent-purple-500"
                                />
                            </label>
                            <div>
                                <label className="text-xs text-muted-foreground">
                                    Max Nodes: {nodeLimit}
                                </label>
                                <input
                                    type="range"
                                    min={50}
                                    max={500}
                                    step={50}
                                    value={nodeLimit}
                                    onChange={(e) => setNodeLimit(Number(e.target.value))}
                                    className="w-full mt-1 accent-purple-500"
                                />
                            </div>
                            <div>
                                <label className="text-xs text-muted-foreground">Entity Type</label>
                                <select
                                    value={filterType}
                                    onChange={(e) => setFilterType(e.target.value)}
                                    className="w-full mt-1 px-3 py-2 rounded-md border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                                >
                                    <option value="">All Types</option>
                                    {Object.keys(ENTITY_COLORS).map((type) => (
                                        <option key={type} value={type}>
                                            {type}
                                        </option>
                                    ))}
                                </select>
                            </div>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm">Entity Types</CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="grid grid-cols-2 gap-2">
                                {Object.entries(ENTITY_COLORS).map(([type, color]) => (
                                    <button
                                        key={type}
                                        onClick={() => setFilterType(filterType === type ? "" : type)}
                                        className={`flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs transition-colors ${
                                            filterType === type
                                                ? "bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 ring-1 ring-purple-300 dark:ring-purple-700"
                                                : "bg-muted hover:bg-muted/80"
                                        }`}
                                    >
                                        <div
                                            className="w-2.5 h-2.5 rounded-full flex-shrink-0"
                                            style={{ background: color }}
                                        />
                                        {type}
                                    </button>
                                ))}
                            </div>
                        </CardContent>
                    </Card>
                </div>

                <div className="col-span-12 lg:col-span-6">
                    <Card className="overflow-hidden">
                        <CardHeader className="pb-2">
                            <CardTitle className="text-sm text-muted-foreground font-normal">
                                {graphData.nodes.length} nodes · {graphData.links.length} edges
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="p-0">
                            {graphData.nodes.length === 0 && !loading ? (
                                <div className="flex flex-col items-center justify-center py-24 text-muted-foreground">
                                    <Share2 className="w-16 h-16 mb-4 opacity-20" />
                                    <h3 className="text-lg font-medium mb-2">No Knowledge Graph Data</h3>
                                    <p className="text-sm text-center max-w-md mb-6">
                                        Trigger entity extraction from your data sources to populate the graph.
                                    </p>
                                    <Button onClick={() => (window.location.href = "/sources")} variant="default">
                                        Go to Sources
                                    </Button>
                                </div>
                            ) : (
                                <div
                                    ref={containerRef}
                                    className="w-full overflow-hidden rounded-b-lg ring-1 ring-inset ring-slate-800/40 dark:ring-slate-700/40"
                                    style={{ height: "640px", background: sceneBg }}
                                >
                                    {loading ? (
                                        <div className="flex items-center justify-center h-full">
                                            <Loader2 className="w-8 h-8 animate-spin text-purple-500" />
                                        </div>
                                    ) : (
                                        <ForceGraph3D
                                            ref={fgRef}
                                            graphData={graphData as any}
                                            width={dim.w}
                                            height={dim.h}
                                            backgroundColor={sceneBg}
                                            showNavInfo={false}
                                            nodeThreeObject={nodeThreeObject}
                                            nodeThreeObjectExtend={false}
                                            nodeLabel={(n: any) => `${n.label} · ${n.entity_type}`}
                                            nodeOpacity={1}
                                            linkColor={(l: any) =>
                                                highlightLinks.has(linkId(l))
                                                    ? "rgba(168, 85, 247, 0.9)"
                                                    : "rgba(113, 113, 122, 0.35)"
                                            }
                                            linkWidth={(l: any) => (highlightLinks.has(linkId(l)) ? 1.8 : 0.4)}
                                            linkDirectionalParticles={particlesEnabled ? 2 : 0}
                                            linkDirectionalParticleWidth={1.2}
                                            linkDirectionalParticleSpeed={0.005}
                                            linkDirectionalParticleColor={(l: any) =>
                                                highlightLinks.has(linkId(l)) ? "#c084fc" : "#7c7c8a"
                                            }
                                            onNodeClick={handleNodeClick}
                                            onBackgroundClick={handleBackgroundClick}
                                            enableNodeDrag={false}
                                            cooldownTicks={120}
                                        />
                                    )}
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </div>

                <div className="col-span-12 lg:col-span-3 space-y-4">
                    {selectedNode ? (
                        <Card>
                            <CardHeader className="pb-3">
                                <div className="flex items-center justify-between">
                                    <CardTitle className="text-sm flex items-center gap-2">
                                        <Info className="w-4 h-4" /> Entity
                                    </CardTitle>
                                    <button
                                        onClick={handleBackgroundClick}
                                        className="text-muted-foreground hover:text-foreground"
                                    >
                                        <X className="w-4 h-4" />
                                    </button>
                                </div>
                            </CardHeader>
                            <CardContent className="space-y-3">
                                <div className="flex items-center gap-3">
                                    <div
                                        className="w-10 h-10 rounded-xl flex items-center justify-center"
                                        style={{ background: (selectedNode.color || "#95A5A6") + "20" }}
                                    >
                                        <Circle
                                            className="w-5 h-5"
                                            style={{ color: selectedNode.color || "#95A5A6" }}
                                        />
                                    </div>
                                    <div className="min-w-0">
                                        <div className="font-medium truncate">{selectedNode.label}</div>
                                        <div className="text-xs text-muted-foreground">
                                            {selectedNode.entity_type}
                                        </div>
                                    </div>
                                </div>
                                <div className="text-xs text-muted-foreground">
                                    Neighbors highlighted in graph ({Math.max(0, highlightNodes.size - 1)})
                                </div>
                            </CardContent>
                        </Card>
                    ) : (
                        <Card>
                            <CardContent className="pt-6">
                                <div className="text-center py-8 text-muted-foreground text-sm">
                                    <Sparkles className="w-8 h-8 mx-auto mb-3 opacity-30" />
                                    <p>Click a node to focus the camera and highlight neighbors</p>
                                </div>
                            </CardContent>
                        </Card>
                    )}

                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm">Controls</CardTitle>
                        </CardHeader>
                        <CardContent className="text-xs text-muted-foreground space-y-1">
                            <div>· Left-drag: orbit camera</div>
                            <div>· Right-drag: pan</div>
                            <div>· Scroll: zoom</div>
                            <div>· Click node: focus + neighbors</div>
                            <div>· Click empty: reset</div>
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    );
}
