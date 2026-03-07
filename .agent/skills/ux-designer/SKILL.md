---
name: ux-designer
description: UX/UI design system and patterns for Project Mimir dashboard — design tokens (oklch colors, radius, typography), shadcn/ui component library, dark mode support, status badge palette, layout patterns, accessibility standards, empty/loading/error states, and responsive design. Triggers when designing UI, creating components, choosing colors, styling elements, improving user experience, or building dashboard pages.
---

# UX Designer Skill

Project Mimir's dashboard follows a cohesive design system built on **shadcn/ui + Tailwind CSS + oklch color space**. This skill ensures visual consistency and premium user experience across all pages.

## Design System Foundation

### Color Space: oklch
All theme colors use **oklch** format in `globals.css` for perceptually uniform color manipulation:

```css
/* Light mode */
--background: oklch(1 0 0);          /* Pure white */
--foreground: oklch(0.145 0 0);      /* Near black */
--muted-foreground: oklch(0.556 0 0); /* Secondary text */
--border: oklch(0.922 0 0);          /* Light gray */
--destructive: oklch(0.577 0.245 27.325); /* Red */

/* Dark mode (.dark class) */
--background: oklch(0.145 0 0);      /* Near black */
--foreground: oklch(0.985 0 0);      /* Near white */
--card: oklch(0.205 0 0);            /* Elevated surface */
--border: oklch(1 0 0 / 10%);        /* White 10% opacity */
```

### Typography
- **Sans**: `Geist` (`--font-geist-sans`) — body text, headings, UI
- **Mono**: `Geist Mono` (`--font-geist-mono`) — code, numbers, IDs
- Set in `layout.tsx` via `next/font/google`

### Border Radius Scale
```css
--radius: 0.625rem;  /* Base = 10px */
--radius-sm: calc(var(--radius) - 4px);   /* 6px */
--radius-md: calc(var(--radius) - 2px);   /* 8px */
--radius-lg: var(--radius);               /* 10px */
--radius-xl: calc(var(--radius) + 4px);   /* 14px */
```

## Status Color Palette

Standard color pairs for light/dark mode. **Always provide both variants:**

| Status               | Light Mode                      | Dark Mode                                    |
| -------------------- | ------------------------------- | -------------------------------------------- |
| 🟡 Warning/Processing | `bg-amber-100 text-amber-700`   | `dark:bg-amber-900/40 dark:text-amber-300`   |
| 🟢 Success/Complete   | `bg-green-100 text-green-700`   | `dark:bg-green-900/40 dark:text-green-300`   |
| 🔴 Error/Failed       | `bg-red-100 text-red-700`       | `dark:bg-red-900/40 dark:text-red-300`       |
| 🔵 Info/Primary       | `bg-blue-100 text-blue-700`     | `dark:bg-blue-900/40 dark:text-blue-300`     |
| 🟣 Highlight          | `bg-purple-100 text-purple-700` | `dark:bg-purple-900/40 dark:text-purple-300` |
| 🔶 Accent             | `bg-cyan-100 text-cyan-700`     | `dark:bg-cyan-900/40 dark:text-cyan-300`     |
| 🌹 Danger             | `bg-rose-100 text-rose-700`     | `dark:bg-rose-900/40 dark:text-rose-300`     |
| 🌊 Teal               | `bg-teal-100 text-teal-700`     | `dark:bg-teal-900/40 dark:text-teal-300`     |

### Applying Colors
```tsx
// ✅ Always pair light + dark
className="bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"

// ❌ Never use only light mode colors
className="bg-green-100 text-green-700"
```

## Component Patterns

### Status Badge (Rounded Pill)
```tsx
<span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
    <Loader2 className="w-3 h-3 animate-spin" />
    Running
</span>
```

### KPI Stat Card
```tsx
<div className="rounded-xl border bg-card p-5 shadow-sm">
    <div className="flex items-center gap-2 text-muted-foreground text-sm mb-2">
        <Database className="w-4 h-4" />
        Total Sources
    </div>
    <div className="text-2xl font-bold">{count.toLocaleString()}</div>
</div>
```

### Page Header
```tsx
<div className="flex items-center justify-between">
    <div>
        <h1 className="text-2xl font-bold flex items-center gap-2">
            <BookOpen className="w-6 h-6 text-blue-600" />
            Page Title
        </h1>
        <p className="text-muted-foreground text-sm mt-1">
            Description text
        </p>
    </div>
    {/* Optional: KPI pills on the right */}
    <div className="flex items-center gap-3 text-sm text-muted-foreground">
        <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted">
            <Layers className="w-4 h-4" />
            <span className="font-semibold">{total.toLocaleString()}</span> items
        </div>
    </div>
</div>
```

### Toast Notification
Fixed position, top-right, auto-dismiss after 4s:
```tsx
<div className={`fixed top-4 right-4 z-50 flex items-center gap-2 px-4 py-3 rounded-lg shadow-lg
    ${type === "success" ? "bg-green-600 text-white" : "bg-red-600 text-white"}`}>
    {type === "success" ? <CheckCircle2 className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
    <span className="text-sm font-medium">{message}</span>
</div>
```

### Floating Action Bar (Bottom-Center)
For bulk selection operations:
```tsx
<div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 flex items-center gap-4 px-6 py-3 bg-card border shadow-xl rounded-full">
    <span className="text-sm font-medium">{count} selected</span>
    <Button size="sm">Action</Button>
    <button className="text-muted-foreground hover:text-foreground"><X className="w-4 h-4" /></button>
</div>
```

## Three-State Rendering

Every data-driven component must handle three states:

### 1. Loading State
```tsx
<div className="flex items-center justify-center py-16 text-muted-foreground">
    <div className="animate-spin w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full mr-3" />
    Loading data...
</div>
```

### 2. Empty State
```tsx
<div className="flex flex-col items-center justify-center py-16 text-center">
    <div className="w-16 h-16 rounded-full bg-blue-50 dark:bg-blue-900/30 flex items-center justify-center mb-4">
        <BookOpen className="w-8 h-8 text-blue-600 dark:text-blue-400" />
    </div>
    <h3 className="text-lg font-semibold mb-2">No items found</h3>
    <p className="text-muted-foreground text-sm max-w-sm">Description text</p>
    <Button className="mt-4" variant="outline">Action</Button>
</div>
```

### 3. Content State
Render the actual data (tables, cards, lists).

## Layout Patterns

### Page Container
```tsx
<div className="container mx-auto p-8 space-y-6">
    {/* Header */}
    {/* Filters */}
    {/* Content */}
</div>
```

### Dashboard Grid
```tsx
<div className="grid gap-6 lg:grid-cols-5">
    <div className="lg:col-span-3">{/* Main content */}</div>
    <div className="lg:col-span-2">{/* Sidebar */}</div>
</div>
```

### Root Layout (layout.tsx)
```
┌────────────────────────────────┐
│ Navbar (sticky top)            │
├────────────────────────────────┤
│ PipelineStatusBar              │
├────────────────────────────────┤
│ Main Content (flex-1)          │
│                                │
│                                │
├────────────────────────────────┤
│ FeedbackButton (fixed bottom)  │
└────────────────────────────────┘
```

## Icon Library: Lucide React

Use `lucide-react` for all icons. Common icons used:

| Context        | Icon                            |
| -------------- | ------------------------------- |
| Knowledge/Docs | `BookOpen`                      |
| Search         | `Search`                        |
| Filter         | `Filter`                        |
| Settings       | `Settings`                      |
| Loading        | `Loader2` (with `animate-spin`) |
| Success        | `CheckCircle2`                  |
| Error          | `AlertCircle`                   |
| AI/Generate    | `Sparkles`                      |
| Refresh        | `RefreshCw`                     |
| Navigation     | `ChevronLeft`, `ChevronRight`   |
| Database       | `Database`                      |
| Analytics      | `BarChart3`                     |
| Agent/Bot      | `Bot`                           |

### Icon Sizing
- **Inline text**: `w-4 h-4`
- **Header icons**: `w-6 h-6`
- **Empty state**: `w-8 h-8`
- **Badge icons**: `w-3 h-3`

## Accessibility (a11y)

### Required
- **aria-label** on all interactive elements without visible text
- **Semantic HTML**: Use `<main>`, `<nav>`, `<section>`, `<button>` (not `<div onClick>`)
- **Keyboard navigation**: All interactive elements must be focusable
- **Color contrast**: WCAG AA minimum (dark mode carefully checked)
- **Unique IDs** on interactive elements for testing

### Pattern
```tsx
// ✅ Accessible
<button onClick={doThing} aria-label="Close selection">
    <X className="w-4 h-4" />
</button>

// ❌ Inaccessible
<div onClick={doThing}>
    <X className="w-4 h-4" />
</div>
```

## Animations

### Standard transitions
- **Hover**: `transition-colors` for color changes
- **Spin**: `animate-spin` for loading indicators
- **Toast slide-in**: `animate-in slide-in-from-top-2`
- **Typing dots**: Custom `typing-dot` keyframe animation (in `globals.css`)

### Custom Animation Pattern (globals.css)
```css
@keyframes typing-dot {
    0%, 80%, 100% {
        transform: translateY(0);
        opacity: 0.4;
    }
    40% {
        transform: translateY(-4px);
        opacity: 1;
    }
}
```

## Markdown Rendering

Chat messages and content displays use the `.markdown-body` class from `globals.css`:
```tsx
<div className="markdown-body prose prose-sm dark:prose-invert max-w-none">
    {renderedContent}
</div>
```

## Design Checklist for New Pages

- [ ] Uses `container mx-auto p-8 space-y-6` container
- [ ] Page header with icon + title + description
- [ ] Loading, empty, and content states implemented
- [ ] All colors have dark mode variants
- [ ] Icons from lucide-react with correct sizing
- [ ] Interactive elements have `aria-label` attributes
- [ ] Hover transitions smooth (`transition-colors`)
- [ ] Toast notifications for user actions
- [ ] Table rows have `hover:bg-muted/50 transition-colors`
- [ ] shadcn/ui components for all UI primitives
