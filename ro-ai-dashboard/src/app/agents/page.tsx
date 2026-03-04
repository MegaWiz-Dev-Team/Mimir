"use client";

import { useState, useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
    AgentConfig,
    AgentTemplate,
    AgentChatResponse,
    CreateAgentRequest,
    PROVIDERS,
    fetchAgents,
    createAgent,
    getAgent,
    updateAgent,
    deleteAgent,
    publishAgent,
    agentChat,
    fetchTemplates,
    fetchModels,
    modelsToProviders,
    LlmProvider,
} from "@/lib/api";
import {
    Plus, Brain, Bot, Send, Trash2, Edit, Rocket, Copy, Check,
    ChevronLeft, Loader2, Globe, Zap, Database, Wrench, Sparkles,
    ThumbsUp, ThumbsDown, Clock, Hash, X, LayoutGrid, MessageSquare,
} from "lucide-react";

// ─── Types ──────────────────────────────────────────────────────────────────────

interface ChatMessage {
    role: "user" | "assistant";
    content: string;
    latency_ms?: number;
    input_tokens?: number;
    output_tokens?: number;
}

type View = "list" | "builder" | "chat";

// ─── Component ──────────────────────────────────────────────────────────────────

export default function AgentStudioPage() {
    // Data state
    const [agents, setAgents] = useState<AgentConfig[]>([]);
    const [templates, setTemplates] = useState<AgentTemplate[]>([]);
    const [providers, setProviders] = useState<LlmProvider[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    // View state
    const [view, setView] = useState<View>("list");
    const [editingAgent, setEditingAgent] = useState<AgentConfig | null>(null);
    const [selectedAgent, setSelectedAgent] = useState<AgentConfig | null>(null);

    // Builder form state
    const [formName, setFormName] = useState("");
    const [formDisplayName, setFormDisplayName] = useState("");
    const [formDescription, setFormDescription] = useState("");
    const [formSystemPrompt, setFormSystemPrompt] = useState("");
    const [formModelId, setFormModelId] = useState("llama3.2");
    const [formProvider, setFormProvider] = useState("ollama");
    const [formTemperature, setFormTemperature] = useState(0.7);
    const [formMaxTokens, setFormMaxTokens] = useState(2048);
    const [formTopK, setFormTopK] = useState(5);
    const [formUseRag, setFormUseRag] = useState(true);
    const [formUseKG, setFormUseKG] = useState(false);
    const [formTools, setFormTools] = useState<string[]>([]);
    const [formTraits, setFormTraits] = useState<string[]>([]);
    const [formGreeting, setFormGreeting] = useState("");
    const [formTemplateId, setFormTemplateId] = useState<string | null>(null);

    // Chat state
    const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
    const [chatInput, setChatInput] = useState("");
    const [chatSessionId, setChatSessionId] = useState<string | null>(null);
    const [chatSending, setChatSending] = useState(false);
    const chatEndRef = useRef<HTMLDivElement>(null);

    // Misc
    const [saving, setSaving] = useState(false);
    const [copiedKey, setCopiedKey] = useState(false);
    const [showTemplates, setShowTemplates] = useState(false);
    const [activeTab, setActiveTab] = useState<"basic" | "model" | "behavior" | "rag" | "tools">("basic");

    // ─── Load data ──────────────────────────────────────────────────────────────

    const loadAgents = async () => {
        try {
            setLoading(true);
            const data = await fetchAgents();
            setAgents(data.agents || []);
        } catch (err: any) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadAgents();
        fetchTemplates().then(setTemplates).catch(() => { });
        // Merge DB models with static PROVIDERS (ensures Heimdall always appears)
        fetchModels().then(m => {
            const dbProviders = modelsToProviders(m);
            // Add any PROVIDERS not already in dbProviders
            const mergedMap = new Map<string, LlmProvider>();
            for (const p of PROVIDERS) mergedMap.set(p.id, p);
            for (const p of dbProviders) mergedMap.set(p.id, p); // DB overrides if exists
            setProviders(Array.from(mergedMap.values()));
        }).catch(() => setProviders(PROVIDERS)); // fallback to static list
    }, []);

    useEffect(() => {
        chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [chatMessages]);

    // ─── Builder helpers ────────────────────────────────────────────────────────

    const resetForm = () => {
        setFormName(""); setFormDisplayName(""); setFormDescription("");
        setFormSystemPrompt(""); setFormModelId("llama3.2"); setFormProvider("ollama");
        setFormTemperature(0.7); setFormMaxTokens(2048); setFormTopK(5);
        setFormUseRag(true); setFormUseKG(false); setFormTools([]);
        setFormTraits([]); setFormGreeting(""); setFormTemplateId(null);
        setEditingAgent(null); setActiveTab("basic");
    };

    const loadTemplate = (t: AgentTemplate) => {
        setFormName(t.name); setFormDisplayName(t.display_name);
        setFormDescription(t.description); setFormSystemPrompt(t.system_prompt);
        setFormModelId(t.model_id); setFormProvider(t.provider);
        setFormTemperature(t.temperature); setFormMaxTokens(t.max_tokens);
        setFormUseRag(t.use_rag); setFormUseKG(t.use_knowledge_graph);
        setFormTools(t.tools); setFormTraits(t.personality_traits);
        setFormGreeting(t.greeting); setFormTemplateId(t.id);
        setShowTemplates(false);
    };

    const loadAgentToForm = (a: AgentConfig) => {
        setEditingAgent(a);
        setFormName(a.name); setFormDisplayName(a.display_name || "");
        setFormDescription(a.description || ""); setFormSystemPrompt(a.system_prompt);
        setFormModelId(a.model_id); setFormProvider(a.provider);
        setFormTemperature(a.temperature ?? 0.7); setFormMaxTokens(a.max_tokens ?? 2048);
        setFormTopK(a.top_k ?? 5); setFormUseRag(a.use_rag ?? true);
        setFormUseKG(a.use_knowledge_graph ?? false);
        setFormTools(a.tools || []); setFormTraits(a.personality_traits || []);
        setFormGreeting(a.greeting || ""); setFormTemplateId(a.template_id || null);
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            const data: CreateAgentRequest = {
                name: formName, display_name: formDisplayName || undefined,
                description: formDescription || undefined, system_prompt: formSystemPrompt,
                model_id: formModelId, provider: formProvider,
                temperature: formTemperature, max_tokens: formMaxTokens,
                top_k: formTopK, use_rag: formUseRag, use_knowledge_graph: formUseKG,
                tools: formTools.length > 0 ? formTools : undefined,
                personality_traits: formTraits.length > 0 ? formTraits : undefined,
                greeting: formGreeting || undefined, template_id: formTemplateId || undefined,
            };

            if (editingAgent) {
                await updateAgent(editingAgent.id, data);
            } else {
                await createAgent(data);
            }
            await loadAgents();
            setView("list");
            resetForm();
        } catch (err: any) {
            setError(err.message);
        } finally {
            setSaving(false);
        }
    };

    const handleDelete = async (id: number) => {
        if (!confirm("Delete this agent?")) return;
        try {
            await deleteAgent(id);
            await loadAgents();
        } catch (err: any) {
            setError(err.message);
        }
    };

    const handlePublish = async (id: number) => {
        try {
            const result = await publishAgent(id);
            await loadAgents();
            alert(`Agent published!\nAPI Key: ${result.api_key}`);
        } catch (err: any) {
            setError(err.message);
        }
    };

    // ─── Chat ───────────────────────────────────────────────────────────────────

    const openChat = (agent: AgentConfig) => {
        setSelectedAgent(agent);
        setChatMessages([]);
        setChatSessionId(null);
        setChatInput("");
        setView("chat");
        if (agent.greeting) {
            setChatMessages([{ role: "assistant", content: agent.greeting }]);
        }
    };

    const handleSendMessage = async () => {
        if (!chatInput.trim() || !selectedAgent || chatSending) return;
        const msg = chatInput.trim();
        setChatInput("");
        setChatMessages(prev => [...prev, { role: "user", content: msg }]);
        setChatSending(true);

        try {
            const resp = await agentChat(selectedAgent.id, msg, chatSessionId || undefined);
            setChatSessionId(resp.session_id);
            setChatMessages(prev => [...prev, {
                role: "assistant",
                content: resp.content,
                latency_ms: resp.latency_ms,
                input_tokens: resp.input_tokens,
                output_tokens: resp.output_tokens,
            }]);
        } catch (err: any) {
            setChatMessages(prev => [...prev, {
                role: "assistant",
                content: `Error: ${err.message}`,
            }]);
        } finally {
            setChatSending(false);
        }
    };

    const copyApiKey = (key: string) => {
        navigator.clipboard.writeText(key);
        setCopiedKey(true);
        setTimeout(() => setCopiedKey(false), 2000);
    };

    // ─── Tool options ───────────────────────────────────────────────────────────

    const availableTools = ["QueryMobDb", "QueryItemDb", "WebSearch", "Calculator"];

    const toggleTool = (tool: string) => {
        setFormTools(prev =>
            prev.includes(tool) ? prev.filter(t => t !== tool) : [...prev, tool]
        );
    };

    // ─── Render ─────────────────────────────────────────────────────────────────

    // --- LIST VIEW ---
    if (view === "list") {
        return (
            <div className="container mx-auto p-6 space-y-6">
                {/* Header */}
                <div className="flex items-center justify-between">
                    <div>
                        <h1 className="text-3xl font-bold bg-gradient-to-r from-purple-600 to-pink-600 bg-clip-text text-transparent">
                            Agent Studio
                        </h1>
                        <p className="text-gray-500 mt-1">Build, test, and deploy AI agents — no code required</p>
                    </div>
                    <Button
                        onClick={() => { resetForm(); setView("builder"); }}
                        className="bg-gradient-to-r from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white"
                    >
                        <Plus className="w-4 h-4 mr-2" /> New Agent
                    </Button>
                </div>

                {/* Error */}
                {error && (
                    <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg flex justify-between">
                        <span>{error}</span>
                        <button onClick={() => setError(null)}><X className="w-4 h-4" /></button>
                    </div>
                )}

                {/* Loading */}
                {loading ? (
                    <div className="flex justify-center py-20">
                        <Loader2 className="w-8 h-8 animate-spin text-purple-500" />
                    </div>
                ) : agents.length === 0 ? (
                    /* Empty state */
                    <Card className="border-dashed border-2 border-purple-200 dark:border-purple-800">
                        <CardContent className="flex flex-col items-center justify-center py-16">
                            <Brain className="w-16 h-16 text-purple-300 mb-4" />
                            <h3 className="text-xl font-semibold text-gray-700 dark:text-zinc-300">No agents yet</h3>
                            <p className="text-gray-500 mt-2 mb-6">Create your first AI agent from scratch or use a template</p>
                            <div className="flex gap-3">
                                <Button onClick={() => { resetForm(); setView("builder"); }}
                                    className="bg-gradient-to-r from-purple-600 to-pink-600 text-white">
                                    <Plus className="w-4 h-4 mr-2" /> Create Agent
                                </Button>
                                <Button variant="outline" onClick={() => { resetForm(); setShowTemplates(true); setView("builder"); }}>
                                    <Sparkles className="w-4 h-4 mr-2" /> Use Template
                                </Button>
                            </div>
                        </CardContent>
                    </Card>
                ) : (
                    /* Agent cards grid */
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {agents.map(agent => (
                            <Card key={agent.id}
                                className="hover:shadow-lg transition-all duration-200 hover:border-purple-300 dark:hover:border-purple-700 group cursor-pointer"
                                onClick={() => openChat(agent)}>
                                <CardHeader className="pb-3">
                                    <div className="flex items-start justify-between">
                                        <div className="flex items-center gap-3">
                                            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-purple-500 to-pink-500 flex items-center justify-center text-white font-bold text-lg shadow-md">
                                                {(agent.display_name || agent.name).charAt(0).toUpperCase()}
                                            </div>
                                            <div>
                                                <CardTitle className="text-base">{agent.display_name || agent.name}</CardTitle>
                                                <p className="text-xs text-gray-500">{agent.provider}/{agent.model_id}</p>
                                            </div>
                                        </div>
                                        <Badge variant={agent.is_published ? "default" : "secondary"}
                                            className={agent.is_published ? "bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-400" : ""}>
                                            {agent.is_published ? "Published" : "Draft"}
                                        </Badge>
                                    </div>
                                </CardHeader>
                                <CardContent>
                                    <p className="text-sm text-gray-600 dark:text-zinc-400 line-clamp-2 mb-4">
                                        {agent.description || "No description"}
                                    </p>
                                    <div className="flex flex-wrap gap-1.5 mb-4">
                                        {agent.use_rag && <Badge variant="outline" className="text-xs"><Database className="w-3 h-3 mr-1" />RAG</Badge>}
                                        {agent.use_knowledge_graph && <Badge variant="outline" className="text-xs"><Globe className="w-3 h-3 mr-1" />KG</Badge>}
                                        {agent.tools && (agent.tools as string[]).length > 0 && (
                                            <Badge variant="outline" className="text-xs"><Wrench className="w-3 h-3 mr-1" />{(agent.tools as string[]).length} tools</Badge>
                                        )}
                                    </div>
                                    <div className="flex gap-2 pt-2 border-t opacity-0 group-hover:opacity-100 transition-opacity" onClick={e => e.stopPropagation()}>
                                        <Button size="sm" variant="ghost" onClick={() => { loadAgentToForm(agent); setView("builder"); }}>
                                            <Edit className="w-3 h-3 mr-1" /> Edit
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => openChat(agent)}>
                                            <MessageSquare className="w-3 h-3 mr-1" /> Chat
                                        </Button>
                                        {!agent.is_published && (
                                            <Button size="sm" variant="ghost" className="text-green-600" onClick={() => handlePublish(agent.id)}>
                                                <Rocket className="w-3 h-3 mr-1" /> Publish
                                            </Button>
                                        )}
                                        <Button size="sm" variant="ghost" className="text-red-600 ml-auto" onClick={() => handleDelete(agent.id)}>
                                            <Trash2 className="w-3 h-3" />
                                        </Button>
                                    </div>
                                    {agent.is_published && agent.api_key && (
                                        <div className="mt-2 flex items-center gap-2 text-xs bg-gray-50 dark:bg-zinc-900 rounded px-2 py-1" onClick={e => e.stopPropagation()}>
                                            <code className="truncate flex-1 font-mono">{agent.api_key}</code>
                                            <button onClick={() => copyApiKey(agent.api_key!)}>
                                                {copiedKey ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
                                            </button>
                                        </div>
                                    )}
                                </CardContent>
                            </Card>
                        ))}
                    </div>
                )}
            </div>
        );
    }

    // --- BUILDER VIEW ---
    if (view === "builder") {
        const tabs = [
            { id: "basic" as const, label: "Basic Info", icon: Brain },
            { id: "model" as const, label: "Model", icon: Zap },
            { id: "behavior" as const, label: "Behavior", icon: Sparkles },
            { id: "rag" as const, label: "RAG & KG", icon: Database },
            { id: "tools" as const, label: "Tools", icon: Wrench },
        ];

        return (
            <div className="container mx-auto p-6 space-y-6">
                {/* Header */}
                <div className="flex items-center gap-4">
                    <Button variant="ghost" onClick={() => { setView("list"); resetForm(); }}>
                        <ChevronLeft className="w-4 h-4 mr-1" /> Back
                    </Button>
                    <h1 className="text-2xl font-bold">{editingAgent ? "Edit Agent" : "New Agent"}</h1>
                    {!editingAgent && (
                        <Button variant="outline" size="sm" onClick={() => setShowTemplates(!showTemplates)}>
                            <Sparkles className="w-4 h-4 mr-1" /> Templates
                        </Button>
                    )}
                </div>

                {/* Template Gallery */}
                {showTemplates && (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                        {templates.map(t => (
                            <Card key={t.id}
                                className="cursor-pointer hover:border-purple-400 hover:shadow-md transition-all"
                                onClick={() => loadTemplate(t)}>
                                <CardHeader className="pb-2">
                                    <CardTitle className="text-sm">{t.display_name}</CardTitle>
                                </CardHeader>
                                <CardContent>
                                    <p className="text-xs text-gray-500 line-clamp-2">{t.description}</p>
                                    <div className="flex gap-1 mt-2">
                                        {t.use_rag && <Badge variant="outline" className="text-[10px]">RAG</Badge>}
                                        {t.use_knowledge_graph && <Badge variant="outline" className="text-[10px]">KG</Badge>}
                                        {t.tools.length > 0 && <Badge variant="outline" className="text-[10px]">{t.tools.length} tools</Badge>}
                                    </div>
                                </CardContent>
                            </Card>
                        ))}
                    </div>
                )}

                {error && (
                    <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg flex justify-between">
                        <span>{error}</span>
                        <button onClick={() => setError(null)}><X className="w-4 h-4" /></button>
                    </div>
                )}

                {/* Tab Navigation */}
                <div className="flex gap-1 bg-gray-100 dark:bg-zinc-900 rounded-lg p-1">
                    {tabs.map(tab => {
                        const Icon = tab.icon;
                        return (
                            <button key={tab.id}
                                onClick={() => setActiveTab(tab.id)}
                                className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === tab.id
                                    ? "bg-white dark:bg-zinc-800 shadow-sm text-purple-700 dark:text-purple-400"
                                    : "text-gray-500 hover:text-gray-700 dark:hover:text-zinc-300"
                                    }`}>
                                <Icon className="w-4 h-4" />
                                {tab.label}
                            </button>
                        );
                    })}
                </div>

                {/* Tab Content */}
                <Card>
                    <CardContent className="pt-6 space-y-4">
                        {activeTab === "basic" && (
                            <>
                                <div className="grid grid-cols-2 gap-4">
                                    <div>
                                        <Label htmlFor="agent-name">Agent Name *</Label>
                                        <Input id="agent-name" value={formName} onChange={e => setFormName(e.target.value)}
                                            placeholder="my_agent" className="mt-1 font-mono" />
                                    </div>
                                    <div>
                                        <Label htmlFor="agent-display">Display Name</Label>
                                        <Input id="agent-display" value={formDisplayName} onChange={e => setFormDisplayName(e.target.value)}
                                            placeholder="My Agent" className="mt-1" />
                                    </div>
                                </div>
                                <div>
                                    <Label htmlFor="agent-desc">Description</Label>
                                    <textarea id="agent-desc" value={formDescription} onChange={e => setFormDescription(e.target.value)}
                                        placeholder="What does this agent do?"
                                        className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm min-h-[60px] resize-y" />
                                </div>
                                <div>
                                    <Label htmlFor="agent-greeting">Greeting Message</Label>
                                    <Input id="agent-greeting" value={formGreeting} onChange={e => setFormGreeting(e.target.value)}
                                        placeholder="Hello! How can I help you?" className="mt-1" />
                                </div>
                                <div>
                                    <Label>Personality Traits</Label>
                                    <div className="flex flex-wrap gap-2 mt-1">
                                        {["helpful", "concise", "friendly", "scholarly", "analytical", "creative", "empathetic", "precise"].map(trait => (
                                            <button key={trait}
                                                onClick={() => setFormTraits(prev =>
                                                    prev.includes(trait) ? prev.filter(t => t !== trait) : [...prev, trait]
                                                )}
                                                className={`px-3 py-1 rounded-full text-xs font-medium transition-all ${formTraits.includes(trait)
                                                    ? "bg-purple-100 text-purple-700 ring-1 ring-purple-300 dark:bg-purple-900/40 dark:text-purple-400"
                                                    : "bg-gray-100 text-gray-600 hover:bg-gray-200 dark:bg-zinc-800 dark:text-zinc-400"
                                                    }`}>
                                                {trait}
                                            </button>
                                        ))}
                                    </div>
                                </div>
                            </>
                        )}

                        {activeTab === "model" && (
                            <>
                                <div className="grid grid-cols-2 gap-4">
                                    <div>
                                        <Label>Provider</Label>
                                        <select value={formProvider}
                                            onChange={e => setFormProvider(e.target.value)}
                                            className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm">
                                            {providers.length > 0 ? providers.map(p => (
                                                <option key={p.id} value={p.id}>{p.display_name}</option>
                                            )) : (
                                                <>
                                                    <option value="heimdall">Heimdall (Self-Hosted)</option>
                                                    <option value="ollama">Ollama (Local)</option>
                                                    <option value="gemini">Google Gemini</option>
                                                    <option value="openai">OpenAI</option>
                                                </>
                                            )}
                                        </select>
                                    </div>
                                    <div>
                                        <Label>Model</Label>
                                        <select value={formModelId}
                                            onChange={e => setFormModelId(e.target.value)}
                                            className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm">
                                            {providers.find(p => p.id === formProvider)?.models.map(m => (
                                                <option key={m.id} value={m.id}>{m.display_name || m.id}</option>
                                            )) || <option value={formModelId}>{formModelId}</option>}
                                        </select>
                                    </div>
                                </div>
                                <div className="grid grid-cols-3 gap-4">
                                    <div>
                                        <Label>Temperature ({formTemperature.toFixed(2)})</Label>
                                        <input type="range" min="0" max="2" step="0.05" value={formTemperature}
                                            onChange={e => setFormTemperature(parseFloat(e.target.value))}
                                            className="mt-2 w-full accent-purple-600" />
                                        <div className="flex justify-between text-xs text-gray-400"><span>Precise</span><span>Creative</span></div>
                                    </div>
                                    <div>
                                        <Label htmlFor="max-tokens">Max Tokens</Label>
                                        <Input id="max-tokens" type="number" value={formMaxTokens}
                                            onChange={e => setFormMaxTokens(parseInt(e.target.value) || 2048)} className="mt-1" />
                                    </div>
                                    <div>
                                        <Label htmlFor="top-k">Top K (RAG chunks)</Label>
                                        <Input id="top-k" type="number" value={formTopK}
                                            onChange={e => setFormTopK(parseInt(e.target.value) || 5)} className="mt-1" />
                                    </div>
                                </div>
                            </>
                        )}

                        {activeTab === "behavior" && (
                            <div>
                                <Label htmlFor="system-prompt">System Prompt *</Label>
                                <textarea id="system-prompt" value={formSystemPrompt}
                                    onChange={e => setFormSystemPrompt(e.target.value)}
                                    placeholder="You are a helpful assistant..."
                                    className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm min-h-[200px] resize-y font-mono"
                                />
                                <p className="text-xs text-gray-400 mt-1">{formSystemPrompt.length} characters</p>
                            </div>
                        )}

                        {activeTab === "rag" && (
                            <div className="space-y-4">
                                <div className="flex items-center gap-6">
                                    <label className="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" checked={formUseRag}
                                            onChange={e => setFormUseRag(e.target.checked)}
                                            className="w-4 h-4 rounded accent-purple-600" />
                                        <span className="text-sm font-medium">Enable RAG (Vector Search)</span>
                                    </label>
                                    <label className="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" checked={formUseKG}
                                            onChange={e => setFormUseKG(e.target.checked)}
                                            className="w-4 h-4 rounded accent-purple-600" />
                                        <span className="text-sm font-medium">Enable Knowledge Graph</span>
                                    </label>
                                </div>
                                <p className="text-sm text-gray-500">
                                    RAG retrieves relevant context from your knowledge base before generating responses.
                                    Knowledge Graph enables structured relationship queries for deeper context.
                                </p>
                            </div>
                        )}

                        {activeTab === "tools" && (
                            <div className="space-y-4">
                                <Label>Available Tools</Label>
                                <div className="grid grid-cols-2 gap-3">
                                    {availableTools.map(tool => (
                                        <div key={tool}
                                            onClick={() => toggleTool(tool)}
                                            className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-all ${formTools.includes(tool)
                                                ? "border-purple-400 bg-purple-50 dark:bg-purple-900/20 dark:border-purple-700"
                                                : "border-gray-200 hover:border-gray-300 dark:border-zinc-700 dark:hover:border-zinc-600"
                                                }`}>
                                            <Wrench className={`w-5 h-5 ${formTools.includes(tool) ? "text-purple-600" : "text-gray-400"}`} />
                                            <div>
                                                <span className="text-sm font-medium">{tool}</span>
                                                <p className="text-xs text-gray-400">
                                                    {tool === "QueryMobDb" && "Query monster database"}
                                                    {tool === "QueryItemDb" && "Query item database"}
                                                    {tool === "WebSearch" && "Search the web"}
                                                    {tool === "Calculator" && "Mathematical calculations"}
                                                </p>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* Save button */}
                <div className="flex justify-end gap-3">
                    <Button variant="outline" onClick={() => { setView("list"); resetForm(); }}>Cancel</Button>
                    <Button onClick={handleSave} disabled={!formName || !formSystemPrompt || saving}
                        className="bg-gradient-to-r from-purple-600 to-pink-600 text-white min-w-[120px]">
                        {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : (editingAgent ? "Update Agent" : "Create Agent")}
                    </Button>
                </div>
            </div>
        );
    }

    // --- CHAT VIEW ---
    return (
        <div className="flex h-[calc(100vh-64px)]">
            {/* Chat area */}
            <div className="flex-1 flex flex-col">
                {/* Chat header */}
                <div className="border-b px-6 py-3 flex items-center justify-between bg-white dark:bg-zinc-950">
                    <div className="flex items-center gap-3">
                        <Button variant="ghost" size="sm" onClick={() => setView("list")}>
                            <ChevronLeft className="w-4 h-4" />
                        </Button>
                        <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-purple-500 to-pink-500 flex items-center justify-center text-white font-bold text-sm">
                            {(selectedAgent?.display_name || selectedAgent?.name || "A").charAt(0).toUpperCase()}
                        </div>
                        <div>
                            <h2 className="font-semibold text-sm">{selectedAgent?.display_name || selectedAgent?.name}</h2>
                            <p className="text-xs text-gray-500">{selectedAgent?.provider}/{selectedAgent?.model_id}</p>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <Badge variant="outline" className="text-xs">
                            {selectedAgent?.is_published ? "Published" : "Draft"}
                        </Badge>
                        {chatSessionId && (
                            <Badge variant="secondary" className="text-xs font-mono">
                                {chatSessionId.substring(0, 8)}...
                            </Badge>
                        )}
                    </div>
                </div>

                {/* Messages */}
                <div className="flex-1 overflow-y-auto p-6 space-y-4">
                    {chatMessages.map((msg, i) => (
                        <div key={i} className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
                            <div className={`max-w-[70%] rounded-2xl px-4 py-3 ${msg.role === "user"
                                ? "bg-gradient-to-r from-purple-600 to-pink-600 text-white"
                                : "bg-gray-100 dark:bg-zinc-800 text-gray-900 dark:text-zinc-100"
                                }`}>
                                <div className="text-sm leading-relaxed whitespace-pre-wrap">
                                    {msg.role === "assistant" ? (
                                        <ReactMarkdown>{msg.content}</ReactMarkdown>
                                    ) : msg.content}
                                </div>
                                {msg.role === "assistant" && msg.latency_ms !== undefined && (
                                    <div className="flex items-center gap-3 mt-2 pt-2 border-t border-gray-200 dark:border-zinc-700">
                                        <span className="flex items-center gap-1 text-[10px] text-gray-400">
                                            <Clock className="w-3 h-3" /> {msg.latency_ms}ms
                                        </span>
                                        <span className="flex items-center gap-1 text-[10px] text-gray-400">
                                            <Hash className="w-3 h-3" /> {(msg.input_tokens || 0) + (msg.output_tokens || 0)} tokens
                                        </span>
                                    </div>
                                )}
                            </div>
                        </div>
                    ))}
                    {chatSending && (
                        <div className="flex justify-start">
                            <div className="bg-gray-100 dark:bg-zinc-800 rounded-2xl px-4 py-3">
                                <Loader2 className="w-4 h-4 animate-spin text-purple-500" />
                            </div>
                        </div>
                    )}
                    <div ref={chatEndRef} />
                </div>

                {/* Input */}
                <div className="border-t px-6 py-4 bg-white dark:bg-zinc-950">
                    <div className="flex gap-3">
                        <Input
                            value={chatInput}
                            onChange={e => setChatInput(e.target.value)}
                            onKeyDown={e => e.key === "Enter" && !e.shiftKey && handleSendMessage()}
                            placeholder="Type a message..."
                            className="flex-1"
                            disabled={chatSending}
                        />
                        <Button onClick={handleSendMessage} disabled={!chatInput.trim() || chatSending}
                            className="bg-gradient-to-r from-purple-600 to-pink-600 text-white">
                            <Send className="w-4 h-4" />
                        </Button>
                    </div>
                </div>
            </div>

            {/* Right sidebar — Agent info */}
            <div className="w-80 border-l bg-gray-50 dark:bg-zinc-900 overflow-y-auto p-4 space-y-4 hidden lg:block">
                <h3 className="font-semibold text-sm text-gray-700 dark:text-zinc-300">Agent Config</h3>
                <div className="space-y-3 text-xs">
                    <div>
                        <span className="text-gray-400">Model</span>
                        <p className="font-mono">{selectedAgent?.model_id}</p>
                    </div>
                    <div>
                        <span className="text-gray-400">Provider</span>
                        <p>{selectedAgent?.provider}</p>
                    </div>
                    <div>
                        <span className="text-gray-400">Temperature</span>
                        <p>{selectedAgent?.temperature ?? 0.7}</p>
                    </div>
                    <div>
                        <span className="text-gray-400">Max Tokens</span>
                        <p>{selectedAgent?.max_tokens ?? 2048}</p>
                    </div>
                    <div>
                        <span className="text-gray-400">RAG</span>
                        <p>{selectedAgent?.use_rag ? "Enabled" : "Disabled"}</p>
                    </div>
                    <div>
                        <span className="text-gray-400">Knowledge Graph</span>
                        <p>{selectedAgent?.use_knowledge_graph ? "Enabled" : "Disabled"}</p>
                    </div>
                    {selectedAgent?.personality_traits && (selectedAgent.personality_traits as string[]).length > 0 && (
                        <div>
                            <span className="text-gray-400">Traits</span>
                            <div className="flex flex-wrap gap-1 mt-1">
                                {(selectedAgent.personality_traits as string[]).map(t => (
                                    <Badge key={t} variant="outline" className="text-[10px]">{t}</Badge>
                                ))}
                            </div>
                        </div>
                    )}
                    {selectedAgent?.tools && (selectedAgent.tools as string[]).length > 0 && (
                        <div>
                            <span className="text-gray-400">Tools</span>
                            <div className="flex flex-wrap gap-1 mt-1">
                                {(selectedAgent.tools as string[]).map(t => (
                                    <Badge key={t} variant="outline" className="text-[10px]">{t}</Badge>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
                <div className="pt-3 border-t space-y-2">
                    <Button size="sm" variant="outline" className="w-full" onClick={() => { if (selectedAgent) { loadAgentToForm(selectedAgent); setView("builder"); } }}>
                        <Edit className="w-3 h-3 mr-2" /> Edit Agent
                    </Button>
                    {selectedAgent && !selectedAgent.is_published && (
                        <Button size="sm" className="w-full bg-green-600 hover:bg-green-700 text-white" onClick={() => handlePublish(selectedAgent.id)}>
                            <Rocket className="w-3 h-3 mr-2" /> Publish
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
}
