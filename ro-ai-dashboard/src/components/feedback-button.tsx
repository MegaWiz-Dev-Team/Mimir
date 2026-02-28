"use client";

import { useState, useEffect } from "react";
import { usePathname } from "next/navigation";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetFooter } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MessageSquarePlus, Bug, Lightbulb, Sparkles, Send, CheckCircle2, Loader2 } from "lucide-react";
import { submitFeedbackReport, FeedbackRequest } from "@/lib/api";

type ReportType = "bug" | "feedback" | "feature";
type Priority = "critical" | "high" | "medium" | "low";

const REPORT_TYPES: { value: ReportType; label: string; icon: React.ReactNode; color: string }[] = [
    { value: "bug", label: "Bug Report", icon: <Bug className="w-4 h-4" />, color: "text-red-500" },
    { value: "feedback", label: "Feedback", icon: <Lightbulb className="w-4 h-4" />, color: "text-amber-500" },
    { value: "feature", label: "Feature Request", icon: <Sparkles className="w-4 h-4" />, color: "text-blue-500" },
];

const PRIORITIES: { value: Priority; label: string; color: string }[] = [
    { value: "critical", label: "🔴 Critical", color: "border-red-500" },
    { value: "high", label: "🟠 High", color: "border-orange-500" },
    { value: "medium", label: "🟡 Medium", color: "border-yellow-500" },
    { value: "low", label: "🟢 Low", color: "border-green-500" },
];

export function FeedbackButton() {
    const pathname = usePathname();
    const [open, setOpen] = useState(false);
    const [reportType, setReportType] = useState<ReportType>("bug");
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [priority, setPriority] = useState<Priority>("medium");
    const [submitting, setSubmitting] = useState(false);
    const [submitted, setSubmitted] = useState(false);
    const [githubUrl, setGithubUrl] = useState<string | null>(null);
    const [mounted, setMounted] = useState(false);

    useEffect(() => { setMounted(true); }, []);

    if (!mounted || pathname === "/login") return null;

    const reset = () => {
        setReportType("bug");
        setTitle("");
        setDescription("");
        setPriority("medium");
        setSubmitted(false);
        setGithubUrl(null);
    };

    const handleSubmit = async () => {
        if (!title.trim()) return;
        setSubmitting(true);
        try {
            const data: FeedbackRequest = {
                report_type: reportType,
                title,
                description: description || undefined,
                priority,
                page_url: pathname,
                browser_info: {
                    userAgent: navigator.userAgent,
                    language: navigator.language,
                    screen: `${screen.width}x${screen.height}`,
                },
            };
            const result = await submitFeedbackReport(data);
            setSubmitted(true);
            setGithubUrl(result.github_issue_url || null);
        } catch (error) {
            console.warn("[Feedback] Submit failed:", error);
            alert("Failed to submit feedback. Please try again.");
        } finally {
            setSubmitting(false);
        }
    };

    return (
        <>
            {/* Floating Action Button */}
            <button
                onClick={() => { reset(); setOpen(true); }}
                data-testid="feedback-fab"
                className="fixed bottom-6 right-6 z-50 w-12 h-12 rounded-full bg-gradient-to-br from-blue-600 to-indigo-600 text-white shadow-lg hover:shadow-xl hover:scale-110 transition-all flex items-center justify-center group"
                title="Send Feedback"
            >
                <MessageSquarePlus className="w-5 h-5 group-hover:rotate-12 transition-transform" />
            </button>

            {/* Feedback Sheet */}
            <Sheet open={open} onOpenChange={(o) => { if (!o) { reset(); } setOpen(o); }}>
                <SheetContent className="sm:max-w-md" data-testid="feedback-sheet">
                    <SheetHeader>
                        <SheetTitle className="flex items-center gap-2">
                            <MessageSquarePlus className="w-5 h-5 text-blue-500" />
                            Send Feedback
                        </SheetTitle>
                        <SheetDescription>
                            Report bugs, share ideas, or request features. Auto-creates a GitHub issue.
                        </SheetDescription>
                    </SheetHeader>

                    <div className="px-6 py-4 space-y-4 flex-1 overflow-y-auto">
                        {submitted ? (
                            <div className="text-center py-8 space-y-3">
                                <CheckCircle2 className="w-12 h-12 text-green-500 mx-auto" />
                                <p className="font-medium text-lg">Thank you!</p>
                                <p className="text-sm text-muted-foreground">Your feedback has been submitted.</p>
                                {githubUrl && (
                                    <a
                                        href={githubUrl}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="text-sm text-blue-500 hover:underline inline-block"
                                    >
                                        View GitHub Issue →
                                    </a>
                                )}
                            </div>
                        ) : (
                            <>
                                {/* Report Type */}
                                <div>
                                    <Label className="text-sm font-medium mb-2 block">Type</Label>
                                    <div className="grid grid-cols-3 gap-2">
                                        {REPORT_TYPES.map((rt) => (
                                            <button
                                                key={rt.value}
                                                type="button"
                                                data-testid={`report-type-${rt.value}`}
                                                onClick={() => setReportType(rt.value)}
                                                className={`p-2.5 rounded-lg border text-center text-sm transition-all flex flex-col items-center gap-1.5 ${reportType === rt.value
                                                        ? "border-blue-500 bg-blue-50 dark:bg-blue-950/30 ring-1 ring-blue-500"
                                                        : "hover:bg-muted/50"
                                                    }`}
                                            >
                                                <span className={rt.color}>{rt.icon}</span>
                                                <span className="font-medium">{rt.label}</span>
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                {/* Title */}
                                <div className="grid gap-1.5">
                                    <Label htmlFor="fb-title">Title *</Label>
                                    <Input
                                        id="fb-title"
                                        data-testid="feedback-title"
                                        value={title}
                                        onChange={(e) => setTitle(e.target.value)}
                                        placeholder="Brief summary of your report"
                                    />
                                </div>

                                {/* Description */}
                                <div className="grid gap-1.5">
                                    <Label htmlFor="fb-desc">Description</Label>
                                    <textarea
                                        id="fb-desc"
                                        data-testid="feedback-description"
                                        value={description}
                                        onChange={(e) => setDescription(e.target.value)}
                                        placeholder="Detailed explanation..."
                                        rows={4}
                                        className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-zinc-900 dark:border-zinc-700 focus:ring-2 focus:ring-blue-500 outline-none resize-none"
                                    />
                                </div>

                                {/* Priority */}
                                <div>
                                    <Label className="text-sm font-medium mb-2 block">Priority</Label>
                                    <div className="grid grid-cols-4 gap-1.5">
                                        {PRIORITIES.map((p) => (
                                            <button
                                                key={p.value}
                                                type="button"
                                                data-testid={`priority-${p.value}`}
                                                onClick={() => setPriority(p.value)}
                                                className={`px-2 py-1.5 rounded border text-xs text-center transition-all ${priority === p.value
                                                        ? `${p.color} bg-muted font-medium ring-1 ring-current`
                                                        : "hover:bg-muted/50"
                                                    }`}
                                            >
                                                {p.label}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                {/* Auto-captured info badge */}
                                <div className="text-xs text-muted-foreground bg-muted/30 rounded-md p-2">
                                    📍 Page: <code className="text-foreground">{pathname}</code> • Browser info auto-captured
                                </div>
                            </>
                        )}
                    </div>

                    <SheetFooter className="px-6 py-4 border-t">
                        {!submitted ? (
                            <Button onClick={handleSubmit} disabled={submitting || !title.trim()} className="w-full">
                                {submitting ? (
                                    <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Submitting...</>
                                ) : (
                                    <><Send className="w-4 h-4 mr-2" /> Submit Feedback</>
                                )}
                            </Button>
                        ) : (
                            <Button variant="outline" onClick={() => { reset(); setOpen(false); }} className="w-full">
                                Close
                            </Button>
                        )}
                    </SheetFooter>
                </SheetContent>
            </Sheet>
        </>
    );
}
