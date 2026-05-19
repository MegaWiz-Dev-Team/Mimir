"use client";

import { useEffect, useMemo, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
    Card, CardContent, CardHeader, CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import {
    ArrowLeft, Search, Loader2, AlertCircle, ChevronLeft, ChevronRight,
} from "lucide-react";
import { API_BASE_URL, authFetch } from "@/lib/api";

type Column = {
    name: string;
    label: string;
    kind: "string" | "code" | "boolean" | "enum" | string;
};

type ItemsResponse = {
    kb_id: string;
    columns: Column[];
    items: Record<string, any>[];
    total: number;
    page: number;
    per_page: number;
    filters: Record<string, string[]>;
};

export default function KbBrowserPage() {
    const params = useParams();
    const kbId = String(params?.kbId || "");

    const [data, setData] = useState<ItemsResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    // Input state
    const [search, setSearch] = useState("");
    const [searchDebounce, setSearchDebounce] = useState("");
    const [activeFilters, setActiveFilters] = useState<Record<string, string>>({});
    const [page, setPage] = useState(1);
    const PER_PAGE = 20;

    useEffect(() => {
        const t = setTimeout(() => setSearchDebounce(search), 300);
        return () => clearTimeout(t);
    }, [search]);

    useEffect(() => {
        // Reset to page 1 when search/filter changes
        setPage(1);
    }, [searchDebounce, activeFilters]);

    useEffect(() => {
        if (!kbId) return;
        let cancelled = false;
        (async () => {
            try {
                setLoading(true);
                const qs = new URLSearchParams();
                qs.set("page", String(page));
                qs.set("per_page", String(PER_PAGE));
                if (searchDebounce) qs.set("q", searchDebounce);
                for (const [k, v] of Object.entries(activeFilters)) {
                    if (v) qs.set(`filter_${k}`, v);
                }
                const res = await authFetch(
                    `${API_BASE_URL}/knowledge/shared/${kbId}/items?${qs}`,
                    { cache: "no-store" },
                );
                if (!res.ok) {
                    const body = await res.text();
                    throw new Error(`HTTP ${res.status}: ${body.slice(0, 200)}`);
                }
                const j = await res.json();
                if (!cancelled) {
                    setData(j);
                    setError(null);
                }
            } catch (e: any) {
                if (!cancelled) setError(e?.message || "Failed to load");
            } finally {
                if (!cancelled) setLoading(false);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [kbId, page, searchDebounce, activeFilters]);

    const totalPages = useMemo(
        () => (data ? Math.max(1, Math.ceil(data.total / data.per_page)) : 1),
        [data],
    );

    const renderCell = (col: Column, item: Record<string, any>) => {
        const v = item[col.name];
        if (v === null || v === undefined || v === "") {
            return <span className="text-muted-foreground italic">—</span>;
        }
        if (col.kind === "boolean") {
            return v ? (
                <span className="text-green-600 font-mono text-xs">true</span>
            ) : (
                <span className="text-zinc-500 font-mono text-xs">false</span>
            );
        }
        if (col.kind === "code") {
            return <span className="font-mono text-sm">{String(v)}</span>;
        }
        if (col.kind === "enum") {
            return (
                <span className="inline-block px-1.5 py-0.5 rounded text-[10px] font-mono uppercase bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300">
                    {String(v)}
                </span>
            );
        }
        return <span className="text-sm">{String(v)}</span>;
    };

    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* Header */}
            <div className="flex items-center gap-3">
                <Link href="/knowledge/shared">
                    <Button variant="ghost" size="sm">
                        <ArrowLeft className="w-4 h-4 mr-1" /> All KBs
                    </Button>
                </Link>
                <h1 className="text-2xl font-bold font-mono">{kbId}</h1>
                {data && (
                    <span className="text-sm text-muted-foreground">
                        {data.total.toLocaleString()} items
                    </span>
                )}
            </div>

            {/* Search + filters */}
            <Card>
                <CardContent className="py-4 flex items-center gap-3 flex-wrap">
                    <div className="relative flex-1 min-w-[240px]">
                        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                        <Input
                            placeholder="Search code or name…"
                            value={search}
                            onChange={(e) => setSearch(e.target.value)}
                            className="pl-10"
                        />
                    </div>
                    {data &&
                        Object.entries(data.filters).map(([field, values]) => (
                            <div key={field} className="flex items-center gap-1">
                                <label className="text-xs text-muted-foreground">
                                    {field}:
                                </label>
                                <select
                                    value={activeFilters[field] || ""}
                                    onChange={(e) =>
                                        setActiveFilters((p) => ({
                                            ...p,
                                            [field]: e.target.value,
                                        }))
                                    }
                                    className="h-9 rounded-md border bg-background px-2 py-1 text-sm min-w-[100px]"
                                >
                                    <option value="">all</option>
                                    {values.slice(0, 50).map((v) => (
                                        <option key={v} value={v}>
                                            {v}
                                        </option>
                                    ))}
                                </select>
                            </div>
                        ))}
                </CardContent>
            </Card>

            {/* Error */}
            {error && (
                <Card className="border-red-300 bg-red-50 dark:bg-red-900/20">
                    <CardContent className="py-4 flex items-center gap-3">
                        <AlertCircle className="w-5 h-5 text-red-600" />
                        <span className="text-sm">{error}</span>
                    </CardContent>
                </Card>
            )}

            {/* Table */}
            <Card>
                <CardContent className="p-0">
                    {loading ? (
                        <div className="flex items-center justify-center py-16 text-muted-foreground">
                            <Loader2 className="w-5 h-5 animate-spin mr-2" /> Loading…
                        </div>
                    ) : !data || data.items.length === 0 ? (
                        <div className="py-16 text-center text-muted-foreground">
                            No items match the current search/filter.
                        </div>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    {data.columns.map((c) => (
                                        <TableHead key={c.name}>{c.label}</TableHead>
                                    ))}
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {data.items.map((it, idx) => (
                                    <TableRow key={idx}>
                                        {data.columns.map((c) => (
                                            <TableCell key={c.name}>
                                                {renderCell(c, it)}
                                            </TableCell>
                                        ))}
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
                {data && totalPages > 1 && (
                    <div className="flex items-center justify-between px-6 py-3 border-t">
                        <p className="text-sm text-muted-foreground">
                            {(data.page - 1) * data.per_page + 1}–
                            {Math.min(data.page * data.per_page, data.total)} of{" "}
                            {data.total.toLocaleString()}
                        </p>
                        <div className="flex items-center gap-2">
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={page <= 1}
                                onClick={() => setPage((p) => Math.max(1, p - 1))}
                            >
                                <ChevronLeft className="w-4 h-4" />
                            </Button>
                            <span className="text-sm font-medium px-2">
                                {page} / {totalPages}
                            </span>
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={page >= totalPages}
                                onClick={() =>
                                    setPage((p) => Math.min(totalPages, p + 1))
                                }
                            >
                                <ChevronRight className="w-4 h-4" />
                            </Button>
                        </div>
                    </div>
                )}
            </Card>
        </div>
    );
}
