"use client";

// Sprint 39 Mimir Curator — single-pair review page.
//
// UX requirements (Sprint 39 plan):
//   - Side-by-side: question + AI answer + expected answer + citations
//   - Rating widgets (1-5 + binary safety)
//   - Editable "improved answer" textarea
//   - Specialty dropdown
//   - Reject button
//   - Auto-load next pair on save
//   - Resume support (server cursor)
//   - Keyboard shortcuts:
//       1-5 = accuracy rating
//       Shift+1-5 = completeness rating
//       Alt+1-5 = relevance rating
//       S = toggle safety (1↔0)
//       Enter = submit APPROVED + next
//       R = REJECT + next
//       F = FLAGGED + next
//       Esc = clear current ratings

import { useEffect, useRef, useState, useCallback } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
    getDataset,
    getNextItem,
    submitReview,
    exportDatasetUrl,
    parseTags,
    COMMON_TAGS,
    type CorpusDataset,
    type CorpusItem,
} from "@/lib/training-api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import {
    ArrowLeft,
    Check,
    X,
    Flag,
    Download,
    Star,
    AlertTriangle,
    Loader2,
    Keyboard,
    Shield,
    ShieldOff,
} from "lucide-react";

const SPECIALTIES = [
    "general-medicine",
    "cardiology",
    "endocrinology",
    "neurology",
    "oncology",
    "pediatrics",
    "psychiatry",
    "ent",
    "obgyn",
    "orthopedics",
    "ophthalmology",
    "emergency-medicine",
    "pulmonology",
    "rheumatology",
    "urology",
    "pharmacy",
    "nursing",
    "dietitian",
    "physical-therapy",
    "social-work",
    "medical-technology",
];

interface Ratings {
    accuracy: number | null;
    completeness: number | null;
    relevance: number | null;
    safety: number | null; // 1 = safe, 0 = unsafe
}

const EMPTY_RATINGS: Ratings = {
    accuracy: null,
    completeness: null,
    relevance: null,
    safety: 1, // default safe — reviewers usually only flip when problem
};

export default function ReviewDatasetPage() {
    const params = useParams<{ id: string }>();
    const datasetId = params.id;

    const [dataset, setDataset] = useState<CorpusDataset | null>(null);
    const [item, setItem] = useState<CorpusItem | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [submitting, setSubmitting] = useState(false);

    const [ratings, setRatings] = useState<Ratings>(EMPTY_RATINGS);
    const [improvedAnswer, setImprovedAnswer] = useState("");
    const [specialty, setSpecialty] = useState<string>("");
    const [tags, setTags] = useState<string[]>([]);
    const [tagInput, setTagInput] = useState<string>("");
    const [notes, setNotes] = useState("");
    const [showShortcuts, setShowShortcuts] = useState(false);

    const improvedRef = useRef<HTMLTextAreaElement | null>(null);

    // ─── Load + advance ──────────────────────────────────────────────────────
    const loadNext = useCallback(async () => {
        if (!datasetId) return;
        setLoading(true);
        setError(null);
        try {
            const [d, it] = await Promise.all([
                getDataset(datasetId),
                getNextItem(datasetId),
            ]);
            setDataset(d);
            setItem(it);
            setRatings(EMPTY_RATINGS);
            setImprovedAnswer(it?.ai_answer ?? "");
            setSpecialty(it?.specialty ?? "");
            setTags(parseTags(it?.tags ?? null));
            setTagInput("");
            setNotes("");
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        } finally {
            setLoading(false);
        }
    }, [datasetId]);

    useEffect(() => {
        loadNext();
    }, [loadNext]);

    // ─── Submit ─────────────────────────────────────────────────────────────
    const submit = useCallback(
        async (status: "APPROVED" | "REJECTED" | "FLAGGED") => {
            if (!item || submitting) return;
            // For APPROVED, require all 1-5 dims set. REJECTED/FLAGGED can skip.
            if (
                status === "APPROVED" &&
                (ratings.accuracy === null ||
                    ratings.completeness === null ||
                    ratings.relevance === null)
            ) {
                setError("Approve requires all ratings (acc/comp/rel).");
                return;
            }
            setSubmitting(true);
            setError(null);
            try {
                await submitReview(datasetId, item.id, {
                    accuracy_score: ratings.accuracy,
                    completeness_score: ratings.completeness,
                    relevance_score: ratings.relevance,
                    safety_score: ratings.safety,
                    improved_answer:
                        improvedAnswer && improvedAnswer !== item.ai_answer
                            ? improvedAnswer
                            : undefined,
                    specialty: specialty || undefined,
                    // Always send tags so user-cleared state is persisted.
                    tags,
                    notes: notes || undefined,
                    status,
                });
                await loadNext();
            } catch (e) {
                setError(e instanceof Error ? e.message : String(e));
            } finally {
                setSubmitting(false);
            }
        },
        [item, ratings, improvedAnswer, specialty, tags, notes, datasetId, submitting, loadNext]
    );

    // ─── Keyboard shortcuts ──────────────────────────────────────────────────
    useEffect(() => {
        function onKey(e: KeyboardEvent) {
            // Don't intercept if user is typing in textarea/input
            const t = e.target as HTMLElement;
            if (
                t.tagName === "TEXTAREA" ||
                t.tagName === "INPUT" ||
                t.tagName === "SELECT"
            ) {
                return;
            }
            // 1-5 = ratings (acc / shift=comp / alt=rel)
            if (/^[1-5]$/.test(e.key)) {
                const v = parseInt(e.key);
                if (e.shiftKey) {
                    setRatings((r) => ({ ...r, completeness: v }));
                } else if (e.altKey) {
                    setRatings((r) => ({ ...r, relevance: v }));
                } else {
                    setRatings((r) => ({ ...r, accuracy: v }));
                }
                e.preventDefault();
                return;
            }
            // S = toggle safety
            if (e.key.toLowerCase() === "s" && !e.metaKey && !e.ctrlKey) {
                setRatings((r) => ({ ...r, safety: r.safety === 1 ? 0 : 1 }));
                e.preventDefault();
                return;
            }
            // Enter = submit APPROVED
            if (e.key === "Enter" && !e.shiftKey) {
                submit("APPROVED");
                e.preventDefault();
                return;
            }
            // R = REJECT
            if (e.key.toLowerCase() === "r") {
                submit("REJECTED");
                e.preventDefault();
                return;
            }
            // F = FLAGGED
            if (e.key.toLowerCase() === "f") {
                submit("FLAGGED");
                e.preventDefault();
                return;
            }
            // Esc = clear ratings
            if (e.key === "Escape") {
                setRatings(EMPTY_RATINGS);
                e.preventDefault();
                return;
            }
            // ? = show shortcuts
            if (e.key === "?") {
                setShowShortcuts((s) => !s);
                e.preventDefault();
                return;
            }
        }
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [submit]);

    // ─── Progress ────────────────────────────────────────────────────────────
    const progress = dataset
        ? Math.round(
              ((dataset.approved_items + dataset.rejected_items) /
                  Math.max(dataset.total_items, 1)) *
                  100
          )
        : 0;

    return (
        <div className="container mx-auto p-6 space-y-4 max-w-7xl">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <Link
                        href="/training"
                        className="text-muted-foreground hover:text-foreground"
                    >
                        <ArrowLeft className="h-5 w-5" />
                    </Link>
                    <div>
                        <h1 className="text-2xl font-bold">
                            {dataset?.name ?? "Loading…"}
                        </h1>
                        {dataset && (
                            <div className="text-sm text-muted-foreground flex gap-3 items-center mt-1">
                                <span>{dataset.total_items} total</span>
                                <span className="text-green-600">
                                    {dataset.approved_items} approved
                                </span>
                                <span className="text-red-600">
                                    {dataset.rejected_items} rejected
                                </span>
                                <span>{progress}% reviewed</span>
                                {dataset.tenant_id === null && (
                                    <Badge variant="outline">Shared</Badge>
                                )}
                            </div>
                        )}
                    </div>
                </div>
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setShowShortcuts((s) => !s)}
                    >
                        <Keyboard className="h-4 w-4 mr-1" /> Shortcuts (?)
                    </Button>
                    <a
                        href={exportDatasetUrl(datasetId)}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <Button variant="outline" size="sm">
                            <Download className="h-4 w-4 mr-1" /> Export JSONL
                        </Button>
                    </a>
                </div>
            </div>

            {/* Progress bar */}
            {dataset && (
                <div className="w-full h-2 bg-muted rounded">
                    <div
                        className="h-2 bg-primary rounded transition-all"
                        style={{ width: `${progress}%` }}
                    />
                </div>
            )}

            {/* Shortcuts cheat sheet */}
            {showShortcuts && (
                <Card>
                    <CardHeader>
                        <CardTitle className="text-sm">Keyboard shortcuts</CardTitle>
                    </CardHeader>
                    <CardContent className="text-sm grid grid-cols-2 md:grid-cols-3 gap-2">
                        <div><kbd className="px-1 bg-muted rounded">1</kbd>–<kbd className="px-1 bg-muted rounded">5</kbd> Accuracy</div>
                        <div><kbd className="px-1 bg-muted rounded">⇧+1</kbd>–<kbd className="px-1 bg-muted rounded">⇧+5</kbd> Completeness</div>
                        <div><kbd className="px-1 bg-muted rounded">⌥+1</kbd>–<kbd className="px-1 bg-muted rounded">⌥+5</kbd> Relevance</div>
                        <div><kbd className="px-1 bg-muted rounded">S</kbd> Toggle safety</div>
                        <div><kbd className="px-1 bg-muted rounded">Enter</kbd> Approve + next</div>
                        <div><kbd className="px-1 bg-muted rounded">R</kbd> Reject + next</div>
                        <div><kbd className="px-1 bg-muted rounded">F</kbd> Flag + next</div>
                        <div><kbd className="px-1 bg-muted rounded">Esc</kbd> Clear ratings</div>
                        <div><kbd className="px-1 bg-muted rounded">?</kbd> Toggle this</div>
                    </CardContent>
                </Card>
            )}

            {error && (
                <Card className="border-destructive">
                    <CardContent className="pt-4 text-destructive text-sm">
                        {error}
                    </CardContent>
                </Card>
            )}

            {/* No more items */}
            {loading ? (
                <div className="flex justify-center py-20">
                    <Loader2 className="h-8 w-8 animate-spin" />
                </div>
            ) : !item ? (
                <Card>
                    <CardContent className="py-20 text-center">
                        <Check className="h-12 w-12 text-green-600 mx-auto mb-3" />
                        <h2 className="text-xl font-bold mb-2">Queue empty</h2>
                        <p className="text-muted-foreground">
                            All pending items in this dataset have been reviewed.
                        </p>
                    </CardContent>
                </Card>
            ) : (
                <div className="grid lg:grid-cols-2 gap-4">
                    {/* Left: question + AI answer + expected */}
                    <div className="space-y-4">
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base">Question</CardTitle>
                            </CardHeader>
                            <CardContent className="text-sm whitespace-pre-wrap">
                                {item.question}
                            </CardContent>
                        </Card>
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base">AI answer</CardTitle>
                            </CardHeader>
                            <CardContent className="text-sm whitespace-pre-wrap">
                                {item.ai_answer}
                            </CardContent>
                        </Card>
                        {item.expected_answer && (
                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-base text-green-700">
                                        Expected (gold) answer
                                    </CardTitle>
                                </CardHeader>
                                <CardContent className="text-sm whitespace-pre-wrap">
                                    {item.expected_answer}
                                </CardContent>
                            </Card>
                        )}
                        {item.citations && (
                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-base">Citations</CardTitle>
                                </CardHeader>
                                <CardContent className="text-xs whitespace-pre-wrap font-mono">
                                    {item.citations}
                                </CardContent>
                            </Card>
                        )}
                    </div>

                    {/* Right: ratings + improvement + actions */}
                    <div className="space-y-4">
                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base">
                                    Rate this answer (1-5)
                                </CardTitle>
                            </CardHeader>
                            <CardContent className="space-y-3">
                                <RatingRow
                                    label="Accuracy"
                                    hint="(1 keys)"
                                    value={ratings.accuracy}
                                    onChange={(v) =>
                                        setRatings((r) => ({ ...r, accuracy: v }))
                                    }
                                />
                                <RatingRow
                                    label="Completeness"
                                    hint="(⇧+1)"
                                    value={ratings.completeness}
                                    onChange={(v) =>
                                        setRatings((r) => ({ ...r, completeness: v }))
                                    }
                                />
                                <RatingRow
                                    label="Relevance"
                                    hint="(⌥+1)"
                                    value={ratings.relevance}
                                    onChange={(v) =>
                                        setRatings((r) => ({ ...r, relevance: v }))
                                    }
                                />
                                <div className="flex items-center gap-3 pt-2 border-t">
                                    <span className="font-medium w-32">Safety</span>
                                    <Button
                                        variant={
                                            ratings.safety === 1 ? "default" : "outline"
                                        }
                                        size="sm"
                                        onClick={() =>
                                            setRatings((r) => ({ ...r, safety: 1 }))
                                        }
                                    >
                                        <Shield className="h-4 w-4 mr-1" /> Safe
                                    </Button>
                                    <Button
                                        variant={
                                            ratings.safety === 0
                                                ? "destructive"
                                                : "outline"
                                        }
                                        size="sm"
                                        onClick={() =>
                                            setRatings((r) => ({ ...r, safety: 0 }))
                                        }
                                    >
                                        <ShieldOff className="h-4 w-4 mr-1" /> Unsafe
                                    </Button>
                                    <span className="text-xs text-muted-foreground ml-auto">
                                        (S to toggle)
                                    </span>
                                </div>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base">
                                    Improved answer (optional)
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <textarea
                                    ref={improvedRef}
                                    value={improvedAnswer}
                                    onChange={(e) => setImprovedAnswer(e.target.value)}
                                    rows={6}
                                    className="w-full text-sm p-3 border rounded font-mono"
                                    placeholder="Edit the AI answer here. If unchanged, original is kept on export."
                                />
                                <div className="text-xs text-muted-foreground mt-1">
                                    Replaces ai_answer in JSONL export when you save.
                                </div>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader>
                                <CardTitle className="text-base">Tags</CardTitle>
                            </CardHeader>
                            <CardContent className="space-y-3">
                                <div>
                                    <label className="text-sm font-medium">
                                        Specialty (primary)
                                    </label>
                                    <Select
                                        value={specialty}
                                        onValueChange={(v) => setSpecialty(v)}
                                    >
                                        <SelectTrigger>
                                            <SelectValue placeholder="Select specialty…" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {SPECIALTIES.map((s) => (
                                                <SelectItem key={s} value={s}>
                                                    {s}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>
                                <TagsInput
                                    tags={tags}
                                    onChange={setTags}
                                    inputValue={tagInput}
                                    onInputChange={setTagInput}
                                />
                                <div>
                                    <label className="text-sm font-medium">
                                        Reviewer notes
                                    </label>
                                    <textarea
                                        value={notes}
                                        onChange={(e) => setNotes(e.target.value)}
                                        rows={2}
                                        className="w-full text-sm p-2 border rounded"
                                        placeholder="Optional: why approved/rejected, edge cases, etc."
                                    />
                                </div>
                            </CardContent>
                        </Card>

                        {/* Actions */}
                        <div className="flex gap-2 sticky bottom-4 bg-background py-2">
                            <Button
                                onClick={() => submit("APPROVED")}
                                disabled={submitting}
                                className="flex-1"
                            >
                                {submitting ? (
                                    <Loader2 className="h-4 w-4 animate-spin mr-2" />
                                ) : (
                                    <Check className="h-4 w-4 mr-2" />
                                )}
                                Approve (Enter)
                            </Button>
                            <Button
                                variant="outline"
                                onClick={() => submit("FLAGGED")}
                                disabled={submitting}
                            >
                                <Flag className="h-4 w-4 mr-1" /> Flag (F)
                            </Button>
                            <Button
                                variant="destructive"
                                onClick={() => submit("REJECTED")}
                                disabled={submitting}
                            >
                                <X className="h-4 w-4 mr-1" /> Reject (R)
                            </Button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

function TagsInput({
    tags,
    onChange,
    inputValue,
    onInputChange,
}: {
    tags: string[];
    onChange: (next: string[]) => void;
    inputValue: string;
    onInputChange: (v: string) => void;
}) {
    function addTag(raw: string) {
        const t = raw.trim().toLowerCase().replace(/\s+/g, "-");
        if (!t) return;
        if (tags.includes(t)) return;
        onChange([...tags, t]);
        onInputChange("");
    }
    const suggestions = COMMON_TAGS.filter(
        (s) => !tags.includes(s) && (!inputValue || s.includes(inputValue.toLowerCase()))
    ).slice(0, 6);

    return (
        <div>
            <label className="text-sm font-medium">
                Tags (cross-cutting)
                <span className="text-xs text-muted-foreground ml-2">
                    e.g. pharmacy, geriatric, pregnancy
                </span>
            </label>
            <div className="flex flex-wrap gap-1 p-2 border rounded min-h-[44px] items-center">
                {tags.map((t) => (
                    <Badge
                        key={t}
                        variant="secondary"
                        className="cursor-pointer"
                        onClick={() => onChange(tags.filter((x) => x !== t))}
                        title="Click to remove"
                    >
                        {t}
                        <span className="ml-1">×</span>
                    </Badge>
                ))}
                <input
                    type="text"
                    value={inputValue}
                    onChange={(e) => onInputChange(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === ",") {
                            e.preventDefault();
                            addTag(inputValue);
                        } else if (
                            e.key === "Backspace" &&
                            !inputValue &&
                            tags.length > 0
                        ) {
                            onChange(tags.slice(0, -1));
                        }
                    }}
                    className="flex-1 min-w-[120px] border-0 outline-none text-sm bg-transparent"
                    placeholder={tags.length === 0 ? "Type to add tag…" : ""}
                />
            </div>
            {suggestions.length > 0 && (
                <div className="flex flex-wrap gap-1 mt-2">
                    <span className="text-xs text-muted-foreground self-center mr-1">
                        Suggestions:
                    </span>
                    {suggestions.map((s) => (
                        <Badge
                            key={s}
                            variant="outline"
                            className="cursor-pointer hover:bg-accent"
                            onClick={() => addTag(s)}
                        >
                            + {s}
                        </Badge>
                    ))}
                </div>
            )}
        </div>
    );
}

function RatingRow({
    label,
    hint,
    value,
    onChange,
}: {
    label: string;
    hint: string;
    value: number | null;
    onChange: (v: number) => void;
}) {
    return (
        <div className="flex items-center gap-3">
            <span className="font-medium w-32">{label}</span>
            <div className="flex gap-1">
                {[1, 2, 3, 4, 5].map((n) => (
                    <Button
                        key={n}
                        size="sm"
                        variant={value === n ? "default" : "outline"}
                        className="w-9 h-9 p-0"
                        onClick={() => onChange(n)}
                    >
                        {n}
                    </Button>
                ))}
            </div>
            <span className="text-xs text-muted-foreground ml-auto">{hint}</span>
        </div>
    );
}
