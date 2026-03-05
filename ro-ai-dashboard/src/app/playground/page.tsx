"use client";

import { useState, useRef, useEffect, useCallback, useMemo, Suspense } from "react";
import { useSearchParams } from "next/navigation";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import Link from "next/link";
import {
    PROVIDERS,
    Persona,
    ChatResponse,
    SourceCitation,
    StreamDone,
    LlmProvider,
    AgentConfigResponse,
    sendChat,
    streamChat,
    fetchModels,
    fetchPlaygroundAgents,
    modelsToProviders,
    updatePersonaConfig,
    fetchVectorStats,
} from "@/lib/api";
import { Send, Trash2, User, Bot, Loader2, AlertCircle, BookOpen, Database, Zap, Cloud, HardDrive, X, Save, CheckCircle2, Copy, Check, Brain } from "lucide-react";

interface Message {
    role: "user" | "assistant";
    content: string;
    latency_ms?: number;
    confidence_score?: number;
    confidence_level?: string;
    sources?: SourceCitation[];
    tools_used?: string[];
    streaming?: boolean;
    provider?: string;
    model?: string;
    action?: any;
}

function MarkdownMessage({ content }: { content: string }) {
    const remarkPlugins = useMemo(() => [remarkGfm], []);
    return (
        <div className="markdown-body">
            <ReactMarkdown remarkPlugins={remarkPlugins}>
                {content}
            </ReactMarkdown>
        </div>
    );
}

function TypingIndicator() {
    return (
        <div className="flex gap-1 items-center h-5 px-1 min-w-[40px]">
            <div className="typing-dot" />
            <div className="typing-dot" />
            <div className="typing-dot" />
        </div>
    );
}

function WikiModal({
    filename,
    content,
    onClose,
    loading
}: {
    filename: string;
    content: string;
    onClose: () => void;
    loading: boolean;
}) {
    if (!filename) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-200">
            <Card className="w-full max-w-3xl max-h-[80vh] flex flex-col shadow-2xl animate-in zoom-in-95 duration-200">
                <CardHeader className="flex flex-row items-center justify-between py-4 border-b">
                    <div className="flex items-center gap-2">
                        <BookOpen className="h-5 w-5 text-primary" />
                        <CardTitle className="text-lg font-bold truncate">{filename}</CardTitle>
                    </div>
                    <Button variant="ghost" size="icon" onClick={onClose} className="h-8 w-8">
                        <X className="h-4 w-4" />
                    </Button>
                </CardHeader>
                <CardContent className="flex-1 overflow-y-auto p-6">
                    {loading ? (
                        <div className="flex flex-col items-center justify-center py-20 gap-3">
                            <Loader2 className="h-8 w-8 animate-spin text-primary" />
                            <p className="text-sm text-muted-foreground">Loading document...</p>
                        </div>
                    ) : (
                        <div className="markdown-body prose dark:prose-invert max-w-none">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                {content}
                            </ReactMarkdown>
                        </div>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}

export default function PlaygroundPage() {
    return (
        <Suspense fallback={<div className="container mx-auto p-8 max-w-6xl"><div className="flex items-center gap-2"><Loader2 className="h-5 w-5 animate-spin" /><span>Loading Playground...</span></div></div>}>
            <PlaygroundContent />
        </Suspense>
    );
}

function PlaygroundContent() {
    const searchParams = useSearchParams();
    const agentParam = searchParams.get("agent");

    const [tier, setTier] = useState<1 | 2>(2);
    const [persona, setPersona] = useState<string>("");
    const [provider, setProvider] = useState<string>("ollama");
    const [model, setModel] = useState<string>("");
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState("");
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [useStreaming, setUseStreaming] = useState(true);
    const [providers, setProviders] = useState<LlmProvider[]>(PROVIDERS);
    const [modelsLoading, setModelsLoading] = useState(true);
    const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

    // Agent configs from API (DB-backed) — single source of truth
    const [agentConfigs, setAgentConfigs] = useState<AgentConfigResponse[]>([]);
    const [personas, setPersonas] = useState<Persona[]>([]);
    const [agentsLoading, setAgentsLoading] = useState(true);

    // Wiki Modal state
    const [viewingWiki, setViewingWiki] = useState<string | null>(null);
    const [wikiContent, setWikiContent] = useState<string>("");
    const [wikiLoading, setWikiLoading] = useState(false);

    // Vector stats state
    const [vectorStats, setVectorStats] = useState<any>(null);

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const abortRef = useRef<(() => void) | null>(null);

    // Get selected persona and provider details
    const selectedPersona = personas.find(p => p.name === persona) || personas[0];
    const selectedProvider = providers.find(p => p.id === provider) || providers[0];
    const selectedModel = selectedProvider?.models.find(m => m.id === model) || selectedProvider?.models[0];
    const selectedAgent = agentConfigs.find(a => a.name === persona);

    // Fetch agents from Agent Studio API on mount — DB is the single source of truth
    useEffect(() => {
        async function loadAgents() {
            try {
                const { personas: dbPersonas, agents } = await fetchPlaygroundAgents();
                setAgentConfigs(agents);
                setPersonas(dbPersonas);

                if (agents.length > 0) {
                    // Use ?agent= param if provided (deep-link from Agent Studio)
                    const targetAgent = agentParam
                        ? agents.find(a => a.name === agentParam) || agents[0]
                        : agents[0];
                    setPersona(targetAgent.name);
                    setTier((targetAgent.tier || 2) as 1 | 2);
                    if (targetAgent.provider) setProvider(targetAgent.provider);
                    if (targetAgent.model_id) setModel(targetAgent.model_id);
                    if (targetAgent.response_mode) setUseStreaming(targetAgent.response_mode === "streaming");
                }
            } catch (err) {
                console.error("Failed to fetch agents from Agent Studio:", err);
            } finally {
                setAgentsLoading(false);
            }
        }
        loadAgents();
    }, [agentParam]);

    // Fetch vector stats on mount
    useEffect(() => {
        async function loadStats() {
            try {
                const stats = await fetchVectorStats();
                setVectorStats(stats);
            } catch (err) {
                console.warn("Failed to fetch vector stats:", err);
            }
        }
        loadStats();
    }, []);

    // Fetch models from database on mount
    useEffect(() => {
        async function loadModels() {
            try {
                const models = await fetchModels();
                if (models.length > 0) {
                    const dynamicProviders = modelsToProviders(models);
                    setProviders(dynamicProviders);
                    // Set default provider and model
                    if (dynamicProviders.length > 0) {
                        setProvider(dynamicProviders[0].id);
                        if (dynamicProviders[0].models.length > 0) {
                            setModel(dynamicProviders[0].models[0].id);
                        }
                    }
                } else {
                    // Use fallback PROVIDERS
                    if (PROVIDERS.length > 0 && PROVIDERS[0].models.length > 0) {
                        setProvider(PROVIDERS[0].id);
                        setModel(PROVIDERS[0].models[0].id);
                    }
                }
            } catch (err) {
                console.warn("Failed to fetch models from DB, using fallback:", err);
                // Use fallback PROVIDERS
                if (PROVIDERS.length > 0 && PROVIDERS[0].models.length > 0) {
                    setProvider(PROVIDERS[0].id);
                    setModel(PROVIDERS[0].models[0].id);
                }
            } finally {
                setModelsLoading(false);
            }
        }
        loadModels();
    }, []);

    // Update model when provider changes
    useEffect(() => {
        const newProvider = providers.find(p => p.id === provider);
        if (newProvider && newProvider.models.length > 0) {
            if (!newProvider.models.find(m => m.id === model)) {
                setModel(newProvider.models[0].id);
            }
        }
    }, [provider, model, providers]);

    // Auto-scroll to bottom when messages change
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [messages]);

    // Auto-fill settings when persona/agent changes
    useEffect(() => {
        if (!selectedPersona) return; // Guard: personas not loaded yet

        const agent = agentConfigs.find(a => a.name === persona);
        if (agent) {
            // Auto-fill from DB-backed agent config
            setTier((agent.tier || 2) as 1 | 2);
            if (agent.provider) {
                setProvider(agent.provider);
            }
            if (agent.model_id) {
                setModel(agent.model_id);
            }
            if (agent.response_mode) {
                setUseStreaming(agent.response_mode === "streaming");
            }
        }

        if (messages.length === 0 && selectedPersona.greeting) {
            setMessages([{
                role: "assistant",
                content: selectedPersona.greeting,
            }]);
        }
    }, [persona, selectedPersona?.name, selectedPersona?.greeting, agentConfigs]);

    // Cleanup streaming on unmount
    useEffect(() => {
        return () => {
            if (abortRef.current) {
                abortRef.current();
            }
        };
    }, []);
    const handleOpenWiki = useCallback(async (filename: string) => {
        setViewingWiki(filename);
        setWikiLoading(true);
        setWikiContent("");

        try {
            const resp = await fetch(`http://localhost:8080/api/wiki/${filename}`);
            if (resp.ok) {
                const text = await resp.text();
                setWikiContent(text);
            } else {
                setWikiContent("Failed to load document content.");
            }
        } catch (err) {
            setWikiContent("Error connecting to server.");
        } finally {
            setWikiLoading(false);
        }
    }, []);

    const handleSend = useCallback(async () => {
        if (!input.trim() || loading) return;

        const userMessage = input.trim();
        setInput("");
        setError(null);

        // Add user message
        setMessages(prev => [...prev, { role: "user", content: userMessage }]);
        setLoading(true);

        try {
            if (useStreaming) {
                // Streaming mode
                setMessages(prev => [...prev, {
                    role: "assistant",
                    content: "",
                    streaming: true,
                    provider,
                    model,
                }]);

                abortRef.current = streamChat(
                    { tier, message: userMessage, persona, provider, model },
                    (token) => {
                        // On token
                        setMessages(prev => {
                            const newMessages = [...prev];
                            const lastIndex = newMessages.length - 1;
                            const lastMessage = newMessages[lastIndex];
                            if (lastMessage.role === "assistant") {
                                // Important: deeply clone the object to avoid React Strict Mode double-mutation bugs
                                newMessages[lastIndex] = {
                                    ...lastMessage,
                                    content: lastMessage.content + token,
                                };
                            }
                            return newMessages;
                        });
                    },
                    (metadata: StreamDone) => {
                        // On done
                        setMessages(prev => {
                            const newMessages = [...prev];
                            const lastMessage = newMessages[newMessages.length - 1];
                            if (lastMessage.role === "assistant") {
                                lastMessage.streaming = false;
                                lastMessage.latency_ms = metadata.latency_ms;
                                lastMessage.confidence_score = metadata.confidence_score;
                                lastMessage.confidence_level = metadata.confidence_level;
                                lastMessage.sources = metadata.sources;
                                lastMessage.action = metadata.action;
                            }
                            return newMessages;
                        });
                        setLoading(false);
                    },
                    (errorMsg) => {
                        // On error
                        setError(errorMsg);
                        setMessages(prev => {
                            const newMessages = [...prev];
                            const lastMessage = newMessages[newMessages.length - 1];
                            if (lastMessage.role === "assistant") {
                                lastMessage.streaming = false;
                                lastMessage.content = `Error: ${errorMsg}`;
                            }
                            return newMessages;
                        });
                        setLoading(false);
                    }
                );
            } else {
                // Non-streaming mode
                const response: ChatResponse = await sendChat({
                    tier,
                    message: userMessage,
                    persona,
                    provider,
                    model,
                });

                setMessages(prev => [...prev, {
                    role: "assistant",
                    content: response.content,
                    latency_ms: response.latency_ms,
                    confidence_score: response.confidence_score,
                    confidence_level: response.confidence_level,
                    sources: response.sources,
                    tools_used: response.tools_used,
                    provider: response.provider,
                    model: response.model,
                    action: response.action,
                }]);
                setLoading(false);
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : "Unknown error");
            setLoading(false);
        }
    }, [input, loading, tier, persona, provider, model, useStreaming]);

    const handleClear = () => {
        setMessages([{
            role: "assistant",
            content: selectedPersona.greeting,
        }]);
        setError(null);
    };

    const handleSaveConfig = async () => {
        try {
            setLoading(true);
            await updatePersonaConfig(persona, model);
            alert(`Successfully updated Persona '${selectedPersona.display_name}' to use model '${model}'.`);
            setLoading(false);
        } catch (err) {
            setError(err instanceof Error ? err.message : "Failed to save config");
            setLoading(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const getConfidenceColor = (level?: string) => {
        switch (level) {
            case "High": return "bg-green-500";
            case "Medium": return "bg-yellow-500";
            case "Low": return "bg-red-500";
            default: return "bg-gray-500";
        }
    };

    const getProviderIcon = (providerId?: string) => {
        switch (providerId) {
            case "ollama": return <HardDrive className="h-3 w-3" />;
            case "google": return <Cloud className="h-3 w-3" />;
            case "gemini": return <Cloud className="h-3 w-3" />;
            default: return null;
        }
    };

    const getProviderIconLarge = (providerId: string) => {
        switch (providerId) {
            case "ollama": return <HardDrive className="h-4 w-4 text-green-500" />;
            case "google":
            case "gemini": return <Cloud className="h-4 w-4 text-blue-500" />;
            default: return <Database className="h-4 w-4 text-gray-500" />;
        }
    };

    const handleCopyLog = (index: number) => {
        const assistantMsg = messages[index];
        if (assistantMsg.role !== "assistant") return;

        let userMsgContent = "";
        if (index > 0 && messages[index - 1].role === "user") {
            userMsgContent = messages[index - 1].content;
        }

        const formattedLog = `=== Project Mimir AI Debug Log ===\n` +
            `Time: ${new Date().toISOString()}\n` +
            `Persona: ${selectedPersona?.name} (Tier ${tier})\n` +
            `Model: ${assistantMsg.provider || provider} / ${assistantMsg.model || model}\n` +
            `Latency: ${assistantMsg.latency_ms || 0}ms\n` +
            `Confidence: ${assistantMsg.confidence_level || 'N/A'} (${assistantMsg.confidence_score || 0})\n` +
            (assistantMsg.action ? `Action: ${JSON.stringify(assistantMsg.action)}\n` : '') +
            `\n[User Query]\n${userMsgContent}\n` +
            `\n[Assistant Response]\n${assistantMsg.content}\n` +
            (assistantMsg.sources && assistantMsg.sources.length > 0 ? `\n[Sources]\n${assistantMsg.sources.map(s => `- ${s.source_id} (${s.relevance})`).join('\n')}\n` : '');

        navigator.clipboard.writeText(formattedLog).then(() => {
            setCopiedIndex(index);
            setTimeout(() => setCopiedIndex(null), 2000);
        });
    };

    return (
        <div className="container mx-auto p-8 max-w-6xl">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Agent Playground</h1>
                    <p className="text-muted-foreground">
                        {agentsLoading
                            ? "Loading agents from Agent Studio..."
                            : agentConfigs.length > 0
                                ? `${agentConfigs.length} agents loaded from Agent Studio`
                                : "No agents found — create agents in Agent Studio first"}
                    </p>
                </div>
                <div className="flex gap-2">
                    <Link href="/agents">
                        <Button variant="outline" size="sm">
                            <Brain className="mr-2 h-4 w-4" />
                            Agent Studio
                        </Button>
                    </Link>
                    <Button variant="outline" onClick={handleClear}>
                        <Trash2 className="mr-2 h-4 w-4" />
                        Clear Chat
                    </Button>
                </div>
            </div>

            {/* Empty state: no agents in DB */}
            {!agentsLoading && personas.length === 0 && (
                <Card className="border-dashed">
                    <CardContent className="flex flex-col items-center justify-center py-16 text-center">
                        <Brain className="h-12 w-12 text-muted-foreground/50 mb-4" />
                        <h3 className="text-lg font-semibold mb-2">No Agents Available</h3>
                        <p className="text-muted-foreground max-w-md mb-6">
                            Create agents in Agent Studio first, then come back here to test them in the Playground.
                        </p>
                        <Link href="/agents">
                            <Button>
                                <Brain className="mr-2 h-4 w-4" />
                                Go to Agent Studio
                            </Button>
                        </Link>
                    </CardContent>
                </Card>
            )}

            {personas.length > 0 && (
                <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
                    {/* Settings Panel */}
                    <Card className="lg:col-span-1">
                        <CardHeader>
                            <CardTitle className="text-lg">Settings</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            {/* Provider Selector */}
                            <div className="space-y-2">
                                <Label>LLM Provider</Label>
                                <Select value={provider} onValueChange={setProvider} disabled={modelsLoading}>
                                    <SelectTrigger>
                                        <SelectValue placeholder={modelsLoading ? "Loading..." : "Select provider"} />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {providers.map((p) => (
                                            <SelectItem key={p.id} value={p.id}>
                                                <div className="flex items-center gap-2">
                                                    {getProviderIconLarge(p.id)}
                                                    {p.display_name}
                                                </div>
                                            </SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                                <p className="text-xs text-muted-foreground">
                                    {selectedProvider?.description}
                                </p>
                            </div>

                            {/* Model Selector */}
                            <div className="space-y-2">
                                <Label>Model</Label>
                                <Select value={model} onValueChange={setModel} disabled={modelsLoading || !selectedProvider?.models.length}>
                                    <SelectTrigger>
                                        <SelectValue placeholder={modelsLoading ? "Loading..." : "Select model"} />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {selectedProvider?.models.map((m) => (
                                            <SelectItem key={m.id} value={m.id}>
                                                {m.display_name}
                                            </SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                                <p className="text-xs text-muted-foreground mb-2">
                                    {selectedModel?.description}
                                </p>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="w-full mt-2"
                                    onClick={handleSaveConfig}
                                    disabled={loading || !model}
                                >
                                    <Save className="mr-2 h-4 w-4" /> Save Default Model For NPC
                                </Button>
                            </div>

                            {/* Tier Selector */}
                            <div className="space-y-2">
                                <Label>Agent Tier</Label>
                                <Select value={tier.toString()} onValueChange={(v) => setTier(Number(v) as 1 | 2)}>
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="1">
                                            <div className="flex items-center gap-2">
                                                <Zap className="h-4 w-4 text-yellow-500" />
                                                Tier 1 - Simple NPC
                                            </div>
                                        </SelectItem>
                                        <SelectItem value="2">
                                            <div className="flex items-center gap-2">
                                                <Database className="h-4 w-4 text-blue-500" />
                                                Tier 2 - RAG Agent
                                            </div>
                                        </SelectItem>
                                    </SelectContent>
                                </Select>
                                <p className="text-xs text-muted-foreground">
                                    {tier === 1
                                        ? "Fast responses without RAG"
                                        : "Knowledge-enhanced with citations"}
                                </p>
                            </div>

                            {/* Persona / Agent Selector */}
                            <div className="space-y-2">
                                <Label>{agentConfigs.length > 0 ? "Agent (from DB)" : "Persona"}</Label>
                                <Select value={persona} onValueChange={setPersona} disabled={agentsLoading}>
                                    <SelectTrigger>
                                        <SelectValue placeholder={agentsLoading ? "Loading agents..." : "Select agent"} />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {personas.map((p) => (
                                            <SelectItem key={p.name} value={p.name}>
                                                {p.display_name}
                                            </SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                                <p className="text-xs text-muted-foreground">
                                    {selectedPersona.description}
                                </p>
                            </div>

                            {/* Streaming Toggle */}
                            <div className="space-y-2">
                                <Label>Response Mode</Label>
                                <Select
                                    value={useStreaming ? "streaming" : "complete"}
                                    onValueChange={(v) => setUseStreaming(v === "streaming")}
                                >
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="streaming">Streaming (SSE)</SelectItem>
                                        <SelectItem value="complete">Complete Response</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>

                            {/* Persona Info */}
                            <div className="pt-4 border-t flex flex-col gap-3">
                                <div className="flex items-center gap-3">
                                    {selectedPersona.avatar_url && (
                                        <div className="w-16 h-16 rounded-full overflow-hidden border-2 border-primary/20 shrink-0">
                                            <img
                                                src={selectedPersona.avatar_url}
                                                alt={selectedPersona.display_name}
                                                className="w-full h-full object-cover"
                                                onError={(e) => {
                                                    // Fallback if image doesn't exist yet
                                                    (e.target as HTMLImageElement).src = 'data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-bot"><path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/><path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/></svg>';
                                                }}
                                            />
                                        </div>
                                    )}
                                    <div>
                                        <h4 className="font-medium mb-1">{selectedPersona.display_name}</h4>
                                        <div className="flex flex-col gap-2">
                                            <div className="flex flex-wrap gap-1">
                                                {selectedPersona.traits.map((trait) => (
                                                    <Badge key={trait} variant="secondary" className="text-xs">
                                                        {trait}
                                                    </Badge>
                                                ))}
                                            </div>
                                            {/* Capability Badges */}
                                            <div className="flex flex-wrap gap-1 mt-1">
                                                {selectedPersona.name === "Mimir" && (
                                                    <Badge variant="default" className="text-[10px] bg-indigo-500 hover:bg-indigo-600">
                                                        ⚔️ Actions: heal, buff
                                                    </Badge>
                                                )}
                                                {selectedPersona.name === "sage_ariel" && (
                                                    <Badge variant="default" className="text-[10px] bg-emerald-500 hover:bg-emerald-600">
                                                        📚 RAG: item_db, mob_db
                                                    </Badge>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            {/* Vector DB Status */}
                            <div className="pt-4 border-t space-y-3">
                                <div className="flex items-center justify-between">
                                    <Label className="flex items-center gap-1 text-muted-foreground"><Database className="h-4 w-4" /> Knowledge Base (Qdrant)</Label>
                                    {vectorStats ? (
                                        <Badge variant="outline" className="text-[10px] px-1 h-5 text-green-500 bg-green-500/10 border-green-500/20">Online</Badge>
                                    ) : (
                                        <Badge variant="outline" className="text-[10px] px-1 h-5 text-yellow-500 bg-yellow-500/10 border-yellow-500/20">Checking...</Badge>
                                    )}
                                </div>
                                {vectorStats && (
                                    <div className="grid grid-cols-2 gap-2 text-xs">
                                        <div className="p-2 bg-muted rounded-md text-center">
                                            <div className="font-medium text-foreground text-sm">{vectorStats.database?.total_qa || 0}</div>
                                            <div className="text-muted-foreground uppercase tracking-wider text-[9px]">Text DB</div>
                                        </div>
                                        <div className="p-2 bg-muted rounded-md text-center">
                                            <div className="font-medium text-foreground text-sm">{vectorStats.qdrant?.result?.points_count || 0}</div>
                                            <div className="text-muted-foreground uppercase tracking-wider text-[9px]">Vector DB</div>
                                        </div>
                                    </div>
                                )}
                            </div>
                        </CardContent>
                    </Card>

                    {/* Chat Panel */}
                    <div className="lg:col-span-3 flex flex-col gap-4">
                        {/* Messages */}
                        <Card className="flex-1 min-h-[500px]">
                            <CardContent className="p-4 h-full overflow-y-auto">
                                <div className="space-y-4">
                                    {messages.map((msg, idx) => (
                                        <div
                                            key={idx}
                                            className={`flex gap-3 ${msg.role === "user" ? "justify-end" : "justify-start"
                                                }`}
                                        >
                                            {msg.role === "assistant" && (
                                                <div className="w-8 h-8 rounded-full bg-primary flex items-center justify-center flex-shrink-0 overflow-hidden">
                                                    {selectedPersona.avatar_url ? (
                                                        <img
                                                            src={selectedPersona.avatar_url}
                                                            alt="Assistant"
                                                            className="w-full h-full object-cover"
                                                            onError={(e) => {
                                                                (e.target as HTMLImageElement).src = 'data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-bot"><path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/><path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/></svg>';
                                                            }}
                                                        />
                                                    ) : (
                                                        <Bot className="h-4 w-4 text-primary-foreground" />
                                                    )}
                                                </div>
                                            )}
                                            <div className={`max-w-[80%] space-y-2 ${msg.role === "user"
                                                ? "bg-primary text-primary-foreground"
                                                : "bg-muted"
                                                } rounded-lg p-3`}>
                                                {msg.role === "assistant" ? (
                                                    msg.content ? (
                                                        <MarkdownMessage content={msg.content} />
                                                    ) : (
                                                        <TypingIndicator />
                                                    )
                                                ) : (
                                                    <p className="whitespace-pre-wrap">{msg.content}</p>
                                                )}

                                                {/* Metadata for assistant messages */}
                                                {msg.role === "assistant" && !msg.streaming && (
                                                    <div className="flex flex-wrap gap-2 pt-2 border-t border-border">
                                                        {msg.provider && msg.model && (
                                                            <Badge variant="outline" className="text-xs flex items-center gap-1">
                                                                {getProviderIcon(msg.provider)}
                                                                {msg.model}
                                                            </Badge>
                                                        )}
                                                        {msg.latency_ms && (
                                                            <Badge variant="outline" className="text-xs">
                                                                {msg.latency_ms}ms
                                                            </Badge>
                                                        )}
                                                        {msg.confidence_level && (
                                                            <Badge
                                                                variant="outline"
                                                                className={`text-xs text-white ${getConfidenceColor(msg.confidence_level)}`}
                                                            >
                                                                {msg.confidence_level} ({((msg.confidence_score || 0) * 100).toFixed(0)}%)
                                                            </Badge>
                                                        )}
                                                        {msg.streaming && (
                                                            <Loader2 className="h-3 w-3 animate-spin" />
                                                        )}

                                                        <div className="flex-1" />
                                                        <Button
                                                            variant="ghost"
                                                            size="sm"
                                                            className="h-6 w-6 p-0 hover:bg-background/50 text-muted-foreground"
                                                            onClick={() => handleCopyLog(idx)}
                                                            title="Copy Debug Log"
                                                        >
                                                            {copiedIndex === idx ? <Check className="h-3 w-3 text-green-500" /> : <Copy className="h-3 w-3" />}
                                                        </Button>
                                                    </div>
                                                )}

                                                {/* Action Display for Tier 1 NPC Actions */}
                                                {msg.role === "assistant" && msg.action && msg.action.command && (
                                                    <div className="mt-2 bg-green-500/10 border border-green-500/20 rounded-md p-3 text-sm">
                                                        <div className="flex items-center gap-2 text-green-600 dark:text-green-400 font-medium mb-1">
                                                            <CheckCircle2 className="h-4 w-4" />
                                                            Action Invoked: {msg.action.command}
                                                        </div>
                                                        <pre className="text-xs overflow-x-auto text-muted-foreground">
                                                            {JSON.stringify(msg.action.params, null, 2)}
                                                        </pre>
                                                    </div>
                                                )}
                                            </div>
                                            {msg.role === "user" && (
                                                <div className="w-8 h-8 rounded-full bg-secondary flex items-center justify-center flex-shrink-0">
                                                    <User className="h-4 w-4" />
                                                </div>
                                            )}
                                        </div>
                                    ))}
                                    <div ref={messagesEndRef} />
                                </div>
                            </CardContent>
                        </Card>

                        {/* Source Citations (Tier 2 only) */}
                        {tier === 2 && messages.length > 0 && messages[messages.length - 1].sources && (
                            <Card>
                                <CardHeader className="py-3">
                                    <CardTitle className="text-sm flex items-center gap-2">
                                        <BookOpen className="h-4 w-4" />
                                        Sources
                                    </CardTitle>
                                </CardHeader>
                                <CardContent className="py-2">
                                    <div className="space-y-2">
                                        {messages[messages.length - 1].sources?.map((source, idx) => (
                                            <div
                                                key={idx}
                                                className="flex items-start gap-2 text-sm p-2 rounded-md hover:bg-accent cursor-pointer transition-colors group"
                                                onClick={() => handleOpenWiki(source.source_id)}
                                            >
                                                <Badge variant="outline" className="text-xs flex-shrink-0">
                                                    {source.source_type}
                                                </Badge>
                                                <div className="flex-1 min-w-0">
                                                    <p className="font-medium truncate flex items-center gap-1">
                                                        {source.source_id}
                                                        <Zap className="h-3 w-3 opacity-0 group-hover:opacity-100 transition-opacity text-primary" />
                                                    </p>
                                                    <p className="text-xs text-muted-foreground line-clamp-2">
                                                        {source.snippet}
                                                    </p>
                                                </div>
                                                <Badge variant="secondary" className="text-xs">
                                                    {(source.relevance * 100).toFixed(0)}%
                                                </Badge>
                                            </div>
                                        ))}
                                    </div>
                                </CardContent>
                            </Card>
                        )}

                        {/* Wiki View Modal */}
                        {viewingWiki && (
                            <WikiModal
                                filename={viewingWiki}
                                content={wikiContent}
                                loading={wikiLoading}
                                onClose={() => setViewingWiki(null)}
                            />
                        )}

                        {/* Error Display */}
                        {error && (
                            <div className="flex items-center gap-2 text-destructive bg-destructive/10 p-3 rounded-lg">
                                <AlertCircle className="h-4 w-4" />
                                <p className="text-sm">{error}</p>
                            </div>
                        )}

                        {/* Input */}
                        <div className="flex gap-2">
                            <Input
                                placeholder="Type your message..."
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={handleKeyDown}
                                disabled={loading}
                                className="flex-1"
                            />
                            <Button onClick={handleSend} disabled={loading || !input.trim()}>
                                {loading ? (
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                ) : (
                                    <Send className="h-4 w-4" />
                                )}
                            </Button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
