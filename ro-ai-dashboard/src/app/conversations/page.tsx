"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
    fetchConversations,
    getConversation,
    submitFeedback,
    fetchConversationStats,
    ConversationMessage,
} from "@/lib/api";
import {
    MessageSquare, Clock, ArrowLeft, ThumbsUp, ThumbsDown,
    Loader2, Search, Hash, Bot, User, ChevronRight, BarChart3,
} from "lucide-react";

interface Session {
    session_id: string;
    agent_config_id?: number;
    agent_name?: string;
    source?: string;
    message_count: number;
    first_message_at?: string;
    last_message_at?: string;
}

interface ConvStats {
    total_sessions: number;
    total_messages: number;
    avg_messages_per_session: number;
    thumbs_up: number;
    thumbs_down: number;
}

export default function ConversationsPage() {
    const [sessions, setSessions] = useState<Session[]>([]);
    const [stats, setStats] = useState<ConvStats | null>(null);
    const [transcript, setTranscript] = useState<ConversationMessage[]>([]);
    const [selectedSession, setSelectedSession] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);
    const [loadingTranscript, setLoadingTranscript] = useState(false);
    const [search, setSearch] = useState("");
    const [page, setPage] = useState(1);

    const loadSessions = useCallback(async () => {
        try {
            setLoading(true);
            const data = await fetchConversations({ page, per_page: 20 });
            setSessions(data.sessions || data || []);
        } catch {
            setSessions([]);
        } finally {
            setLoading(false);
        }
    }, [page]);

    useEffect(() => {
        loadSessions();
        fetchConversationStats()
            .then(setStats)
            .catch(() => setStats(null));
    }, [loadSessions]);

    const openTranscript = async (sessionId: string) => {
        setSelectedSession(sessionId);
        setLoadingTranscript(true);
        try {
            const data = await getConversation(sessionId);
            setTranscript(data.messages || data || []);
        } catch {
            setTranscript([]);
        } finally {
            setLoadingTranscript(false);
        }
    };

    const handleFeedback = async (msgId: number, fb: "thumbs_up" | "thumbs_down") => {
        try {
            await submitFeedback(msgId, fb);
            // Refresh transcript
            if (selectedSession) openTranscript(selectedSession);
        } catch (e) {
            console.error("Feedback failed:", e);
        }
    };

    const filteredSessions = sessions.filter(s => {
        if (!search) return true;
        const q = search.toLowerCase();
        return (
            s.session_id.toLowerCase().includes(q) ||
            (s.agent_name || "").toLowerCase().includes(q) ||
            (s.source || "").toLowerCase().includes(q)
        );
    });

    const formatDate = (d?: string) => {
        if (!d) return "-";
        return new Date(d).toLocaleString("en-US", {
            month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
        });
    };

    // --- Transcript View ---
    if (selectedSession) {
        const session = sessions.find(s => s.session_id === selectedSession);
        return (
            <div className="container mx-auto p-6 space-y-4">
                <div className="flex items-center gap-3">
                    <Button variant="ghost" onClick={() => setSelectedSession(null)}>
                        <ArrowLeft className="w-4 h-4 mr-1" /> Back
                    </Button>
                    <div>
                        <h1 className="text-xl font-bold">Conversation Transcript</h1>
                        <p className="text-xs text-gray-500 font-mono">{selectedSession}</p>
                    </div>
                    {session && (
                        <Badge variant="outline" className="ml-auto">
                            {session.agent_name || session.source || "Playground"}
                        </Badge>
                    )}
                </div>

                {loadingTranscript ? (
                    <div className="flex justify-center py-20">
                        <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
                    </div>
                ) : (
                    <Card>
                        <CardContent className="pt-6 space-y-4">
                            {transcript.map((msg, i) => (
                                <div key={i} className={`flex gap-3 ${msg.role === "user" ? "" : ""}`}>
                                    <div className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${msg.role === "user"
                                            ? "bg-blue-100 text-blue-600 dark:bg-blue-900/40 dark:text-blue-400"
                                            : "bg-purple-100 text-purple-600 dark:bg-purple-900/40 dark:text-purple-400"
                                        }`}>
                                        {msg.role === "user" ? <User className="w-4 h-4" /> : <Bot className="w-4 h-4" />}
                                    </div>
                                    <div className="flex-1">
                                        <div className="flex items-center gap-2 mb-1">
                                            <span className="text-sm font-medium capitalize">{msg.role}</span>
                                            {msg.model_id && (
                                                <Badge variant="secondary" className="text-[10px]">{msg.model_id}</Badge>
                                            )}
                                            {msg.latency_ms && (
                                                <span className="text-[10px] text-gray-400 flex items-center gap-1">
                                                    <Clock className="w-3 h-3" /> {msg.latency_ms}ms
                                                </span>
                                            )}
                                            <span className="text-[10px] text-gray-400 ml-auto">
                                                {formatDate(msg.created_at)}
                                            </span>
                                        </div>
                                        <div className="text-sm text-gray-700 dark:text-zinc-300 whitespace-pre-wrap bg-gray-50 dark:bg-zinc-800/50 rounded-lg px-3 py-2">
                                            {msg.content}
                                        </div>
                                        {msg.role === "assistant" && (
                                            <div className="flex items-center gap-2 mt-1.5">
                                                {msg.input_tokens && msg.output_tokens && (
                                                    <span className="text-[10px] text-gray-400 flex items-center gap-1">
                                                        <Hash className="w-3 h-3" /> {msg.input_tokens + msg.output_tokens} tokens
                                                    </span>
                                                )}
                                                <div className="flex gap-1 ml-auto">
                                                    <button
                                                        onClick={() => handleFeedback(msg.id, "thumbs_up")}
                                                        className={`p-1 rounded hover:bg-green-50 dark:hover:bg-green-900/20 ${msg.feedback === "thumbs_up" ? "text-green-500" : "text-gray-300"
                                                            }`}>
                                                        <ThumbsUp className="w-3.5 h-3.5" />
                                                    </button>
                                                    <button
                                                        onClick={() => handleFeedback(msg.id, "thumbs_down")}
                                                        className={`p-1 rounded hover:bg-red-50 dark:hover:bg-red-900/20 ${msg.feedback === "thumbs_down" ? "text-red-500" : "text-gray-300"
                                                            }`}>
                                                        <ThumbsDown className="w-3.5 h-3.5" />
                                                    </button>
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            ))}
                            {transcript.length === 0 && (
                                <p className="text-center text-gray-500 py-8">No messages in this conversation</p>
                            )}
                        </CardContent>
                    </Card>
                )}
            </div>
        );
    }

    // --- Session List View ---
    return (
        <div className="container mx-auto p-6 space-y-6">
            <div>
                <h1 className="text-3xl font-bold bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                    Conversation History
                </h1>
                <p className="text-gray-500 mt-1">Browse and review all AI conversations</p>
            </div>

            {/* Stats Cards */}
            {stats && (
                <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
                    <Card>
                        <CardContent className="pt-4 pb-3 text-center">
                            <p className="text-2xl font-bold">{stats.total_sessions}</p>
                            <p className="text-xs text-gray-500">Sessions</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardContent className="pt-4 pb-3 text-center">
                            <p className="text-2xl font-bold">{stats.total_messages}</p>
                            <p className="text-xs text-gray-500">Messages</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardContent className="pt-4 pb-3 text-center">
                            <p className="text-2xl font-bold">{stats.avg_messages_per_session?.toFixed(1) || "0"}</p>
                            <p className="text-xs text-gray-500">Avg / Session</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardContent className="pt-4 pb-3 text-center">
                            <p className="text-2xl font-bold text-green-600">{stats.thumbs_up}</p>
                            <p className="text-xs text-gray-500">👍 Positive</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardContent className="pt-4 pb-3 text-center">
                            <p className="text-2xl font-bold text-red-600">{stats.thumbs_down}</p>
                            <p className="text-xs text-gray-500">👎 Negative</p>
                        </CardContent>
                    </Card>
                </div>
            )}

            {/* Search */}
            <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
                <Input value={search} onChange={e => setSearch(e.target.value)}
                    placeholder="Search conversations..." className="pl-10" />
            </div>

            {/* Session list */}
            {loading ? (
                <div className="flex justify-center py-20">
                    <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
                </div>
            ) : filteredSessions.length === 0 ? (
                <Card>
                    <CardContent className="flex flex-col items-center py-16">
                        <MessageSquare className="w-16 h-16 text-gray-300 mb-4" />
                        <h3 className="text-xl font-semibold text-gray-700 dark:text-zinc-300">No conversations yet</h3>
                        <p className="text-gray-500 mt-2">Start a chat in the Playground or Agent Studio</p>
                    </CardContent>
                </Card>
            ) : (
                <div className="space-y-2">
                    {filteredSessions.map(session => (
                        <Card key={session.session_id}
                            onClick={() => openTranscript(session.session_id)}
                            className="cursor-pointer hover:shadow-md hover:border-blue-300 dark:hover:border-blue-700 transition-all">
                            <CardContent className="py-3 px-4 flex items-center gap-4">
                                <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-blue-500 to-indigo-500 flex items-center justify-center text-white flex-shrink-0">
                                    <MessageSquare className="w-4 h-4" />
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-2">
                                        <span className="font-medium text-sm truncate">
                                            {session.agent_name || session.source || "Playground Chat"}
                                        </span>
                                        <Badge variant="secondary" className="text-[10px]">
                                            {session.message_count} msgs
                                        </Badge>
                                    </div>
                                    <p className="text-xs text-gray-500 font-mono truncate">{session.session_id}</p>
                                </div>
                                <div className="text-right flex-shrink-0">
                                    <p className="text-xs text-gray-500">{formatDate(session.last_message_at)}</p>
                                    <p className="text-[10px] text-gray-400">Started {formatDate(session.first_message_at)}</p>
                                </div>
                                <ChevronRight className="w-4 h-4 text-gray-400 flex-shrink-0" />
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            {/* Pagination */}
            <div className="flex justify-center gap-2">
                <Button variant="outline" size="sm" disabled={page === 1}
                    onClick={() => setPage(p => Math.max(1, p - 1))}>
                    Previous
                </Button>
                <span className="text-sm text-gray-500 flex items-center">Page {page}</span>
                <Button variant="outline" size="sm"
                    onClick={() => setPage(p => p + 1)}>
                    Next
                </Button>
            </div>
        </div>
    );
}
