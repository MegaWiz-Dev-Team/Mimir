"use client";

// Asgard Analytics — Spatial Map (ADR-024 P4). Plot a read-only SQL query that
// returns lng/lat columns onto a MapLibre map. Offline / basemap-light: no tile
// server — a blank background style + the data layers (the province .pmtiles
// basemap is a v0.2 add). Optional H3 density via the geo/h3 op (mimir-geo).

import { useEffect, useRef, useState } from "react";
import "maplibre-gl/dist/maplibre-gl.css";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Loader2, Play, AlertCircle, MapPin, Hexagon } from "lucide-react";

const TENANT = "asgard_analytics";
const TH_CENTER: [number, number] = [100.9925, 13.7563]; // Thailand-ish
const LNG_RE = /^(lng|lon|long|longitude|x)$/i;
const LAT_RE = /^(lat|latitude|y)$/i;

interface QueryResult { columns: { name: string; type: string }[]; rows: string[][] }

export default function SpatialMapPage() {
    const mapEl = useRef<HTMLDivElement>(null);
    const mapRef = useRef<any>(null);
    const [sql, setSql] = useState(
        "-- query must return longitude + latitude columns (lng/lat), optional value\nSELECT lng, lat, name FROM places LIMIT 500",
    );
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [info, setInfo] = useState<string | null>(null);
    const [h3, setH3] = useState<{ cell: string; count: number }[] | null>(null);
    const ptsRef = useRef<[number, number][]>([]); // [lng,lat] for H3 (sent as lat,lng)

    // init map once (dynamic import → no SSR window access)
    useEffect(() => {
        let map: any;
        (async () => {
            const maplibregl = (await import("maplibre-gl")).default;
            if (!mapEl.current || mapRef.current) return;
            map = new maplibregl.Map({
                container: mapEl.current,
                style: {
                    version: 8,
                    sources: {},
                    layers: [{ id: "bg", type: "background", paint: { "background-color": "#0b1020" } }],
                    glyphs: undefined as any,
                },
                center: TH_CENTER,
                zoom: 5,
                attributionControl: false,
            });
            map.addControl(new maplibregl.NavigationControl({ showCompass: false }), "top-right");
            map.on("load", () => {
                map.addSource("points", { type: "geojson", data: { type: "FeatureCollection", features: [] } });
                map.addLayer({
                    id: "pts",
                    type: "circle",
                    source: "points",
                    paint: {
                        "circle-radius": 5,
                        "circle-color": ["case", ["has", "v"],
                            ["interpolate", ["linear"], ["get", "v"], 0, "#3b82f6", 1, "#ef4444"], "#22d3ee"],
                        "circle-stroke-width": 1,
                        "circle-stroke-color": "#0b1020",
                        "circle-opacity": 0.85,
                    },
                });
            });
            mapRef.current = map;
        })();
        return () => { if (map) map.remove(); mapRef.current = null; };
    }, []);

    async function run() {
        setLoading(true); setError(null); setInfo(null); setH3(null);
        try {
            const resp = await fetch("/api/analytics/query", {
                method: "POST", headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ tenant_id: TENANT, sql }),
            });
            const data = (await resp.json()) as QueryResult & { error?: string };
            if (!resp.ok || data.error) throw new Error(data.error || `HTTP ${resp.status}`);
            const cols = data.columns.map((c) => c.name);
            const li = cols.findIndex((c) => LNG_RE.test(c));
            const ai = cols.findIndex((c) => LAT_RE.test(c));
            if (li < 0 || ai < 0) throw new Error("query must return a longitude (lng/lon/x) and latitude (lat/y) column");
            // optional value column = first numeric col that isn't lng/lat
            const vi = cols.findIndex((c, i) => i !== li && i !== ai && data.rows.some((r) => r[i] !== null && !isNaN(Number(r[i]))));
            const vals = vi >= 0 ? data.rows.map((r) => Number(r[vi])).filter((n) => !isNaN(n)) : [];
            const vmin = vals.length ? Math.min(...vals) : 0, vmax = vals.length ? Math.max(...vals) : 1;
            const norm = (x: number) => (vmax > vmin ? (x - vmin) / (vmax - vmin) : 0.5);
            const lnglat: [number, number][] = [];
            const features = data.rows
                .map((r) => {
                    const lng = Number(r[li]), lat = Number(r[ai]);
                    if (isNaN(lng) || isNaN(lat)) return null;
                    lnglat.push([lng, lat]);
                    const props: any = {};
                    if (vi >= 0 && !isNaN(Number(r[vi]))) props.v = norm(Number(r[vi]));
                    return { type: "Feature", geometry: { type: "Point", coordinates: [lng, lat] }, properties: props };
                })
                .filter(Boolean);
            ptsRef.current = lnglat;
            const map = mapRef.current;
            const src = map?.getSource("points");
            if (src) src.setData({ type: "FeatureCollection", features });
            if (lnglat.length && map) {
                const b = lnglat.reduce(
                    (acc, [lng, lat]) => [Math.min(acc[0], lng), Math.min(acc[1], lat), Math.max(acc[2], lng), Math.max(acc[3], lat)],
                    [180, 90, -180, -90],
                );
                map.fitBounds([[b[0], b[1]], [b[2], b[3]]], { padding: 50, maxZoom: 12, duration: 600 });
            }
            setInfo(`${features.length} point(s) plotted${vi >= 0 ? ` · coloured by "${cols[vi]}"` : ""}`);
        } catch (e: any) {
            setError(String(e.message || e));
        } finally {
            setLoading(false);
        }
    }

    async function h3density() {
        if (!ptsRef.current.length) { setError("run a query first"); return; }
        setError(null);
        try {
            const points = ptsRef.current.map(([lng, lat]) => [lat, lng]); // mimir-geo wants (lat,lng)
            const resp = await fetch("/api/analytics/geo", {
                method: "POST", headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ op: "geo/h3", points, resolution: 7 }),
            });
            const data = await resp.json();
            if (!resp.ok || data.error) throw new Error(data.error || `HTTP ${resp.status}`);
            setH3(data.cells || []);
        } catch (e: any) { setError(String(e.message || e)); }
    }

    return (
        <div className="p-6 space-y-4">
            <div>
                <h1 className="text-2xl font-semibold flex items-center gap-2"><MapPin className="h-6 w-6" /> Spatial Map</h1>
                <p className="text-sm text-muted-foreground">Plot a SQL query (lng/lat) on the map. Offline basemap-light (mimir-geo).</p>
            </div>
            <Card>
                <CardHeader><CardTitle className="text-base">Query</CardTitle></CardHeader>
                <CardContent className="space-y-3">
                    <textarea
                        className="w-full h-24 font-mono text-xs rounded-md border bg-background p-2"
                        value={sql} onChange={(e) => setSql(e.target.value)}
                    />
                    <div className="flex items-center gap-2">
                        <Button onClick={run} disabled={loading}>
                            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />} Plot
                        </Button>
                        <Button variant="outline" onClick={h3density} disabled={loading}>
                            <Hexagon className="h-4 w-4" /> H3 density
                        </Button>
                        {info && <span className="text-xs text-muted-foreground">{info}</span>}
                    </div>
                    {error && (
                        <div className="flex items-center gap-2 text-sm text-red-500"><AlertCircle className="h-4 w-4" /> {error}</div>
                    )}
                </CardContent>
            </Card>
            <div ref={mapEl} className="w-full rounded-lg border" style={{ height: "60vh" }} />
            {h3 && (
                <Card>
                    <CardHeader><CardTitle className="text-base">H3 density (res 7) — top cells</CardTitle></CardHeader>
                    <CardContent>
                        <div className="text-xs font-mono space-y-1">
                            {h3.slice(0, 10).map((c) => (
                                <div key={c.cell}>{c.cell} · <b>{c.count}</b></div>
                            ))}
                            {h3.length === 0 && <span className="text-muted-foreground">no cells</span>}
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
