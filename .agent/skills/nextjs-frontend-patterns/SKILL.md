---
name: nextjs-frontend-patterns
description: Next.js App Router frontend patterns for Project Mimir — page structure, API integration via lib/api.ts, shadcn/ui component usage, polling and debounce patterns, toast notifications, badge design, and TypeScript conventions. Triggers when building React components, creating pages, integrating APIs, styling UI elements, or working on the ro-ai-dashboard frontend.
---

# Next.js Frontend Patterns Skill

Project Mimir's dashboard (`ro-ai-dashboard/`) is built with **Next.js App Router + TypeScript + shadcn/ui + Tailwind CSS**. This skill defines the standard patterns for all frontend development.

## Project Structure

```
ro-ai-dashboard/src/
├── app/                     # Next.js App Router pages
│   ├── layout.tsx           # Root layout (Navbar + PipelineStatusBar + FeedbackButton)
│   ├── page.tsx             # Dashboard home
│   ├── globals.css          # Global styles + CSS variables
│   ├── knowledge/           # Knowledge Base page
│   │   └── page.tsx
│   ├── playground/          # LLM Chat Playground
│   ├── agents/              # Agent Studio
│   ├── settings/            # Provider settings
│   ├── analytics/           # Usage analytics
│   ├── coverage/            # Coverage dashboard
│   └── ...                  # More route pages
├── components/              # Shared components
│   ├── ui/                  # shadcn/ui primitives (Card, Button, Dialog, Table, etc.)
│   ├── navbar.tsx           # Navigation bar
│   ├── pipeline-status-bar.tsx
│   ├── feedback-button.tsx
│   └── *.test.tsx           # Co-located tests
├── lib/                     # Shared utilities
│   ├── api.ts               # Central API client (ALL backend calls)
│   ├── api.test.ts          # API tests
│   └── utils.ts             # Utility functions
└── types/                   # TypeScript type definitions
    └── pipeline.ts
```

## 1. Page Pattern (App Router)

Every page follows this structure:

```tsx
"use client";  // Required for pages with interactivity

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { BookOpen } from "lucide-react";
import { fetchSomeData, SomeType } from "@/lib/api";

export default function MyPage() {
    // ─── State ────────────────────────────────────────────────────
    const [data, setData] = useState<SomeType[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [page, setPage] = useState(1);

    // ─── Data Fetching ────────────────────────────────────────────
    const loadData = useCallback(async () => {
        setIsLoading(true);
        try {
            const result = await fetchSomeData({ page });
            setData(result.items);
        } catch {
            setData([]);
        } finally {
            setIsLoading(false);
        }
    }, [page]);

    useEffect(() => { loadData(); }, [loadData]);

    // ─── Render ───────────────────────────────────────────────────
    return (
        <div className="container mx-auto p-8 space-y-6">
            {/* Header */}
            <div>
                <h1 className="text-2xl font-bold flex items-center gap-2">
                    <BookOpen className="w-6 h-6 text-blue-600" />
                    Page Title
                </h1>
                <p className="text-muted-foreground text-sm mt-1">
                    Description text
                </p>
            </div>

            {/* Content */}
            <Card>
                <CardContent>
                    {isLoading ? (
                        <LoadingSpinner />
                    ) : data.length === 0 ? (
                        <EmptyState />
                    ) : (
                        <DataTable items={data} />
                    )}
                </CardContent>
            </Card>
        </div>
    );
}
```

### Key Conventions:
- `"use client"` directive for all interactive pages
- **Section comments** using `// ─── Section Name ────...` borders
- **Three-state rendering**: Loading → Empty → Content
- **Container**: `className="container mx-auto p-8 space-y-6"`

## 2. API Integration (`lib/api.ts`)

ALL backend calls go through `lib/api.ts`. Never call `fetch()` directly in components.

### Adding a New API Function

```typescript
// In lib/api.ts

// ─── Feature Name API ──────────────────────────────────────────────────

export interface MyItem {
    id: number;
    name: string;
    tenant_id: string;
}

export async function fetchMyItems(params?: { page?: number }): Promise<MyItem[]> {
    const query = params?.page ? `?page=${params.page}` : "";
    const res = await authFetch(`${API_BASE_URL}/my-items${query}`);
    if (!res.ok) throw new Error("Failed to fetch items");
    return res.json();
}

export async function createMyItem(data: Partial<MyItem>): Promise<MyItem> {
    const res = await authFetch(`${API_BASE_URL}/my-items`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error("Failed to create item");
    return res.json();
}
```

### Rules:
- Use `authFetch()` wrapper (auto-adds auth headers + tenant ID)
- Group functions by domain with section comment headers
- Export TypeScript interfaces alongside functions
- Throw descriptive errors on non-OK responses

## 3. Component Patterns

### shadcn/ui Usage
All UI primitives come from `@/components/ui/`:
```tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
```

### Icons: Lucide React
```tsx
import { BookOpen, Search, Filter, ChevronLeft, ChevronRight, Loader2, CheckCircle2, AlertCircle, Sparkles } from "lucide-react";
```

### Status Badge Pattern
Color-coded rounded pills for status display:
```tsx
function StatusBadge({ status }: { status: "none" | "processing" | "completed" | "failed" }) {
    switch (status) {
        case "processing":
            return (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
                    <Loader2 className="w-3 h-3 animate-spin" /> Running
                </span>
            );
        case "completed":
            return (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300">
                    <CheckCircle2 className="w-3 h-3" /> Done
                </span>
            );
        case "failed":
            return (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300">
                    <AlertCircle className="w-3 h-3" /> Failed
                </span>
            );
        default:
            return <span className="text-muted-foreground text-xs">—</span>;
    }
}
```

### Badge Color Palette
| Status             | Light                         | Dark                             |
| ------------------ | ----------------------------- | -------------------------------- |
| Processing/Warning | `bg-amber-100 text-amber-700` | `bg-amber-900/40 text-amber-300` |
| Success/Complete   | `bg-green-100 text-green-700` | `bg-green-900/40 text-green-300` |
| Error/Failed       | `bg-red-100 text-red-700`     | `bg-red-900/40 text-red-300`     |
| Info/Default       | `bg-blue-100 text-blue-700`   | `bg-blue-900/40 text-blue-300`   |

## 4. Common Patterns

### Debounce Search
```tsx
const [search, setSearch] = useState("");
const [searchDebounce, setSearchDebounce] = useState("");

useEffect(() => {
    const t = setTimeout(() => setSearchDebounce(search), 300);
    return () => clearTimeout(t);
}, [search]);

// Reset page on search change
useEffect(() => { setPage(1); }, [searchDebounce]);
```

### Auto-Refresh Polling
```tsx
const [pollActive, setPollActive] = useState(false);

useEffect(() => {
    if (!pollActive) return;
    const interval = setInterval(() => {
        loadData().then(() => {
            const anyProcessing = items.some(i => i.status === "processing");
            if (!anyProcessing) setPollActive(false);  // Auto-stop
        });
    }, 5000);  // 5s interval
    return () => clearInterval(interval);
}, [pollActive, items, loadData]);

// Activate polling after action
const handleAction = async () => {
    await triggerAction();
    setPollActive(true);
    setTimeout(loadData, 2000);  // First refresh after 2s
};
```

### Toast Notifications
```tsx
const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

// Auto-dismiss after 4 seconds
useEffect(() => {
    if (toast) {
        const t = setTimeout(() => setToast(null), 4000);
        return () => clearTimeout(t);
    }
}, [toast]);

// In JSX — fixed position top-right
{toast && (
    <div className={`fixed top-4 right-4 z-50 flex items-center gap-2 px-4 py-3 rounded-lg shadow-lg ${
        toast.type === "success" ? "bg-green-600 text-white" : "bg-red-600 text-white"
    }`}>
        {toast.type === "success" ? <CheckCircle2 className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
        <span className="text-sm font-medium">{toast.message}</span>
    </div>
)}
```

### Floating Action Bar (Bulk Selection)
```tsx
{selectedIds.size > 0 && (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 flex items-center gap-4 px-6 py-3 bg-card border shadow-xl rounded-full">
        <span className="text-sm font-medium">
            {selectedIds.size} item{selectedIds.size > 1 ? "s" : ""} selected
        </span>
        <Button size="sm" onClick={handleBulkAction}>
            <Sparkles className="w-4 h-4 mr-1" />
            Action
        </Button>
        <button onClick={() => setSelectedIds(new Set())} className="text-muted-foreground hover:text-foreground">
            <X className="w-4 h-4" />
        </button>
    </div>
)}
```

## 5. Testing (Frontend)

### Co-located Tests
Place test files alongside components: `component.test.tsx`

```tsx
// components/my-component.test.tsx
import { render, screen } from "@testing-library/react";
import MyComponent from "./my-component";

describe("MyComponent", () => {
    it("renders title", () => {
        render(<MyComponent />);
        expect(screen.getByText("Title")).toBeInTheDocument();
    });
});
```

### Build Verification
Always verify builds pass:
```bash
cd ro-ai-dashboard && npx next build
```

## 6. TypeScript Conventions

- Define interfaces in `lib/api.ts` alongside API functions (co-location)
- Use `@/` path alias for imports (maps to `src/`)
- Shared types in `types/` directory for cross-component types
- Use strict null checks — handle `undefined` and `null` explicitly
