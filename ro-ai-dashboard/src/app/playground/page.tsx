"use client";

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
    PERSONAS,
    PROVIDERS,
    Persona,
    ChatResponse,
    SourceCitation,
    StreamDone,
    LlmProvider,
    sendChat,
    streamChat,
    fetchModels,
    modelsToProviders,
} from "@/lib/api";
import { Send, Trash2, User, Bot, Loader2, AlertCircle, BookOpen, Database, Zap, Cloud, HardDrive, X } from "lucide-react";

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
    const [tier, setTier] = useState<1 | 2>(2);
    const [persona, setPersona] = useState<string>("sage_ariel");
    const [provider, setProvider] = useState<string>("ollama");
    const [model, setModel] = useState<string>("");
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState("");
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [useStreaming, setUseStreaming] = useState(true);
    const [providers, setProviders] = useState<LlmProvider[]>(PROVIDERS);
    const [modelsLoading, setModelsLoading] = useState(true);

    // Wiki Modal state
    const [viewingWiki, setViewingWiki] = useState<string | null>(null);
    const [wikiContent, setWikiContent] = useState<string>("");
    const [wikiLoading, setWikiLoading] = useState(false);

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const abortRef = useRef<(() => void) | null>(null);

    // Get selected persona and provider details
    const selectedPersona = PERSONAS.find(p => p.name === persona) || PERSONAS[0];
    const selectedProvider = providers.find(p => p.id === provider) || providers[0];
    const selectedModel = selectedProvider?.models.find(m => m.id === model) || selectedProvider?.models[0];

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

    // Set greeting when persona changes
    useEffect(() => {
        if (messages.length === 0 && selectedPersona.greeting) {
            setMessages([{
                role: "assistant",
                content: selectedPersona.greeting,
            }]);
        }
    }, [persona, selectedPersona.greeting]);

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
                            const lastMessage = newMessages[newMessages.length - 1];
                            if (lastMessage.role === "assistant") {
                                lastMessage.content += token;
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

    return (
        <div className="container mx-auto p-8 max-w-6xl">
            <div className="flex justify-between items-center mb-8">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Agent Playground</h1>
                    <p className="text-muted-foreground">Test NPC agents with different personas, providers, and tiers</p>
                </div>
                <Button variant="outline" onClick={handleClear}>
                    <Trash2 className="mr-2 h-4 w-4" />
                    Clear Chat
                </Button>
            </div>

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
                            <p className="text-xs text-muted-foreground">
                                {selectedModel?.description}
                            </p>
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

                        {/* Persona Selector */}
                        <div className="space-y-2">
                            <Label>Persona</Label>
                            <Select value={persona} onValueChange={setPersona}>
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    {PERSONAS.map((p) => (
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
                        <div className="pt-4 border-t">
                            <h4 className="font-medium mb-2">{selectedPersona.display_name}</h4>
                            <div className="flex flex-wrap gap-1">
                                {selectedPersona.traits.map((trait) => (
                                    <Badge key={trait} variant="secondary" className="text-xs">
                                        {trait}
                                    </Badge>
                                ))}
                            </div>
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
                                            <div className="w-8 h-8 rounded-full bg-primary flex items-center justify-center flex-shrink-0">
                                                <Bot className="h-4 w-4 text-primary-foreground" />
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
        </div>
    );
}

