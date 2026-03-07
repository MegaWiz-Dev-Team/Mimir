# Page Example: Knowledge Base

This example shows the canonical page pattern used in Project Mimir, based on `src/app/knowledge/page.tsx`.

## Key Patterns Demonstrated

1. **Three-state rendering**: Loading spinner → Empty state → Data table
2. **Debounced search**: 300ms delay before filtering
3. **Source filter dropdown**: Dynamic filter from API data
4. **Checkbox selection**: Per-row + select-all with floating action bar
5. **QA Status badges**: Color-coded pills (amber/green/red)
6. **Auto-refresh polling**: 5s interval after QA trigger, auto-stop when complete
7. **Toast notifications**: Success/error messages with auto-dismiss
8. **Chunk detail dialog**: Modal for viewing full content

## Helper Function Pattern

Small helpers defined at file top, outside the component:

```tsx
/** Get QA status from chunk metadata */
function getQaStatus(chunk: ChunkItem): "none" | "processing" | "completed" | "failed" {
    if (!chunk.metadata_json) return "none";
    const meta = typeof chunk.metadata_json === "string"
        ? JSON.parse(chunk.metadata_json) : chunk.metadata_json;
    return meta?.qa_status || "none";
}

/** Deterministic color for a source name (consistent hash → color) */
function sourceColor(name: string): string {
    const colors = [
        "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
        "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
        // ... more colors
    ];
    let hash = 0;
    for (let i = 0; i < name.length; i++)
        hash = name.charCodeAt(i) + ((hash << 5) - hash);
    return colors[Math.abs(hash) % colors.length];
}
```

## Component Sections

Structure your page component with section borders:

```tsx
export default function KnowledgePage() {
    // ─── State ────────────────────────────────────────────────────
    const [chunks, setChunks] = useState<ChunkItem[]>([]);
    const [isLoading, setIsLoading] = useState(true);

    // ─── Selection Logic ──────────────────────────────────────────
    const toggleSelect = (id: number) => { /* ... */ };
    const toggleSelectAll = () => { /* ... */ };

    // ─── Auto-Refresh Polling ─────────────────────────────────────
    const [pollActive, setPollActive] = useState(false);
    useEffect(() => { /* polling logic */ }, [pollActive]);

    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* Toast */}
            {/* Header */}
            {/* Filters */}
            {/* Table */}
            {/* Floating Action Bar */}
            {/* Detail Dialog */}
        </div>
    );
}
```

## Reference
Full source: `ro-ai-dashboard/src/app/knowledge/page.tsx`
