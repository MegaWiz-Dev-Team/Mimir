"use client";

import { useState, useEffect, useRef } from "react";
import { usePathname } from "next/navigation";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetFooter } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { MessageSquarePlus, Bug, Lightbulb, Sparkles, Send, CheckCircle2, Loader2, BotMessageSquare } from "lucide-react";
import { submitFeedbackReport, FeedbackRequest, authFetch, API_BASE_URL } from "@/lib/api";
import { cn } from "@/lib/utils";

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

interface ChatMsg {
    role: "assistant" | "user" | "system";
    content: string;
}

export function FeedbackButton() {
    const pathname = usePathname();
    const [open, setOpen] = useState(false);
    const [activeTab, setActiveTab] = useState("chat");

    // Chat State
    const [chatHistory, setChatHistory] = useState<ChatMsg[]>([
        { role: "assistant", content: "Hi! I'm the Mimir Assistant. Need help using the platform or understanding metrics? Ask me anything!" }
    ]);
    const [chatInput, setChatInput] = useState("");
    const [chatSending, setChatSending] = useState(false);
    const scrollRef = useRef<HTMLDivElement>(null);

    // Form State
    const [reportType, setReportType] = useState<ReportType>("bug");
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [priority, setPriority] = useState<Priority>("medium");
    const [submitting, setSubmitting] = useState(false);
    const [submitted, setSubmitted] = useState(false);
    const [githubUrl, setGithubUrl] = useState<string | null>(null);
    const [mounted, setMounted] = useState(false);

    useEffect(() => { setMounted(true); }, []);
    useEffect(() => {
        if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [chatHistory]);

    if (!mounted || pathname === "/login") return null;

    const reset = () => {
        setReportType("bug");
        setTitle("");
        setDescription("");
        setPriority("medium");
        setSubmitted(false);
        setGithubUrl(null);
    };

    const handleSendChat = async () => {
        if (!chatInput.trim() || chatSending) return;
        const msg = chatInput.trim();
        setChatInput("");
        setChatHistory(curr => [...curr, { role: "user", content: msg }, { role: "system", content: "typing" }]);
        setChatSending(true);

        try {
            const apiOrigin = API_BASE_URL.replace(/\/api\/v1$/, "");
            const payload = {
                message: msg,
                history: chatHistory.filter(m => m.role !== "system"),
                current_page: pathname
            };

            const resp = await authFetch(`${apiOrigin}/api/v1/assistant/help`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(payload)
            });

            if (resp.ok) {
                const data = await resp.json();
                setChatHistory(curr => {
                    const newChat = [...curr];
                    newChat[newChat.length - 1] = { role: "assistant", content: data.reply };
                    return newChat;
                });
            } else {
                setChatHistory(curr => {
                    const newChat = [...curr];
                    newChat[newChat.length - 1] = { role: "assistant", content: "Network err communicating with Overseer." };
                    return newChat;
                });
            }
        } catch(e) {
            setChatHistory(curr => {
                const newChat = [...curr];
                newChat[newChat.length - 1] = { role: "assistant", content: "Connection failed to Assistant API." };
                return newChat;
            });
        } finally {
            setChatSending(false);
        }
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
                className="fixed bottom-6 right-6 z-50 w-12 h-12 rounded-full bg-gradient-to-br from-purple-600 to-indigo-600 text-white shadow-lg shadow-purple-500/20 hover:shadow-xl hover:shadow-purple-500/40 hover:scale-110 transition-all flex items-center justify-center group"
                title="Mimir Assistant & Feedback"
            >
                <BotMessageSquare className="w-6 h-6 group-hover:rotate-12 transition-transform" />
            </button>

            {/* AI Assistant & Feedback Sheet */}
            <Sheet open={open} onOpenChange={(o) => { if (!o) { reset(); } setOpen(o); }}>
                <SheetContent className="sm:max-w-lg flex flex-col p-0">
                    <SheetHeader className="px-6 pt-6 pb-2 border-b">
                        <SheetTitle className="flex items-center gap-2 text-indigo-600">
                            <BotMessageSquare className="w-5 h-5" />
                            Mimir Assistant
                        </SheetTitle>
                        <SheetDescription>
                            Get intelligent help or report bugs manually.
                        </SheetDescription>
                        <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full mt-4">
                            <TabsList className="grid w-full grid-cols-2">
                                <TabsTrigger value="chat">✨ Ask AI Help</TabsTrigger>
                                <TabsTrigger value="form">Submit Feedback</TabsTrigger>
                            </TabsList>
                        </Tabs>
                    </SheetHeader>

                    {activeTab === "chat" && (
                        <div className="flex-1 flex flex-col overflow-hidden bg-muted/10">
                            <div className="flex-1 overflow-y-auto p-6 space-y-4" ref={scrollRef}>
                                {chatHistory.map((msg, i) => (
                                    <div key={i} className={cn(
                                        "text-sm max-w-[85%] rounded-2xl px-4 py-2.5 shadow-sm leading-relaxed", 
                                        msg.role === "user" ? "ml-auto bg-indigo-600 text-white" 
                                        : msg.role === "system" ? "mr-auto bg-transparent italic text-muted-foreground shadow-none" 
                                        : "mr-auto bg-white border border-indigo-100 dark:bg-zinc-900"
                                    )}>
                                        {msg.role === "system" ? (
                                            <span className="flex items-center gap-2 text-indigo-500"><Loader2 className="w-3 h-3 animate-spin"/> Assistant is typing...</span>
                                        ) : msg.content}
                                    </div>
                                ))}
                            </div>
                            <div className="p-4 bg-background border-t">
                                <form onSubmit={(e) => { e.preventDefault(); handleSendChat(); }} className="flex gap-2">
                                    <Input 
                                        className="flex-1 rounded-full px-4 border-indigo-200 focus-visible:ring-indigo-500" 
                                        placeholder="Ask a question..." 
                                        value={chatInput} 
                                        onChange={(e) => setChatInput(e.target.value)} 
                                        disabled={chatSending} 
                                    />
                                    <Button size="icon" type="submit" disabled={chatSending || !chatInput.trim()} className="rounded-full bg-indigo-600 hover:bg-indigo-700">
                                        <Send className="w-4 h-4 text-white -ml-0.5" />
                                    </Button>
                                </form>
                            </div>
                        </div>
                    )}

                    {activeTab === "form" && (
                        <>
                            <div className="px-6 py-4 space-y-5 flex-1 overflow-y-auto">
                                {submitted ? (
                                    <div className="text-center py-10 space-y-4">
                                        <div className="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto">
                                            <CheckCircle2 className="w-8 h-8 text-green-600" />
                                        </div>
                                        <div>
                                            <p className="font-semibold text-lg">Thank you!</p>
                                            <p className="text-sm text-muted-foreground mt-1">Your feedback has been submitted successfully.</p>
                                        </div>
                                        {githubUrl && (
                                            <Button variant="outline" asChild className="mt-4">
                                                <a href={githubUrl} target="_blank" rel="noopener noreferrer">
                                                    View GitHub Issue
                                                </a>
                                            </Button>
                                        )}
                                    </div>
                                ) : (
                                    <>
                                        {/* Report Type */}
                                        <div className="space-y-3">
                                            <Label className="text-sm font-medium">Type</Label>
                                            <div className="grid grid-cols-3 gap-2">
                                                {REPORT_TYPES.map((rt) => (
                                                    <button
                                                        key={rt.value}
                                                        type="button"
                                                        onClick={() => setReportType(rt.value)}
                                                        className={`p-3 rounded-xl border text-center text-sm transition-all flex flex-col items-center gap-2 ${
                                                            reportType === rt.value
                                                                ? "border-blue-500 bg-blue-50/50 dark:bg-blue-950/30 ring-1 ring-blue-500 shadow-sm"
                                                                : "bg-white hover:bg-muted/50 dark:bg-zinc-900"
                                                        }`}
                                                    >
                                                        <span className={rt.color}>{rt.icon}</span>
                                                        <span className="font-medium text-xs sm:text-sm">{rt.label}</span>
                                                    </button>
                                                ))}
                                            </div>
                                        </div>

                                        {/* Priority */}
                                        <div className="space-y-3">
                                            <Label className="text-sm font-medium block">Priority</Label>
                                            <div className="grid grid-cols-4 gap-2">
                                                {PRIORITIES.map((p) => (
                                                    <button
                                                        key={p.value}
                                                        type="button"
                                                        onClick={() => setPriority(p.value)}
                                                        className={`py-2 rounded-lg border text-xs text-center transition-all ${
                                                            priority === p.value
                                                                ? `${p.color} bg-muted font-medium ring-1 ring-current shadow-sm`
                                                                : "bg-white hover:bg-muted/50 dark:bg-zinc-900"
                                                        }`}
                                                    >
                                                        {p.label}
                                                    </button>
                                                ))}
                                            </div>
                                        </div>

                                        {/* Title */}
                                        <div className="space-y-2">
                                            <Label htmlFor="fb-title" className="text-sm font-medium">Title <span className="text-red-500">*</span></Label>
                                            <Input
                                                id="fb-title"
                                                value={title}
                                                onChange={(e) => setTitle(e.target.value)}
                                                placeholder="Brief summary of your report"
                                                className="bg-white dark:bg-zinc-900"
                                            />
                                        </div>

                                        {/* Description */}
                                        <div className="space-y-2">
                                            <Label htmlFor="fb-desc" className="text-sm font-medium">Description</Label>
                                            <textarea
                                                id="fb-desc"
                                                value={description}
                                                onChange={(e) => setDescription(e.target.value)}
                                                placeholder="Detailed explanation..."
                                                rows={4}
                                                className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-zinc-900 focus:ring-2 focus:ring-blue-500 outline-none resize-none"
                                            />
                                        </div>

                                        {/* Context tag */}
                                        <div className="text-[11px] text-muted-foreground bg-muted/40 rounded-lg py-2 px-3 flex items-center">
                                            <span className="mr-1">📍</span> Page context (<code className="font-mono bg-background border px-1 rounded">{pathname}</code>) will be attached automatically.
                                        </div>
                                    </>
                                )}
                            </div>

                            <SheetFooter className="px-6 py-4 border-t bg-muted/10">
                                {!submitted ? (
                                    <Button onClick={handleSubmit} disabled={submitting || !title.trim()} className="w-full shadow-sm">
                                        {submitting ? (
                                            <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Submitting...</>
                                        ) : (
                                            <><Send className="w-4 h-4 mr-2" /> Send to Engineering</>
                                        )}
                                    </Button>
                                ) : (
                                    <Button variant="outline" onClick={() => { reset(); setOpen(false); }} className="w-full shadow-sm bg-white">
                                        Close
                                    </Button>
                                )}
                            </SheetFooter>
                        </>
                    )}
                </SheetContent>
            </Sheet>
        </>
    );
}
