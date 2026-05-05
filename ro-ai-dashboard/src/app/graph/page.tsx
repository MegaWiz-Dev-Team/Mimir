"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
    fetchGraphStats,
    fetchGraphVisualization,
    searchGraphEntities,
    fetchEntityNeighbors,
    findGraphPaths,
    fetchExtractionRuns,
    GraphStats,
    GraphEntity,
    VisualizationNode,
    VisualizationEdge,
    ExtractionRun,
} from "@/lib/api";
import {
    Search,
    RefreshCw,
    Filter,
    Info,
    X,
    Share2,
    Database,
    GitBranch,
    Circle,
    ArrowRight,
    Loader2,
} from "lucide-react";

// Entity type color map (matches backend)
const ENTITY_COLORS: Record<string, string> = {
    // Tenant entity types
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
    // PrimeKG global types
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

const isPrimeKG = (entity: { tenant_id?: string | null; source_id?: number }) =>
    entity.tenant_id === null || entity.tenant_id === "" || entity.source_id === undefined;

export default function GraphPage() {
    const [stats, setStats] = useState<GraphStats | null>(null);
    const [nodes, setNodes] = useState<VisualizationNode[]>([]);
    const [edges, setEdges] = useState<VisualizationEdge[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [extractionRuns, setExtractionRuns] = useState<ExtractionRun[]>([]);

    // Search & filter
    const [searchQuery, setSearchQuery] = useState("");
    const [searchResults, setSearchResults] = useState<GraphEntity[]>([]);
    const [filterType, setFilterType] = useState<string>("");
    const [nodeLimit, setNodeLimit] = useState(200);

    // Selected entity detail
    const [selectedNode, setSelectedNode] = useState<VisualizationNode | null>(null);
    const [selectedNeighbors, setSelectedNeighbors] = useState<{ nodes: VisualizationNode[]; edges: VisualizationEdge[] } | null>(null);

    // Path finding
    const [pathFrom, setPathFrom] = useState("");
    const [pathTo, setPathTo] = useState("");
    const [pathResult, setPathResult] = useState<any>(null);

    // Canvas ref for simple graph rendering
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const [nodePositions, setNodePositions] = useState<Map<string, { x: number; y: number }>>(new Map());

    // Load data
    const loadData = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const [statsData, vizData, runsData] = await Promise.all([
                fetchGraphStats(),
                fetchGraphVisualization({ limit: nodeLimit, type: filterType || undefined }),
                fetchExtractionRuns().catch(() => ({ runs: [] })), // Graceful degradation if endpoint missing
            ]);
            setStats(statsData);
            setNodes(vizData.nodes);
            setEdges(vizData.edges);
            setExtractionRuns(runsData.runs);
        } catch (err: any) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    }, [nodeLimit, filterType]);

    useEffect(() => {
        loadData();
    }, [loadData]);

    // Simple force-directed layout calculation
    useEffect(() => {
        if (nodes.length === 0) return;

        const positions = new Map<string, { x: number; y: number }>();
        const width = 800;
        const height = 600;
        const centerX = width / 2;
        const centerY = height / 2;

        nodes.forEach((node, i) => {
            const angle = (2 * Math.PI * i) / nodes.length;
            const radius = Math.min(width, height) * 0.35;
            positions.set(node.id, {
                x: centerX + radius * Math.cos(angle),
                y: centerY + radius * Math.sin(angle),
            });
        });

        for (let iter = 0; iter < 50; iter++) {
            const nodeArr = Array.from(positions.entries());
            for (let i = 0; i < nodeArr.length; i++) {
                for (let j = i + 1; j < nodeArr.length; j++) {
                    const [, posA] = nodeArr[i];
                    const [, posB] = nodeArr[j];
                    const dx = posA.x - posB.x;
                    const dy = posA.y - posB.y;
                    const dist = Math.sqrt(dx * dx + dy * dy) || 1;
                    const force = 5000 / (dist * dist);
                    const fx = (dx / dist) * force;
                    const fy = (dy / dist) * force;
                    posA.x += fx; posA.y += fy;
                    posB.x -= fx; posB.y -= fy;
                }
            }
            for (const edge of edges) {
                const posA = positions.get(edge.source);
                const posB = positions.get(edge.target);
                if (!posA || !posB) continue;
                const dx = posB.x - posA.x;
                const dy = posB.y - posA.y;
                const dist = Math.sqrt(dx * dx + dy * dy) || 1;
                const force = dist * 0.01;
                posA.x += (dx / dist) * force; posA.y += (dy / dist) * force;
                posB.x -= (dx / dist) * force; posB.y -= (dy / dist) * force;
            }
            for (const [, pos] of positions) {
                pos.x += (centerX - pos.x) * 0.01;
                pos.y += (centerY - pos.y) * 0.01;
                pos.x = Math.max(40, Math.min(width - 40, pos.x));
                pos.y = Math.max(40, Math.min(height - 40, pos.y));
            }
        }
        setNodePositions(positions);
    }, [nodes, edges]);

    // Draw on canvas
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas || nodes.length === 0) return;

        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        canvas.width = canvas.offsetWidth * dpr;
        canvas.height = canvas.offsetHeight * dpr;
        ctx.scale(dpr, dpr);

        const w = canvas.offsetWidth;
        const h = canvas.offsetHeight;

        // Detect dark mode via CSS
        const isDark = document.documentElement.classList.contains("dark");
        ctx.fillStyle = isDark ? "#09090b" : "#ffffff";
        ctx.fillRect(0, 0, w, h);

        const scaleX = w / 800;
        const scaleY = h / 600;

        // Draw edges
        ctx.strokeStyle = isDark ? "rgba(113, 113, 122, 0.3)" : "rgba(161, 161, 170, 0.4)";
        ctx.lineWidth = 1;
        for (const edge of edges) {
            const from = nodePositions.get(edge.source);
            const to = nodePositions.get(edge.target);
            if (!from || !to) continue;
            ctx.beginPath();
            ctx.moveTo(from.x * scaleX, from.y * scaleY);
            ctx.lineTo(to.x * scaleX, to.y * scaleY);
            ctx.stroke();
        }

        // Draw nodes
        for (const node of nodes) {
            const pos = nodePositions.get(node.id);
            if (!pos) continue;
            const x = pos.x * scaleX;
            const y = pos.y * scaleY;
            const r = (node.size || 6) * 0.8;

            // Glow effect
            const gradient = ctx.createRadialGradient(x, y, 0, x, y, r * 3);
            gradient.addColorStop(0, node.color + "30");
            gradient.addColorStop(1, "transparent");
            ctx.fillStyle = gradient;
            ctx.beginPath();
            ctx.arc(x, y, r * 3, 0, Math.PI * 2);
            ctx.fill();

            // Node circle
            ctx.fillStyle = node.color;
            ctx.beginPath();
            ctx.arc(x, y, r, 0, Math.PI * 2);
            ctx.fill();

            // Selected highlight
            if (selectedNode?.id === node.id) {
                ctx.strokeStyle = isDark ? "#fff" : "#000";
                ctx.lineWidth = 2;
                ctx.beginPath();
                ctx.arc(x, y, r + 3, 0, Math.PI * 2);
                ctx.stroke();
            }

            // Label
            ctx.fillStyle = isDark ? "#d4d4d8" : "#3f3f46";
            ctx.font = "10px Inter, system-ui, sans-serif";
            ctx.textAlign = "center";
            ctx.fillText(node.label.length > 15 ? node.label.slice(0, 14) + "…" : node.label, x, y + r + 14);
        }
    }, [nodes, edges, nodePositions, selectedNode]);

    // Handle canvas click
    const handleCanvasClick = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const rect = canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        const scaleX = canvas.offsetWidth / 800;
        const scaleY = canvas.offsetHeight / 600;

        let clicked: VisualizationNode | null = null;
        for (const node of nodes) {
            const pos = nodePositions.get(node.id);
            if (!pos) continue;
            const dist = Math.sqrt((x - pos.x * scaleX) ** 2 + (y - pos.y * scaleY) ** 2);
            if (dist < (node.size || 6) * 1.5) { clicked = node; break; }
        }

        setSelectedNode(clicked);
        if (clicked) {
            fetchEntityNeighbors(clicked.id, 1)
                .then((data) => setSelectedNeighbors({ nodes: data.nodes, edges: data.edges }))
                .catch(() => setSelectedNeighbors(null));
        } else {
            setSelectedNeighbors(null);
        }
    }, [nodes, nodePositions]);

    // Search handler
    const handleSearch = useCallback(async () => {
        if (!searchQuery.trim()) { setSearchResults([]); return; }
        try {
            const result = await searchGraphEntities({ q: searchQuery, limit: 20 });
            setSearchResults(result.entities);
        } catch { setSearchResults([]); }
    }, [searchQuery]);

    // Path finding
    const handleFindPath = useCallback(async () => {
        if (!pathFrom.trim() || !pathTo.trim()) return;
        try {
            const result = await findGraphPaths(pathFrom, pathTo);
            setPathResult(result);
        } catch (err: any) {
            setPathResult({ found: false, paths: [], message: err.message });
        }
    }, [pathFrom, pathTo]);

    return (
        <div className="container mx-auto px-4 py-8 space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
                        <Share2 className="w-6 h-6 text-purple-500" />
                        Knowledge Graph
                    </h1>
                    <p className="text-sm text-muted-foreground mt-1">
                        Entity relationships &amp; connections
                    </p>
                </div>
                <Button variant="outline" onClick={loadData} disabled={loading}>
                    <RefreshCw className={`w-4 h-4 mr-2 ${loading ? "animate-spin" : ""}`} />
                    Refresh
                </Button>
            </div>

            {error && (
                <div className="p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 flex items-center gap-2 text-sm">
                    <Info className="w-5 h-5 flex-shrink-0" />
                    <span>{error}</span>
                </div>
            )}

            <div className="grid grid-cols-12 gap-6">
                {/* Left Sidebar — Stats & Search */}
                <div className="col-span-12 lg:col-span-3 space-y-4">
                    {/* Stats */}
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Database className="w-4 h-4" /> Graph Statistics
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            {stats ? (
                                <div className="space-y-3">
                                    <div className="grid grid-cols-2 gap-3">
                                        <div className="p-3 rounded-lg bg-purple-50 dark:bg-purple-950/30">
                                            <div className="text-2xl font-bold text-purple-600 dark:text-purple-400">{stats.total_entities}</div>
                                            <div className="text-xs text-muted-foreground">Entities</div>
                                        </div>
                                        <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-950/30">
                                            <div className="text-2xl font-bold text-blue-600 dark:text-blue-400">{stats.total_relations}</div>
                                            <div className="text-xs text-muted-foreground">Relations</div>
                                        </div>
                                    </div>

                                    {stats.entities_by_type.length > 0 && (
                                        <div>
                                            <div className="text-xs text-muted-foreground mb-2">Entity Types</div>
                                            {stats.entities_by_type.map((t) => (
                                                <div key={t.type} className="flex items-center justify-between py-1">
                                                    <div className="flex items-center gap-2">
                                                        <div className="w-2.5 h-2.5 rounded-full" style={{ background: ENTITY_COLORS[t.type] || "#95A5A6" }} />
                                                        <span className="text-xs">{t.type}</span>
                                                    </div>
                                                    <span className="text-xs text-muted-foreground">{t.count}</span>
                                                </div>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            ) : (
                                <div className="text-center py-6 text-muted-foreground text-sm">
                                    {loading ? <Loader2 className="w-5 h-5 animate-spin mx-auto" /> : "No data yet"}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* Entity Search */}
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Search className="w-4 h-4" /> Search Entities
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="flex gap-2">
                                <Input
                                    value={searchQuery}
                                    onChange={(e) => setSearchQuery(e.target.value)}
                                    onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                                    placeholder="Search by name..."
                                    className="text-sm"
                                />
                                <Button size="icon" onClick={handleSearch} className="shrink-0">
                                    <Search className="w-4 h-4" />
                                </Button>
                            </div>

                            {searchResults.length > 0 && (
                                <div className="mt-3 space-y-1 max-h-48 overflow-y-auto">
                                    {searchResults.map((entity) => (
                                        <div
                                            key={entity.id}
                                            className="flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-muted cursor-pointer transition-colors"
                                        >
                                            <div className="w-2 h-2 rounded-full shrink-0" style={{ background: entity.color || ENTITY_COLORS[entity.entity_type] || "#95A5A6" }} />
                                            <div className="flex-1 min-w-0">
                                                <div className="text-xs font-medium truncate">{entity.name}</div>
                                                <div className="text-xs text-muted-foreground">{entity.entity_type}</div>
                                            </div>
                                            {isPrimeKG(entity) && (
                                                <span className="shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300">
                                                    PrimeKG
                                                </span>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* Filters */}
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Filter className="w-4 h-4" /> Filters
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            <div>
                                <label className="text-xs text-muted-foreground">Entity Type</label>
                                <select
                                    value={filterType}
                                    onChange={(e) => setFilterType(e.target.value)}
                                    className="w-full mt-1 px-3 py-2 rounded-md border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                                >
                                    <option value="">All Types</option>
                                    {Object.keys(ENTITY_COLORS).map((type) => (
                                        <option key={type} value={type}>{type}</option>
                                    ))}
                                </select>
                            </div>
                            <div>
                                <label className="text-xs text-muted-foreground">Max Nodes: {nodeLimit}</label>
                                <input
                                    type="range"
                                    min={50} max={500} step={50}
                                    value={nodeLimit}
                                    onChange={(e) => setNodeLimit(Number(e.target.value))}
                                    className="w-full mt-1 accent-purple-500"
                                />
                            </div>
                        </CardContent>
                    </Card>

                    {/* Path Finding */}
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <GitBranch className="w-4 h-4" /> Find Path
                            </CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-2">
                            <Input
                                value={pathFrom}
                                onChange={(e) => setPathFrom(e.target.value)}
                                placeholder="From entity..."
                                className="text-sm"
                            />
                            <Input
                                value={pathTo}
                                onChange={(e) => setPathTo(e.target.value)}
                                placeholder="To entity..."
                                className="text-sm"
                            />
                            <Button onClick={handleFindPath} className="w-full" variant="default">
                                Find Path
                            </Button>

                            {pathResult && (
                                <div className="mt-2 p-3 rounded-lg bg-muted text-xs">
                                    {pathResult.found ? (
                                        <div>
                                            <div className="text-green-600 dark:text-green-400 font-medium mb-1">Path found!</div>
                                            {pathResult.paths.map((path: any, i: number) => (
                                                <div key={i} className="flex flex-wrap items-center gap-1 mt-1">
                                                    {path.steps.map((step: any, j: number) => (
                                                        <div key={j} className="flex items-center gap-1">
                                                            {j === 0 && <span className="text-purple-600 dark:text-purple-400 font-medium">{step.from}</span>}
                                                            <ArrowRight className="w-3 h-3 text-muted-foreground" />
                                                            <span className="text-muted-foreground italic">{step.relation_type}</span>
                                                            <ArrowRight className="w-3 h-3 text-muted-foreground" />
                                                            <span className="text-purple-600 dark:text-purple-400 font-medium">{step.to}</span>
                                                        </div>
                                                    ))}
                                                </div>
                                            ))}
                                        </div>
                                    ) : (
                                        <div className="text-amber-600 dark:text-amber-400">{pathResult.message || "No path found"}</div>
                                    )}
                                </div>
                            )}
                        </CardContent>
                    </Card>
                </div>

                {/* Main Graph Canvas */}
                <div className="col-span-12 lg:col-span-6">
                    <Card className="overflow-hidden">
                        <CardHeader className="pb-2">
                            <div className="flex items-center justify-between">
                                <CardTitle className="text-sm text-muted-foreground font-normal">
                                    {nodes.length} nodes · {edges.length} edges
                                </CardTitle>
                                <div className="flex items-center gap-3 flex-wrap">
                                    {Object.entries(ENTITY_COLORS).slice(0, 5).map(([type, color]) => (
                                        <div key={type} className="flex items-center gap-1">
                                            <div className="w-2 h-2 rounded-full" style={{ background: color }} />
                                            <span className="text-xs text-muted-foreground">{type}</span>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </CardHeader>
                        <CardContent className="p-0">
                            {nodes.length === 0 && !loading ? (
                                <div className="flex flex-col items-center justify-center py-24 text-muted-foreground">
                                    <Share2 className="w-16 h-16 mb-4 opacity-20" />
                                    <h3 className="text-lg font-medium mb-2">No Knowledge Graph Data</h3>
                                    <p className="text-sm text-center max-w-md mb-6">
                                        Your knowledge graph is currently empty. To populate it, you need to trigger entity extraction from your data sources.
                                    </p>
                                    <Button onClick={() => window.location.href = '/sources'} variant="default">
                                        Go to Sources to Extract
                                    </Button>
                                </div>
                            ) : loading ? (
                                <div className="flex items-center justify-center py-24">
                                    <Loader2 className="w-8 h-8 animate-spin text-purple-500" />
                                </div>
                            ) : (
                                <canvas
                                    ref={canvasRef}
                                    onClick={handleCanvasClick}
                                    className="w-full cursor-crosshair border-t"
                                    style={{ height: "600px" }}
                                />
                            )}
                        </CardContent>
                    </Card>
                </div>

                {/* Right Sidebar — Entity Details */}
                <div className="col-span-12 lg:col-span-3 space-y-4">
                    {selectedNode ? (
                        <Card>
                            <CardHeader className="pb-3">
                                <div className="flex items-center justify-between">
                                    <CardTitle className="text-sm flex items-center gap-2">
                                        <Info className="w-4 h-4" /> Entity Details
                                    </CardTitle>
                                    <button onClick={() => { setSelectedNode(null); setSelectedNeighbors(null); }} className="text-muted-foreground hover:text-foreground">
                                        <X className="w-4 h-4" />
                                    </button>
                                </div>
                            </CardHeader>
                            <CardContent className="space-y-3">
                                <div className="flex items-center gap-3">
                                    <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: selectedNode.color + "20" }}>
                                        <Circle className="w-5 h-5" style={{ color: selectedNode.color }} />
                                    </div>
                                    <div>
                                        <div className="font-medium">{selectedNode.label}</div>
                                        <div className="text-xs text-muted-foreground">{selectedNode.entity_type}</div>
                                    </div>
                                </div>

                                {selectedNeighbors && selectedNeighbors.nodes.length > 0 && (
                                    <div>
                                        <div className="text-xs text-muted-foreground mb-2">Connected ({selectedNeighbors.nodes.length - 1} neighbors)</div>
                                        <div className="space-y-1 max-h-48 overflow-y-auto">
                                            {selectedNeighbors.nodes.filter(n => n.id !== selectedNode.id).map((n) => (
                                                <div key={n.id} className="flex items-center gap-2 px-2 py-1.5 rounded-lg bg-muted">
                                                    <div className="w-2 h-2 rounded-full" style={{ background: n.color }} />
                                                    <span className="text-xs truncate">{n.label}</span>
                                                    <span className="text-xs text-muted-foreground ml-auto">{n.entity_type}</span>
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                )}

                                {selectedNeighbors && selectedNeighbors.edges.length > 0 && (
                                    <div>
                                        <div className="text-xs text-muted-foreground mb-2">Relations ({selectedNeighbors.edges.length})</div>
                                        <div className="space-y-1 max-h-32 overflow-y-auto">
                                            {selectedNeighbors.edges.map((e) => (
                                                <div key={e.id} className="text-xs text-muted-foreground px-2 py-1 rounded bg-muted">
                                                    {e.label}
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                )}
                            </CardContent>
                        </Card>
                    ) : (
                        <Card>
                            <CardContent className="pt-6">
                                <div className="text-center py-8 text-muted-foreground text-sm">
                                    <Info className="w-8 h-8 mx-auto mb-3 opacity-30" />
                                    <p>Click a node in the graph to see its details and connections</p>
                                </div>
                            </CardContent>
                        </Card>
                    )}

                    {/* Entity Type Legend */}
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
                                        className={`flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs transition-colors ${filterType === type
                                                ? "bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 ring-1 ring-purple-300 dark:ring-purple-700"
                                                : "bg-muted hover:bg-muted/80"
                                            }`}
                                    >
                                        <div className="w-2.5 h-2.5 rounded-full flex-shrink-0" style={{ background: color }} />
                                        {type}
                                    </button>
                                ))}
                            </div>
                        </CardContent>
                    </Card>
                    {/* Extraction Runs History */}
                    {extractionRuns.length > 0 && (
                        <Card className="mt-6">
                            <CardHeader className="pb-3">
                                <CardTitle className="text-sm flex items-center gap-2">
                                    <Database className="w-4 h-4" /> Extraction History
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="overflow-x-auto">
                                    <table className="w-full text-sm text-left whitespace-nowrap">
                                        <thead className="text-xs text-muted-foreground bg-muted/50 uppercase">
                                            <tr>
                                                <th className="px-4 py-2 rounded-tl-lg">ID</th>
                                                <th className="px-4 py-2">Source ID</th>
                                                <th className="px-4 py-2">Status</th>
                                                <th className="px-4 py-2">Entities</th>
                                                <th className="px-4 py-2">Relations</th>
                                                <th className="px-4 py-2 rounded-tr-lg">Started</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {extractionRuns.map((run) => (
                                                <tr key={run.id} className="border-b last:border-0 hover:bg-muted/50 border-gray-100 dark:border-zinc-800">
                                                    <td className="px-4 py-2 font-medium">#{run.id}</td>
                                                    <td className="px-4 py-2 text-muted-foreground">{run.source_id}</td>
                                                    <td className="px-4 py-2">
                                                        <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${
                                                            run.status === "completed" ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" :
                                                            run.status === "failed" ? "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400" :
                                                            "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                                                        }`}>
                                                            {run.status} {run.status === "running" && <Loader2 className="w-3 h-3 inline ml-1 animate-spin" />}
                                                        </span>
                                                    </td>
                                                    <td className="px-4 py-2 text-purple-600 dark:text-purple-400">+{run.entities_found}</td>
                                                    <td className="px-4 py-2 text-blue-600 dark:text-blue-400">+{run.relations_found}</td>
                                                    <td className="px-4 py-2 text-muted-foreground text-xs">
                                                        {new Date(run.started_at).toLocaleString()}
                                                    </td>
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            </CardContent>
                        </Card>
                    )}
                </div>
            </div>
        </div>
    );
}
