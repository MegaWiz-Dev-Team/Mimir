"use client";

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
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
    fetchAgents,
    createAgent,
    getAgent,
    updateAgent,
    deleteAgent,
    publishAgent,
    agentChat,
    agentBifrostChat,
    fetchTemplates,
    fetchModels,
    modelsToProviders,
    LlmProvider,
    generateAgent,
    GeneratedAgentDraft,
    fetchBenchmarkDatasets,
    fetchEvalRuns,
    fetchChampion,
    promoteRun,
    startEvalRun,
    autoTuneAgent,
    AutoTuneResponse,
    BenchmarkDataset,
    EvalRunSummary,
    ChampionRun,
} from "@/lib/api";
import {
    Plus, Brain, Bot, Send, Trash2, Edit, Rocket, Copy, Check,
    ChevronLeft, Loader2, Globe, Zap, Database, Wrench, Sparkles,
    ThumbsUp, ThumbsDown, Clock, Hash, X, LayoutGrid, MessageSquare,
    ExternalLink, Wand2, Save, Dna, Stethoscope, BarChart3, Wand, Crown,
} from "lucide-react";
import Link from "next/link";
import remarkGfm from "remark-gfm";
import {
    LineChart, Line, BarChart, Bar, 
    XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer
} from "recharts";
import { AgentStructurePanel } from "@/components/AgentStructurePanel";

// ─── Types ──────────────────────────────────────────────────────────────────────

interface ChatMessage {
    role: "user" | "assistant";
    content: string;
    reasoning?: string;
    trace_id?: string;
    steps?: Array<{
        step_type: string;
        content: string;
        tool_name?: string;
        duration_ms: number;
    }>;
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

    // Eval modal state
    const [evalAgent, setEvalAgent] = useState<AgentConfig | null>(null);
    const [benchmarks, setBenchmarks] = useState<BenchmarkDataset[]>([]);
    const [evalRuns, setEvalRuns] = useState<EvalRunSummary[]>([]);
    const [evalBenchmarkId, setEvalBenchmarkId] = useState<string>("");
    const [evalMaxItems, setEvalMaxItems] = useState<number>(10);
    const [evalRunName, setEvalRunName] = useState<string>("");
    const [evalNotes, setEvalNotes] = useState<string>("");
    const [evalStarting, setEvalStarting] = useState(false);
    const [evalStartedRunId, setEvalStartedRunId] = useState<string>("");
    const [evalError, setEvalError] = useState<string>("");

    // Group agents by agent_group
    interface AgentGroup {
        id: number;
        display_name: string;
        icon_emoji: string;
        color_hex: string;
        sort_order: number;
        agents: AgentConfig[];
    }

    const groupedAgents = useMemo(() => {
        const grouped = new Map<string, AgentGroup>();

        agents.forEach(agent => {
            if (agent.agent_group) {
                const groupKey = agent.agent_group.display_name;
                if (!grouped.has(groupKey)) {
                    grouped.set(groupKey, {
                        id: agent.agent_group.id,
                        display_name: agent.agent_group.display_name,
                        icon_emoji: agent.agent_group.icon_emoji,
                        color_hex: agent.agent_group.color_hex,
                        sort_order: agent.agent_group.sort_order,
                        agents: []
                    });
                }
                grouped.get(groupKey)!.agents.push(agent);
            }
        });

        return Array.from(grouped.values())
            .sort((a, b) => a.sort_order - b.sort_order)
            .map(group => ({
                ...group,
                agents: group.agents.sort((a, b) => (a.display_name || a.name).localeCompare(b.display_name || b.name))
            }));
    }, [agents]);

    const generateRunName = (a: AgentConfig, bench?: BenchmarkDataset) => {
        const modelTag = (a.model_id || "model").split("/").pop()?.split(":")[0] || "model";
        const benchTag = (bench?.source || "eval").replace(/[^a-z0-9_-]/gi, "");
        const ts = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "").slice(2);
        return `${a.name}__${modelTag}__${benchTag}__${ts}`;
    };

    const openEvalModal = async (a: AgentConfig) => {
        setEvalAgent(a);
        setEvalMaxItems(10);
        setEvalNotes("");
        setEvalStartedRunId("");
        setEvalError("");
        try {
            const [bms, runs] = await Promise.all([fetchBenchmarkDatasets(), fetchEvalRuns()]);
            setBenchmarks(bms);
            setEvalRuns(runs);
            const initialBench = bms.length > 0 ? bms[0] : undefined;
            if (initialBench) setEvalBenchmarkId(initialBench.id);
            setEvalRunName(generateRunName(a, initialBench));
        } catch (e) {
            console.error("Failed to load benchmarks", e);
        }
    };

    const handleStartEval = async () => {
        if (!evalAgent || !evalBenchmarkId) return;
        setEvalStarting(true);
        setEvalError("");
        try {
            const res = await startEvalRun({
                tenant_id: evalAgent.tenant_id,
                agent_names: [evalAgent.name],
                model_ids: [evalAgent.model_id],
                question_limit: evalMaxItems,
                benchmark_dataset_id: evalBenchmarkId,
                run_name: evalRunName || generateRunName(evalAgent, benchmarks.find(b => b.id === evalBenchmarkId)),
                notes: evalNotes || undefined,
            });
            setEvalStartedRunId(res.run_id);
            setTimeout(async () => setEvalRuns(await fetchEvalRuns()), 2000);
        } catch (e: any) {
            setEvalError(e.message || "Failed to start eval");
        } finally {
            setEvalStarting(false);
        }
    };

    const recentRunsForAgent = (agentName: string) =>
        evalRuns.filter(r => (r.name || "").toLowerCase().includes(agentName.toLowerCase())).slice(0, 5);

    // ── Champion tracking (Wave 1) ──────────────────────────────────────
    const [champions, setChampions] = useState<Record<string, ChampionRun>>({});

    const loadChampionsForAgents = useCallback(async (list: AgentConfig[]) => {
        const map: Record<string, ChampionRun> = {};
        await Promise.all(list.map(async a => {
            try {
                const c = await fetchChampion(a.name);
                if (c) map[a.name] = c;
            } catch {}
        }));
        setChampions(map);
    }, []);

    useEffect(() => {
        if (agents.length > 0) loadChampionsForAgents(agents);
    }, [agents, loadChampionsForAgents]);

    const handlePromoteRun = async (runId: string) => {
        try {
            await promoteRun(runId);
            const list = agents;
            await loadChampionsForAgents(list);
            const runs = await fetchEvalRuns();
            setEvalRuns(runs);
            alert("Run promoted to Champion ✓");
        } catch (e: any) {
            alert("Promote failed: " + (e.message || String(e)));
        }
    };

    // ── Auto-Tune state ──────────────────────────────────────────────────
    const [tuneAgent, setTuneAgent] = useState<AgentConfig | null>(null);
    const [tuneLoading, setTuneLoading] = useState(false);
    const [tuneResult, setTuneResult] = useState<AutoTuneResponse | null>(null);
    const [tuneError, setTuneError] = useState<string>("");

    const openTuneModal = async (a: AgentConfig) => {
        setTuneAgent(a);
        setTuneResult(null);
        setTuneError("");
        setTuneLoading(true);
        try {
            const result = await autoTuneAgent(a.id);
            if (result.error) setTuneError(result.error);
            else setTuneResult(result);
        } catch (e: any) {
            setTuneError(e.message || "Auto-tune failed");
        } finally {
            setTuneLoading(false);
        }
    };

    const applyTuneSuggestions = async () => {
        if (!tuneAgent || !tuneResult?.suggestions) return;
        const s = tuneResult.suggestions;
        const updates: any = {};
        if (s.system_prompt) updates.system_prompt = s.system_prompt;
        if (s.temperature != null) updates.temperature = s.temperature;
        if (s.max_tokens != null) updates.max_tokens = s.max_tokens;
        if (s.top_k != null) updates.top_k = s.top_k;
        if (s.use_rag != null) updates.use_rag = s.use_rag;
        if (s.use_knowledge_graph != null) updates.use_knowledge_graph = s.use_knowledge_graph;
        const currentTools: string[] = Array.isArray(tuneAgent.tools) ? (tuneAgent.tools as string[]) : [];
        const newTools = currentTools
            .filter(t => !(s.remove_tools || []).includes(t))
            .concat((s.add_tools || []).filter(t => !currentTools.includes(t)));
        if (newTools.join(",") !== currentTools.join(",")) updates.tools = newTools;

        try {
            await updateAgent(tuneAgent.id, updates);
            await loadAgents();
            setTuneAgent(null);
            alert(`Applied ${Object.keys(updates).length} change(s) to ${tuneAgent.name}.`);
        } catch (e: any) {
            setTuneError("Apply failed: " + (e.message || String(e)));
        }
    };

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
    const [formUsePageIndex, setFormUsePageIndex] = useState(false);
    const [formWeights, setFormWeights] = useState({ vector: 0.5, tree: 0.3, graph: 0.2 });
    const [formShowAdvanced, setFormShowAdvanced] = useState(false);
    const [formAdvanced, setFormAdvanced] = useState({
        top_k_per_source: 10, vector_alpha: 0.7, vector_threshold: 0.3, graph_hops: 2,
    });
    const [formRerank, setFormRerank] = useState({
        enabled: false, strategy: "rrf" as "rrf" | "cross_encoder" | "llm",
        model: "BAAI/bge-reranker-v2-m3", final_top_k: 5,
    });
    const [formTools, setFormTools] = useState<string[]>([]);
    const [formMcpServers, setFormMcpServers] = useState<string[]>([]);
    const [formTraits, setFormTraits] = useState<string[]>([]);
    const [formGreeting, setFormGreeting] = useState("");
    const [formTemplateId, setFormTemplateId] = useState<string | null>(null);
    const [formOutputFormat, setFormOutputFormat] = useState<"auto" | "json_chart" | "markdown_table">("auto");

    // Chat state
    const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
    const [chatInput, setChatInput] = useState("");
    const [chatSessionId, setChatSessionId] = useState<string | null>(null);
    const [chatSending, setChatSending] = useState(false);
    const [useBifrost, setUseBifrost] = useState(true);
    const chatEndRef = useRef<HTMLDivElement>(null);

    // Misc
    const [saving, setSaving] = useState(false);
    const [copiedKey, setCopiedKey] = useState(false);
    const [showTemplates, setShowTemplates] = useState(false);
    const [activeTab, setActiveTab] = useState<"basic" | "model" | "behavior" | "rag" | "tools">("basic");

    // AI Generator state
    const [showGenerator, setShowGenerator] = useState(false);
    const [genPrompt, setGenPrompt] = useState("");
    const [genProvider, setGenProvider] = useState("heimdall");
    const [genModelId, setGenModelId] = useState("");
    const [genLoading, setGenLoading] = useState(false);
    const [genDraft, setGenDraft] = useState<GeneratedAgentDraft | null>(null);
    const [genError, setGenError] = useState<string | null>(null);
    const [genLatency, setGenLatency] = useState<number | null>(null);
    const [genSaving, setGenSaving] = useState(false);

    // ─── Load data ──────────────────────────────────────────────────────────────

    const loadAgents = async () => {
        try {
            setLoading(true);
            const data = await fetchAgents();
            // Handle both array and {agents:[]} response shapes
            const list = Array.isArray(data) ? data : ((data as any).agents || []);
            setAgents(list);
        } catch (err: any) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadAgents();
        fetchTemplates().then(setTemplates).catch(() => { });
        fetchModels().then(m => {
            setProviders(modelsToProviders(m));
        }).catch(() => setProviders([])); // Removed static fallback completely

        // Check if user drafted a RAG agent from Playground
        const draftConfigStr = sessionStorage.getItem("draftRagConfig");
        if (draftConfigStr) {
            try {
                const draft = JSON.parse(draftConfigStr);
                resetForm();
                
                // Override with Playground settings
                setFormUseRag(draft.use_rag ?? true);
                setFormUseKG(draft.use_knowledge_graph ?? false);
                setFormUsePageIndex(draft.use_pageindex ?? false);
                
                if (draft.provider) setFormProvider(draft.provider);
                if (draft.model_id) setFormModelId(draft.model_id);
                
                if (draft.rag_params?.weights) setFormWeights(draft.rag_params.weights);
                if (draft.rag_params?.advanced) setFormAdvanced(prev => ({...prev, ...draft.rag_params.advanced}));
                if (draft.rerank_config) setFormRerank(draft.rerank_config);
                
                // Pre-fill a nice placeholder
                setFormDisplayName("Custom RAG Agent");
                setFormName("custom_rag_agent");
                setFormDescription("Agent created from RAG Playground settings.");
                
                // Navigate to RAG tab in builder
                setView("builder");
                setActiveTab("rag");
            } catch (e) {
                console.error("Failed to parse draftRagConfig", e);
            } finally {
                sessionStorage.removeItem("draftRagConfig"); // consume it
            }
        }
    }, [/* intentionally run once on mount */]);

    useEffect(() => {
        chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [chatMessages]);

    // Ensure formModelId is valid for the currently selected formProvider
    useEffect(() => {
        if (view === "builder" && providers.length > 0) {
            const p = providers.find(x => x.id === formProvider);
            if (p && p.models.length > 0) {
                if (!p.models.some(m => m.id === formModelId)) {
                    setFormModelId(p.models[0].id);
                }
            }
        }
    }, [formProvider, formModelId, providers, view]);

    // ─── Builder helpers ────────────────────────────────────────────────────────

    const resetForm = useCallback(() => {
        setFormName(""); setFormDisplayName(""); setFormDescription("");
        setFormSystemPrompt(""); setFormModelId("llama3.2"); setFormProvider("ollama");
        setFormTemperature(0.7); setFormMaxTokens(2048); setFormTopK(5);
        setFormUseRag(true); setFormUseKG(false); setFormUsePageIndex(false);
        setFormWeights({ vector: 0.5, tree: 0.3, graph: 0.2 });
        setFormShowAdvanced(false);
        setFormAdvanced({ top_k_per_source: 10, vector_alpha: 0.7, vector_threshold: 0.3, graph_hops: 2 });
        setFormRerank({ enabled: false, strategy: "rrf", model: "BAAI/bge-reranker-v2-m3", final_top_k: 5 });
        setFormTools([]); setFormMcpServers([]); setFormTraits([]); setFormGreeting(""); setFormTemplateId(null);
        setFormOutputFormat("auto");
        setEditingAgent(null); setActiveTab("basic");
    }, []);

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
        setFormUsePageIndex((a as any).use_pageindex ?? false);
        const rp = (a as any).rag_params;
        if (rp?.weights) setFormWeights(rp.weights);
        if (rp?.advanced) setFormAdvanced({ ...formAdvanced, ...rp.advanced });
        const rc = (a as any).rerank_config;
        if (rc) setFormRerank({ ...formRerank, ...rc });
        setFormTools(a.tools || []); setFormMcpServers((a as any).mcp_servers || []); setFormTraits(a.personality_traits || []);
        setFormGreeting(a.greeting || ""); setFormTemplateId(a.template_id || null);
        if (rp?.output_format) setFormOutputFormat(rp.output_format);
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
                use_pageindex: formUsePageIndex,
                rag_params: {
                    weights: formWeights,
                    advanced: formAdvanced,
                    output_format: formOutputFormat,
                },
                rerank_config: formRerank,
                tools: formTools.length > 0 ? formTools : undefined,
                mcp_servers: formMcpServers.length > 0 ? formMcpServers : undefined,
                personality_traits: formTraits.length > 0 ? formTraits : undefined,
                greeting: formGreeting || undefined, template_id: formTemplateId || undefined,
            };

            if (editingAgent) {
                const updated = await updateAgent(editingAgent.id, data);
                // Manually update the state to bypass any fetch caching layer
                setAgents(prev => prev.map(a => a.id === updated.id ? updated : a));
                if (selectedAgent && selectedAgent.id === updated.id) {
                    setSelectedAgent(updated);
                }
                setView("chat");
            } else {
                await createAgent(data);
                await loadAgents();
                setView("list");
            }
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
        setUseBifrost(agent.is_published ? useBifrost : false);
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
            if (useBifrost) {
                // Generate a session ID if one doesn't exist
                const currentSessionId = chatSessionId || Math.random().toString(36).substring(7);
                const resp = await agentBifrostChat(selectedAgent.id, msg, currentSessionId);
                setChatSessionId(currentSessionId);
                setChatMessages(prev => [...prev, {
                    role: "assistant",
                    content: resp.answer,
                    reasoning: resp.reasoning,
                    trace_id: resp.trace_id,
                    steps: resp.steps,
                }]);
            } else {
                const resp = await agentChat(selectedAgent.id, msg, chatSessionId || undefined);
                setChatSessionId(resp.session_id);
                setChatMessages(prev => [...prev, {
                    role: "assistant",
                    content: resp.content,
                    reasoning: resp.reasoning,
                    latency_ms: resp.latency_ms,
                    input_tokens: resp.input_tokens,
                    output_tokens: resp.output_tokens,
                }]);
            }
        } catch (err: any) {
            setChatMessages(prev => [...prev, {
                role: "assistant",
                content: `Error: ${err.message}`,
            }]);
        } finally {
            setChatSending(false);
        }
    };

    const handleViewTrace = async (traceId: string) => {
        // Will be expanded to fetch via REST API or redirect if not found
        const laminarProjectId = process.env.NEXT_PUBLIC_LAMINAR_PROJECT_ID || "8d0bc7f0-2bbc-4515-b531-f9fb7df422a0";
        const url = `http://laminar.asgard.internal/project/${laminarProjectId}/traces/${traceId}`;
        window.open(url, '_blank');
    };

    const copyApiKey = (key: string) => {
        navigator.clipboard.writeText(key);
        setCopiedKey(true);
        setTimeout(() => setCopiedKey(false), 2000);
    };

    // ─── Tool options ───────────────────────────────────────────────────────────

    // Tool ids MUST match the names the Bifrost runtime recognizes (skills.rs kb_tool_label +
    // overseer.rs RAG/memvid wiring). UI-created agents only work when these strings match exactly.
    const availableTools = ["vector_search", "graph_search", "tree_search", "memvid_agent_memory_search", "search_primekg", "search_clinical_kb"];
    // Friendly display names (the underlying ids must stay as the runtime tool names above).
    const toolLabels: Record<string, string> = {
        vector_search: "Vector Search",
        graph_search: "Graph Search",
        tree_search: "Tree Search",
        memvid_agent_memory_search: "Memory Search",
        search_primekg: "PrimeKG Search",
        search_clinical_kb: "Clinical KB Search",
    };

    const toggleTool = (tool: string) => {
        setFormTools(prev =>
            prev.includes(tool) ? prev.filter(t => t !== tool) : [...prev, tool]
        );
    };

    // ─── Render ─────────────────────────────────────────────────────────────────

    const renderGeneratorDialog = () => {
        if (!showGenerator) return null;
        return (
            <div className="bg-white dark:bg-zinc-900 rounded-2xl border-2 border-indigo-200 dark:border-indigo-800 shadow-xl shadow-indigo-100/50 dark:shadow-none overflow-hidden mb-6">
                <div className="bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 px-6 py-4 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl bg-white/20 backdrop-blur-sm flex items-center justify-center">
                            <Wand2 className="w-5 h-5 text-white" />
                        </div>
                        <div>
                            <h3 className="text-white font-bold text-lg">AI Agent Generator</h3>
                            <p className="text-white/70 text-xs">Describe your agent in natural language</p>
                        </div>
                    </div>
                    <button onClick={() => setShowGenerator(false)} className="p-2 rounded-lg hover:bg-white/20 transition-colors"><X className="w-5 h-5 text-white" /></button>
                </div>

                <div className="p-6 space-y-5">
                    {/* Provider & Model Selection */}
                    <div className="grid grid-cols-2 gap-4">
                        <div>
                            <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">AI Provider</Label>
                            <select value={genProvider}
                                onChange={e => { setGenProvider(e.target.value); setGenModelId(""); }}
                                className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-3 py-2.5 text-sm focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all">
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
                            <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">Model</Label>
                            <select value={genModelId}
                                onChange={e => setGenModelId(e.target.value)}
                                className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-3 py-2.5 text-sm focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all">
                                <option value="">Select a model...</option>
                                {providers.find(p => p.id === genProvider)?.models.map(m => (
                                    <option key={m.id} value={m.id}>{m.display_name || m.id}</option>
                                ))}
                            </select>
                        </div>
                    </div>

                    {/* Prompt Input */}
                    <div>
                        <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">Describe your agent</Label>
                        <textarea
                            value={genPrompt}
                            onChange={e => setGenPrompt(e.target.value)}
                            placeholder="เช่น: สร้าง Agent สำหรับให้คำปรึกษาทางการแพทย์ด้านเด็ก ต้องตอบเป็นภาษาไทย มีความระมัดระวังสูง และต้องขออนุญาตก่อนให้คำแนะนำยา..."
                            className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-4 py-3 text-sm min-h-[100px] resize-y focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all"
                        />
                        <p className="text-xs text-gray-400 mt-1">{genPrompt.length} characters · ยิ่งละเอียดยิ่งแม่นยำ</p>
                    </div>

                    {genError && (
                        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
                            {genError}
                        </div>
                    )}

                    {/* Generate Button */}
                    {!genDraft && (
                        <Button
                            onClick={async () => {
                                if (!genPrompt.trim() || !genModelId) return;
                                setGenLoading(true); setGenError(null);
                                try {
                                    const res = await generateAgent({ prompt: genPrompt, provider: genProvider, model_id: genModelId });
                                    setGenDraft(res.draft);
                                    setGenLatency(res.latency_ms);
                                } catch (err: any) {
                                    setGenError(err.message);
                                } finally {
                                    setGenLoading(false);
                                }
                            }}
                            disabled={!genPrompt.trim() || !genModelId || genLoading}
                            className="w-full bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-700 hover:to-purple-700 text-white py-3 rounded-xl shadow-lg shadow-indigo-200 dark:shadow-none text-sm font-medium"
                        >
                            {genLoading ? (
                                <><Loader2 className="w-4 h-4 animate-spin mr-2" /> AI is drafting your agent...</>
                            ) : (
                                <><Wand2 className="w-4 h-4 mr-2" /> Generate Agent Config</>
                            )}
                        </Button>
                    )}

                    {/* Draft Preview */}
                    {genDraft && (
                        <div className="space-y-4">
                            <div className="flex items-center justify-between">
                                <div className="flex items-center gap-2">
                                    <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                                    <span className="text-sm font-medium text-green-700 dark:text-green-400">Draft Generated</span>
                                    {genLatency && <span className="text-xs text-gray-400">({genLatency}ms)</span>}
                                </div>
                                <Button variant="ghost" size="sm" onClick={() => setGenDraft(null)} className="text-xs">Regenerate</Button>
                            </div>

                            <div className="bg-gray-50 dark:bg-zinc-800 rounded-xl p-5 space-y-3 border border-gray-100 dark:border-zinc-700">
                                <div className="flex items-center gap-3">
                                    <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white font-bold text-xl shadow-md">
                                        {genDraft.display_name.charAt(0).toUpperCase()}
                                    </div>
                                    <div>
                                        <h4 className="font-bold text-base">{genDraft.display_name}</h4>
                                        <p className="text-xs text-gray-500 font-mono">{genDraft.name} · {genDraft.provider}/{(genDraft.model_id || '').split('/').pop()}</p>
                                    </div>
                                </div>
                                <p className="text-sm text-gray-600 dark:text-zinc-400">{genDraft.description}</p>

                                <div className="flex flex-wrap gap-1.5">
                                    {genDraft.use_rag && <span className="text-[10px] font-medium bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400 px-2 py-0.5 rounded-full">RAG</span>}
                                    {genDraft.use_knowledge_graph && <span className="text-[10px] font-medium bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-400 px-2 py-0.5 rounded-full">KG</span>}
                                    {genDraft.use_pageindex && <span className="text-[10px] font-medium bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 px-2 py-0.5 rounded-full">Tree</span>}
                                    {genDraft.personality_traits.map(t => (
                                        <span key={t} className="text-[10px] font-medium bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 px-2 py-0.5 rounded-full">{t}</span>
                                    ))}
                                    <span className="text-[10px] font-medium bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400 px-2 py-0.5 rounded-full">temp: {genDraft.temperature}</span>
                                </div>

                                <details className="mt-2">
                                    <summary className="text-xs text-gray-500 cursor-pointer hover:text-gray-700">View System Prompt ({genDraft.system_prompt.length} chars)</summary>
                                    <pre className="mt-2 text-xs bg-white dark:bg-zinc-900 border border-gray-200 dark:border-zinc-700 rounded-lg p-3 whitespace-pre-wrap max-h-[200px] overflow-y-auto">{genDraft.system_prompt}</pre>
                                </details>

                                {genDraft.greeting && (
                                    <div className="mt-2 text-sm bg-white dark:bg-zinc-900 border border-gray-200 dark:border-zinc-700 rounded-lg p-3 italic text-gray-600 dark:text-zinc-400">
                                        💬 {genDraft.greeting}
                                    </div>
                                )}
                            </div>

                            <div className="flex gap-3">
                                <Button variant="outline" className="flex-1" onClick={() => {
                                    // Load into manual builder for tweaking
                                    setFormName(genDraft.name);
                                    setFormDisplayName(genDraft.display_name);
                                    setFormDescription(genDraft.description);
                                    setFormSystemPrompt(genDraft.system_prompt);
                                    setFormModelId(genDraft.model_id);
                                    setFormProvider(genDraft.provider);
                                    setFormTemperature(genDraft.temperature);
                                    setFormMaxTokens(genDraft.max_tokens);
                                    setFormUseRag(genDraft.use_rag);
                                    setFormUseKG(genDraft.use_knowledge_graph);
                                    setFormUsePageIndex(genDraft.use_pageindex || false);
                                    setFormTools(genDraft.tools);
                                    setFormTraits(genDraft.personality_traits);
                                    setFormGreeting(genDraft.greeting);
                                    setShowGenerator(false);
                                    setView("builder");
                                }}>
                                    <Edit className="w-4 h-4 mr-2" /> Edit Before Saving
                                </Button>
                                <Button
                                    className="flex-1 bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white shadow-lg shadow-green-200 dark:shadow-none"
                                    disabled={genSaving}
                                    onClick={async () => {
                                        setGenSaving(true);
                                        try {
                                            const created = await createAgent({
                                                name: genDraft.name,
                                                display_name: genDraft.display_name,
                                                description: genDraft.description,
                                                system_prompt: genDraft.system_prompt,
                                                model_id: genDraft.model_id,
                                                provider: genDraft.provider,
                                                temperature: genDraft.temperature,
                                                max_tokens: genDraft.max_tokens,
                                                use_rag: genDraft.use_rag,
                                                use_knowledge_graph: genDraft.use_knowledge_graph,
                                                use_pageindex: genDraft.use_pageindex || false,
                                                tools: genDraft.tools,
                                                personality_traits: genDraft.personality_traits,
                                                greeting: genDraft.greeting,
                                                tier: genDraft.tier,
                                            });
                                            await loadAgents();
                                            setShowGenerator(false);
                                            setGenDraft(null);
                                            
                                            // Ensure created agent populates form
                                            setFormName(created.name);
                                            setFormDisplayName(created.display_name || "");
                                            setFormDescription(created.description || "");
                                            setFormSystemPrompt(created.system_prompt || "");
                                            setFormModelId(created.model_id);
                                            setFormProvider(created.provider);
                                            setFormTemperature(created.temperature || 0.7);
                                            setFormMaxTokens(created.max_tokens || 2048);
                                            setFormUseRag(created.use_rag || false);
                                            setFormUseKG(created.use_knowledge_graph || false);
                                            setFormUsePageIndex(created.use_pageindex || false);
                                            setFormTools(created.tools || []);
                                            setFormTraits(created.personality_traits || []);
                                            setFormGreeting(created.greeting || "");
                                            setView("builder");
                                        } catch (err: any) {
                                            setGenError(err.message);
                                        } finally {
                                            setGenSaving(false);
                                        }
                                    }}
                                >
                                    {genSaving ? <Loader2 className="w-4 h-4 animate-spin mr-2" /> : <Save className="w-4 h-4 mr-2" />}
                                    Save Draft Directly
                                </Button>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        );
    };

    // --- LIST VIEW ---
    if (view === "list") {
        const publishedCount = agents.filter(a => a.is_published).length;
        const draftCount = agents.length - publishedCount;
        const agentColors: Record<string, string> = {
            heimdall: "from-violet-500 to-purple-600",
            ollama: "from-emerald-500 to-teal-600",
            gemini: "from-blue-500 to-cyan-600",
            openai: "from-gray-700 to-gray-900",
        };

        return (
            <div className="container mx-auto p-6 space-y-6 max-w-7xl">
                {/* Header */}
                <div className="flex items-end justify-between">
                    <div>
                        <h1 className="text-3xl font-bold bg-gradient-to-r from-purple-600 to-pink-600 bg-clip-text text-transparent">
                            Agent Studio
                        </h1>
                        <p className="text-gray-500 mt-1">Build, test, and deploy AI agents — no code required</p>
                    </div>
                    <div className="flex items-center gap-3">
                        <Button variant="outline" onClick={() => { setShowGenerator(true); setGenDraft(null); setGenError(null); setGenPrompt(""); }} className="hidden sm:flex bg-gradient-to-r from-indigo-50 to-purple-50 dark:from-indigo-900/20 dark:to-purple-900/20 border-indigo-200 dark:border-indigo-800 hover:border-indigo-400">
                            <Wand2 className="w-4 h-4 mr-2 text-indigo-600" /> Generate with AI
                        </Button>
                        <Button variant="outline" onClick={() => { resetForm(); setShowTemplates(true); setView("builder"); }} className="hidden sm:flex">
                            <Sparkles className="w-4 h-4 mr-2" /> From Template
                        </Button>
                        <Button
                            onClick={() => { resetForm(); setView("builder"); }}
                            className="bg-gradient-to-r from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white shadow-lg shadow-purple-200 dark:shadow-none"
                        >
                            <Plus className="w-4 h-4 mr-2" /> New Agent
                        </Button>
                    </div>
                </div>

                {/* Stats Bar */}
                {agents.length > 0 && (
                    <div className="grid grid-cols-3 gap-4">
                        <div className="bg-white dark:bg-zinc-900 rounded-xl border border-gray-100 dark:border-zinc-800 px-5 py-4 flex items-center gap-4">
                            <div className="w-10 h-10 rounded-lg bg-purple-100 dark:bg-purple-900/30 flex items-center justify-center">
                                <Bot className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold">{agents.length}</p>
                                <p className="text-xs text-gray-500">Total Agents</p>
                            </div>
                        </div>
                        <div className="bg-white dark:bg-zinc-900 rounded-xl border border-gray-100 dark:border-zinc-800 px-5 py-4 flex items-center gap-4">
                            <div className="w-10 h-10 rounded-lg bg-green-100 dark:bg-green-900/30 flex items-center justify-center">
                                <Rocket className="w-5 h-5 text-green-600 dark:text-green-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold">{publishedCount}</p>
                                <p className="text-xs text-gray-500">Published</p>
                            </div>
                        </div>
                        <div className="bg-white dark:bg-zinc-900 rounded-xl border border-gray-100 dark:border-zinc-800 px-5 py-4 flex items-center gap-4">
                            <div className="w-10 h-10 rounded-lg bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center">
                                <Edit className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold">{draftCount}</p>
                                <p className="text-xs text-gray-500">Drafts</p>
                            </div>
                        </div>
                    </div>
                )}

                {/* Error */}
                {error && (
                    <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-xl flex justify-between items-center">
                        <span className="text-sm">{error}</span>
                        <button onClick={() => setError(null)} className="p-1 hover:bg-red-100 rounded"><X className="w-4 h-4" /></button>
                    </div>
                )}

                {/* ═══ AI Generator Dialog ═══ */}
                {showGenerator && (
                    <div className="bg-white dark:bg-zinc-900 rounded-2xl border-2 border-indigo-200 dark:border-indigo-800 shadow-xl shadow-indigo-100/50 dark:shadow-none overflow-hidden">
                        <div className="bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 px-6 py-4 flex items-center justify-between">
                            <div className="flex items-center gap-3">
                                <div className="w-10 h-10 rounded-xl bg-white/20 backdrop-blur-sm flex items-center justify-center">
                                    <Wand2 className="w-5 h-5 text-white" />
                                </div>
                                <div>
                                    <h3 className="text-white font-bold text-lg">AI Agent Generator</h3>
                                    <p className="text-white/70 text-xs">Describe your agent in natural language</p>
                                </div>
                            </div>
                            <button onClick={() => setShowGenerator(false)} className="p-2 rounded-lg hover:bg-white/20 transition-colors"><X className="w-5 h-5 text-white" /></button>
                        </div>

                        <div className="p-6 space-y-5">
                            {/* Provider & Model Selection */}
                            <div className="grid grid-cols-2 gap-4">
                                <div>
                                    <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">AI Provider</Label>
                                    <select value={genProvider}
                                        onChange={e => { setGenProvider(e.target.value); setGenModelId(""); }}
                                        className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-3 py-2.5 text-sm focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all">
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
                                    <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">Model</Label>
                                    <select value={genModelId}
                                        onChange={e => setGenModelId(e.target.value)}
                                        className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-3 py-2.5 text-sm focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all">
                                        <option value="">Select a model...</option>
                                        {providers.find(p => p.id === genProvider)?.models.map(m => (
                                            <option key={m.id} value={m.id}>{m.display_name || m.id}</option>
                                        ))}
                                    </select>
                                </div>
                            </div>

                            {/* Prompt Input */}
                            <div>
                                <Label className="text-sm font-medium text-gray-700 dark:text-zinc-300">Describe your agent</Label>
                                <textarea
                                    value={genPrompt}
                                    onChange={e => setGenPrompt(e.target.value)}
                                    placeholder="เช่น: สร้าง Agent สำหรับให้คำปรึกษาทางการแพทย์ด้านเด็ก ต้องตอบเป็นภาษาไทย มีความระมัดระวังสูง และต้องขออนุญาตก่อนให้คำแนะนำยา..."
                                    className="mt-1.5 w-full rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 px-4 py-3 text-sm min-h-[100px] resize-y focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all"
                                />
                                <p className="text-xs text-gray-400 mt-1">{genPrompt.length} characters · ยิ่งละเอียดยิ่งแม่นยำ</p>
                            </div>

                            {genError && (
                                <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
                                    {genError}
                                </div>
                            )}

                            {/* Generate Button */}
                            {!genDraft && (
                                <Button
                                    onClick={async () => {
                                        if (!genPrompt.trim() || !genModelId) return;
                                        setGenLoading(true); setGenError(null);
                                        try {
                                            const res = await generateAgent({ prompt: genPrompt, provider: genProvider, model_id: genModelId });
                                            setGenDraft(res.draft);
                                            setGenLatency(res.latency_ms);
                                        } catch (err: any) {
                                            setGenError(err.message);
                                        } finally {
                                            setGenLoading(false);
                                        }
                                    }}
                                    disabled={!genPrompt.trim() || !genModelId || genLoading}
                                    className="w-full bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-700 hover:to-purple-700 text-white py-3 rounded-xl shadow-lg shadow-indigo-200 dark:shadow-none text-sm font-medium"
                                >
                                    {genLoading ? (
                                        <><Loader2 className="w-4 h-4 animate-spin mr-2" /> AI is drafting your agent...</>
                                    ) : (
                                        <><Wand2 className="w-4 h-4 mr-2" /> Generate Agent Config</>
                                    )}
                                </Button>
                            )}

                            {/* Draft Preview */}
                            {genDraft && (
                                <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                                            <span className="text-sm font-medium text-green-700 dark:text-green-400">Draft Generated</span>
                                            {genLatency && <span className="text-xs text-gray-400">({genLatency}ms)</span>}
                                        </div>
                                        <Button variant="ghost" size="sm" onClick={() => setGenDraft(null)} className="text-xs">Regenerate</Button>
                                    </div>

                                    <div className="bg-gray-50 dark:bg-zinc-800 rounded-xl p-5 space-y-3 border border-gray-100 dark:border-zinc-700">
                                        <div className="flex items-center gap-3">
                                            <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white font-bold text-xl shadow-md">
                                                {genDraft.display_name.charAt(0).toUpperCase()}
                                            </div>
                                            <div>
                                                <h4 className="font-bold text-base">{genDraft.display_name}</h4>
                                                <p className="text-xs text-gray-500 font-mono">{genDraft.name} · {genDraft.provider}/{(genDraft.model_id || '').split('/').pop()}</p>
                                            </div>
                                        </div>
                                        <p className="text-sm text-gray-600 dark:text-zinc-400">{genDraft.description}</p>

                                        <div className="flex flex-wrap gap-1.5">
                                            {genDraft.use_rag && <span className="text-[10px] font-medium bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400 px-2 py-0.5 rounded-full">RAG</span>}
                                            {genDraft.use_knowledge_graph && <span className="text-[10px] font-medium bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-400 px-2 py-0.5 rounded-full">KG</span>}
                                            {genDraft.use_pageindex && <span className="text-[10px] font-medium bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 px-2 py-0.5 rounded-full">Tree</span>}
                                            {genDraft.personality_traits.map(t => (
                                                <span key={t} className="text-[10px] font-medium bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 px-2 py-0.5 rounded-full">{t}</span>
                                            ))}
                                            <span className="text-[10px] font-medium bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400 px-2 py-0.5 rounded-full">temp: {genDraft.temperature}</span>
                                        </div>

                                        <details className="mt-2">
                                            <summary className="text-xs text-gray-500 cursor-pointer hover:text-gray-700">View System Prompt ({genDraft.system_prompt.length} chars)</summary>
                                            <pre className="mt-2 text-xs bg-white dark:bg-zinc-900 border border-gray-200 dark:border-zinc-700 rounded-lg p-3 whitespace-pre-wrap max-h-[200px] overflow-y-auto">{genDraft.system_prompt}</pre>
                                        </details>

                                        {genDraft.greeting && (
                                            <div className="mt-2 text-sm bg-white dark:bg-zinc-900 border border-gray-200 dark:border-zinc-700 rounded-lg p-3 italic text-gray-600 dark:text-zinc-400">
                                                💬 {genDraft.greeting}
                                            </div>
                                        )}
                                    </div>

                                    <div className="flex gap-3">
                                        <Button variant="outline" className="flex-1" onClick={() => {
                                            // Load into manual builder for tweaking
                                            setFormName(genDraft.name);
                                            setFormDisplayName(genDraft.display_name);
                                            setFormDescription(genDraft.description);
                                            setFormSystemPrompt(genDraft.system_prompt);
                                            setFormModelId(genDraft.model_id);
                                            setFormProvider(genDraft.provider);
                                            setFormTemperature(genDraft.temperature);
                                            setFormMaxTokens(genDraft.max_tokens);
                                            setFormUseRag(genDraft.use_rag);
                                            setFormUseKG(genDraft.use_knowledge_graph);
                                            setFormUsePageIndex(genDraft.use_pageindex || false);
                                            setFormTools(genDraft.tools);
                                            setFormTraits(genDraft.personality_traits);
                                            setFormGreeting(genDraft.greeting);
                                            setShowGenerator(false);
                                            setView("builder");
                                        }}>
                                            <Edit className="w-4 h-4 mr-2" /> Edit Before Saving
                                        </Button>
                                        <Button
                                            className="flex-1 bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white shadow-lg shadow-green-200 dark:shadow-none"
                                            disabled={genSaving}
                                            onClick={async () => {
                                                setGenSaving(true);
                                                try {
                                                    await createAgent({
                                                        name: genDraft.name,
                                                        display_name: genDraft.display_name,
                                                        description: genDraft.description,
                                                        system_prompt: genDraft.system_prompt,
                                                        model_id: genDraft.model_id,
                                                        provider: genDraft.provider,
                                                        temperature: genDraft.temperature,
                                                        max_tokens: genDraft.max_tokens,
                                                        use_rag: genDraft.use_rag,
                                                        use_knowledge_graph: genDraft.use_knowledge_graph,
                                                        use_pageindex: genDraft.use_pageindex || false,
                                                        tools: genDraft.tools,
                                                        personality_traits: genDraft.personality_traits,
                                                        greeting: genDraft.greeting,
                                                        tier: genDraft.tier,
                                                    });
                                                    await loadAgents();
                                                    setShowGenerator(false);
                                                    setGenDraft(null);
                                                } catch (err: any) {
                                                    setGenError(err.message);
                                                } finally {
                                                    setGenSaving(false);
                                                }
                                            }}
                                        >
                                            {genSaving ? <Loader2 className="w-4 h-4 animate-spin mr-2" /> : <Check className="w-4 h-4 mr-2" />}
                                            Save Agent
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {/* Loading */}
                {loading ? (
                    <div className="flex flex-col items-center justify-center py-20 gap-3">
                        <Loader2 className="w-8 h-8 animate-spin text-purple-500" />
                        <p className="text-sm text-gray-400">Loading agents...</p>
                    </div>
                ) : agents.length === 0 ? (
                    /* Empty state */
                    <div className="border-2 border-dashed border-purple-200 dark:border-purple-800 rounded-2xl">
                        <div className="flex flex-col items-center justify-center py-20">
                            <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-purple-100 to-pink-100 dark:from-purple-900/40 dark:to-pink-900/40 flex items-center justify-center mb-5">
                                <Brain className="w-10 h-10 text-purple-400" />
                            </div>
                            <h3 className="text-xl font-semibold text-gray-700 dark:text-zinc-300">No agents yet</h3>
                            <p className="text-gray-500 mt-2 mb-8 text-sm">Create your first AI agent from scratch or start with a template</p>
                            <div className="flex gap-3">
                                <Button onClick={() => { resetForm(); setView("builder"); }}
                                    className="bg-gradient-to-r from-purple-600 to-pink-600 text-white shadow-lg shadow-purple-200 dark:shadow-none">
                                    <Plus className="w-4 h-4 mr-2" /> Create Agent
                                </Button>
                                <Button variant="outline" onClick={() => { resetForm(); setShowTemplates(true); setView("builder"); }}>
                                    <Sparkles className="w-4 h-4 mr-2" /> Use Template
                                </Button>
                            </div>
                        </div>
                    </div>
                ) : (
                    /* Agent cards grouped by category */
                    <div className="space-y-12">
                        {groupedAgents.map(group => (
                            <div key={group.id} className="space-y-4">
                                {/* Group Header */}
                                <div className="flex items-center gap-3 pb-3 border-b-2" style={{ borderColor: group.color_hex + "40" }}>
                                    <div className="text-3xl">{group.icon_emoji}</div>
                                    <div>
                                        <h2 className="text-2xl font-bold" style={{ color: group.color_hex }}>
                                            {group.display_name}
                                        </h2>
                                        <p className="text-xs text-gray-500">{group.agents.length} agent{group.agents.length !== 1 ? 's' : ''}</p>
                                    </div>
                                </div>

                                {/* Agents Grid */}
                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
                                    {group.agents.map(agent => {
                                        const gradient = agentColors[agent.provider] || "from-purple-500 to-pink-500";
                                        return (
                                            <div key={agent.id}
                                                className="bg-white dark:bg-zinc-900 rounded-2xl border-2 transition-all duration-300 cursor-pointer overflow-hidden group hover:shadow-lg"
                                                style={{
                                                    borderColor: group.color_hex + "40",
                                                    backgroundColor: "white"
                                                }}
                                                onClick={() => openChat(agent)}>
                                                {/* Gradient top bar with group color */}
                                                <div className="h-1.5" style={{ backgroundColor: group.color_hex }} />
                                                <div className="p-5">
                                                    {/* Header */}
                                                    <div className="flex items-start justify-between mb-3">
                                                        <div className="flex items-center gap-3">
                                                            <div className={`w-11 h-11 rounded-xl bg-gradient-to-br ${gradient} flex items-center justify-center text-white font-bold text-lg shadow-md`}>
                                                                {(agent.display_name || agent.name).charAt(0).toUpperCase()}
                                                            </div>
                                                            <div>
                                                                <h3 className="font-semibold text-[15px] leading-tight flex items-center gap-1.5">
                                                                    {agent.display_name || agent.name}
                                                                    {champions[agent.name] && (
                                                                        <span title={`Champion: ${champions[agent.name].name} · acc captured · cost $${(champions[agent.name].total_cost_usd ?? 0).toFixed(4)}`}
                                                                            className="inline-flex items-center gap-1 text-[10px] font-medium px-1.5 py-0.5 rounded bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">
                                                                            <Crown className="w-3 h-3" /> Champion
                                                                        </span>
                                                                    )}
                                                                </h3>
                                                                <p className="text-xs text-gray-400 mt-0.5">{agent.provider} · {(agent.model_id || '').split('/').pop()}</p>
                                                            </div>
                                                        </div>
                                                        <div className="flex items-center gap-1.5">
                                                            <Badge variant={agent.is_published ? "default" : "secondary"}
                                                                className={`text-[10px] px-2 ${agent.is_published ? "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-400 border-green-200" : "bg-gray-100 text-gray-500"}`}>
                                                                {agent.is_published ? "● Live" : "Draft"}
                                                            </Badge>
                                                            <Badge variant="outline" className="text-[10px] px-1.5 py-0 font-mono">
                                                                T{agent.tier || 2}
                                                            </Badge>
                                                        </div>
                                                    </div>

                                                    {/* Description */}
                                                    <p className="text-sm text-gray-500 dark:text-zinc-400 line-clamp-2 mb-4 min-h-[40px]">
                                                        {agent.description || "No description"}
                                                    </p>

                                                    {/* Feature badges */}
                                                    <div className="flex flex-wrap gap-1.5 mb-4">
                                                        {agent.use_rag && <span className="inline-flex items-center gap-1 text-[10px] font-medium bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 px-2 py-0.5 rounded-full"><Database className="w-2.5 h-2.5" />RAG</span>}
                                                        {agent.use_knowledge_graph && <span className="inline-flex items-center gap-1 text-[10px] font-medium bg-emerald-50 dark:bg-emerald-900/20 text-emerald-600 dark:text-emerald-400 px-2 py-0.5 rounded-full"><Globe className="w-2.5 h-2.5" />KG</span>}
                                                        {agent.tools && (agent.tools as string[]).length > 0 && (
                                                            <span className="inline-flex items-center gap-1 text-[10px] font-medium bg-amber-50 dark:bg-amber-900/20 text-amber-600 dark:text-amber-400 px-2 py-0.5 rounded-full"><Wrench className="w-2.5 h-2.5" />{(agent.tools as string[]).length} tools</span>
                                                        )}
                                                    </div>

                                                    {/* Action bar — always visible */}
                                                    <div className="flex items-center gap-1 pt-3 border-t border-gray-100 dark:border-zinc-800" onClick={e => e.stopPropagation()}>
                                                        <button onClick={() => openChat(agent)} className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-purple-600 px-2.5 py-1.5 rounded-lg hover:bg-purple-50 dark:hover:bg-purple-900/20 transition-colors">
                                                            <MessageSquare className="w-3.5 h-3.5" /> Chat
                                                        </button>

                                                        <button onClick={() => { loadAgentToForm(agent); setView("builder"); }} className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-blue-600 px-2.5 py-1.5 rounded-lg hover:bg-blue-50 dark:hover:bg-blue-900/20 transition-colors">
                                                            <Edit className="w-3.5 h-3.5" /> Edit
                                                        </button>
                                                        <button onClick={() => openEvalModal(agent)} className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-rose-600 px-2.5 py-1.5 rounded-lg hover:bg-rose-50 dark:hover:bg-rose-900/20 transition-colors">
                                                            <BarChart3 className="w-3.5 h-3.5" /> Evaluate
                                                        </button>
                                                        {!agent.is_published && (
                                                            <button onClick={() => handlePublish(agent.id)} className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-green-600 px-2.5 py-1.5 rounded-lg hover:bg-green-50 dark:hover:bg-green-900/20 transition-colors">
                                                                <Rocket className="w-3.5 h-3.5" /> Publish
                                                            </button>
                                                        )}
                                                        <button onClick={() => handleDelete(agent.id)} className="flex items-center gap-1 text-xs text-gray-400 hover:text-red-500 px-2 py-1.5 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors ml-auto">
                                                            <Trash2 className="w-3.5 h-3.5" />
                                                        </button>
                                                    </div>

                                                    {/* API key row */}
                                                    {agent.is_published && agent.api_key && (
                                                        <div className="mt-3 flex items-center gap-2 text-xs bg-gray-50 dark:bg-zinc-800 rounded-lg px-3 py-2" onClick={e => e.stopPropagation()}>
                                                            <code className="truncate flex-1 font-mono text-[11px] text-gray-500">{agent.api_key}</code>
                                                            <button onClick={() => copyApiKey(agent.api_key!)} className="p-1 hover:bg-gray-200 dark:hover:bg-zinc-700 rounded transition-colors">
                                                                {copiedKey ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3 text-gray-400" />}
                                                            </button>
                                                        </div>
                                                    )}
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        ))}
                    </div>
                )}

                {/* ─── Eval Modal ─────────────────────────────────────────── */}
                {evalAgent && (
                    <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={() => setEvalAgent(null)}>
                        <div className="bg-white dark:bg-zinc-900 rounded-xl shadow-2xl max-w-2xl w-full max-h-[90vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
                            <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-zinc-800">
                                <div className="flex items-center gap-3">
                                    <BarChart3 className="w-5 h-5 text-rose-600" />
                                    <h2 className="font-semibold">Evaluate · {evalAgent.display_name || evalAgent.name}</h2>
                                </div>
                                <button onClick={() => setEvalAgent(null)} className="p-1 rounded hover:bg-gray-100 dark:hover:bg-zinc-800">
                                    <X className="w-4 h-4" />
                                </button>
                            </div>

                            <div className="p-6 space-y-5">
                                <div>
                                    <Label className="text-xs">Benchmark Dataset</Label>
                                    {benchmarks.length === 0 ? (
                                        <p className="text-sm text-gray-500 mt-2">No benchmark datasets found. Run <code className="text-xs bg-gray-100 dark:bg-zinc-800 px-1 py-0.5 rounded">scripts/import_healthbench.py</code> to populate.</p>
                                    ) : (
                                        <select value={evalBenchmarkId} onChange={e => setEvalBenchmarkId(e.target.value)} className="w-full mt-2 px-3 py-2 rounded-lg border border-gray-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm">
                                            {benchmarks.map(b => (
                                                <option key={b.id} value={b.id}>{b.name} · {b.total_items} items · {b.source}</option>
                                            ))}
                                        </select>
                                    )}
                                </div>

                                <div>
                                    <Label className="text-xs">Max items to evaluate</Label>
                                    <Input type="number" value={evalMaxItems} onChange={e => setEvalMaxItems(Math.max(1, Math.min(525, parseInt(e.target.value) || 1)))} min={1} max={525} className="mt-2" />
                                    <p className="text-xs text-gray-400 mt-1">~9 sec/item via Gemini 3 Flash · 10 items ≈ 90s, 50 ≈ 8min, 525 ≈ 80min</p>
                                </div>

                                <div className="rounded-lg bg-gray-50 dark:bg-zinc-800/50 border border-gray-200 dark:border-zinc-700 p-3 text-xs">
                                    <div className="flex items-center justify-between text-gray-600 dark:text-zinc-400">
                                        <span>Will evaluate model: <code className="font-mono bg-white dark:bg-black/30 px-1.5 py-0.5 rounded">{evalAgent.model_id.split('/').pop()}</code></span>
                                        <span>(via {evalAgent.provider})</span>
                                    </div>
                                </div>

                                {evalStartedRunId && (
                                    <div className="rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 p-3 text-xs">
                                        <p className="font-semibold text-green-800 dark:text-green-200">✅ Run started: <code className="font-mono">{evalStartedRunId.slice(0, 8)}</code></p>
                                        <p className="text-green-700 dark:text-green-300 mt-1">Watch progress in <Link href="/evaluations" className="underline">/evaluations</Link></p>
                                    </div>
                                )}
                                {evalError && (
                                    <div className="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-3 text-xs text-red-700 dark:text-red-300">
                                        ❌ {evalError}
                                    </div>
                                )}

                                {recentRunsForAgent(evalAgent.name).length > 0 && (
                                    <div>
                                        <Label className="text-xs">Recent runs (this agent)</Label>
                                        <div className="mt-2 space-y-1">
                                            {recentRunsForAgent(evalAgent.name).map(r => (
                                                <div key={r.id} className="flex items-center gap-2 text-xs px-3 py-2 rounded-lg bg-gray-50 dark:bg-zinc-800">
                                                    <span className="truncate flex-1 flex items-center gap-1.5">
                                                        {r.is_champion && <Crown className="w-3 h-3 text-amber-500 flex-shrink-0" />}
                                                        <span className="truncate">{r.name || r.id.slice(0, 8)}</span>
                                                        {r.total_cost_usd != null && r.total_cost_usd > 0 && (
                                                            <span className="text-gray-400">· ${r.total_cost_usd.toFixed(4)}</span>
                                                        )}
                                                    </span>
                                                    <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${r.status === "COMPLETED" ? "bg-green-100 text-green-700" : r.status === "RUNNING" ? "bg-blue-100 text-blue-700 animate-pulse" : "bg-gray-100 text-gray-700"}`}>
                                                        {r.completed_combinations}/{r.total_combinations}
                                                    </span>
                                                    {r.status === "COMPLETED" && !r.is_champion && (
                                                        <button onClick={(e) => { e.preventDefault(); handlePromoteRun(r.id); }}
                                                            title="Promote this run to Champion"
                                                            className="text-[10px] text-amber-600 hover:text-amber-800 hover:bg-amber-50 dark:hover:bg-amber-900/20 px-1.5 py-0.5 rounded">
                                                            <Crown className="w-3 h-3 inline" />
                                                        </button>
                                                    )}
                                                    <Link href="/evaluations" className="text-[10px] text-blue-600 hover:underline">View</Link>
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                )}
                            </div>

                            <div className="px-6 py-4 border-t border-gray-200 dark:border-zinc-800 flex justify-end gap-2">
                                <Button variant="outline" onClick={() => setEvalAgent(null)}>Close</Button>
                                <Link href="/evaluations" className="inline-flex items-center gap-2 px-4 py-2 border border-gray-200 dark:border-zinc-700 rounded-lg text-sm hover:bg-gray-50 dark:hover:bg-zinc-800">
                                    <ExternalLink className="w-4 h-4" /> View Results
                                </Link>
                                <Button onClick={handleStartEval} disabled={evalStarting || benchmarks.length === 0} className="bg-rose-600 hover:bg-rose-700 text-white">
                                    {evalStarting ? <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Starting...</> : <><BarChart3 className="w-4 h-4 mr-2" /> Run Evaluation</>}
                                </Button>
                            </div>
                        </div>
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
                        <div className="flex gap-2">
                            <Button variant="outline" size="sm" onClick={() => setShowTemplates(!showTemplates)}>
                                <Sparkles className="w-4 h-4 mr-1" /> Templates
                            </Button>
                            <Button size="sm" variant="outline" className="bg-gradient-to-r from-indigo-50 to-purple-50 dark:from-indigo-900/20 dark:to-purple-900/20 border-indigo-200 dark:border-indigo-800 hover:border-indigo-400" onClick={() => { 
                                setShowGenerator(true); setGenDraft(null); setGenError(null); 
                                const basePrompt = [formDisplayName, formDescription].filter(Boolean).join(" - ");
                                setGenPrompt(basePrompt || ""); 
                            }}>
                                <Wand2 className="w-4 h-4 mr-1 text-indigo-600" /> Generate with AI
                            </Button>
                        </div>
                    )}
                </div>

                {/* AI Generator Dialog in Builder View */}
                {renderGeneratorDialog()}

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
                                        <Label htmlFor="max-tokens">Response Length Limit (Max Tokens)</Label>
                                        <Input id="max-tokens" type="number" value={formMaxTokens}
                                            onChange={e => setFormMaxTokens(parseInt(e.target.value) || 2048)} className="mt-1" />
                                    </div>
                                    <div>
                                        <Label htmlFor="top-k">Source Documents Limit (Top-K)</Label>
                                        <Input id="top-k" type="number" value={formTopK}
                                            onChange={e => setFormTopK(parseInt(e.target.value) || 5)} className="mt-1" />
                                    </div>
                                </div>
                            </>
                        )}

                        {activeTab === "behavior" && (
                            <div className="space-y-6">
                                <div>
                                    <Label htmlFor="system-prompt">Agent Instructions (System Prompt) *</Label>
                                    <textarea id="system-prompt" value={formSystemPrompt}
                                        onChange={e => setFormSystemPrompt(e.target.value)}
                                        placeholder="You are a helpful assistant..."
                                        className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm min-h-[200px] resize-y font-mono"
                                    />
                                    <p className="text-xs text-gray-400 mt-1">{formSystemPrompt.length} characters</p>
                                </div>
                                
                                <div>
                                    <Label className="text-sm font-medium mb-1.5 block">Output Format Constraint</Label>
                                    <div className="flex bg-gray-100 dark:bg-zinc-800 p-1 rounded-lg">
                                        <button 
                                            onClick={() => setFormOutputFormat("auto")}
                                            className={`flex-1 py-2 text-xs font-medium rounded-md transition-all ${formOutputFormat === "auto" ? "bg-white dark:bg-zinc-700 shadow flex items-center justify-center gap-1.5" : "text-gray-500"}`}
                                        >
                                            Auto (Native)
                                        </button>
                                        <button 
                                            onClick={() => setFormOutputFormat("markdown_table")}
                                            className={`flex-1 py-2 text-xs font-medium rounded-md transition-all ${formOutputFormat === "markdown_table" ? "bg-white dark:bg-zinc-700 shadow flex items-center justify-center gap-1.5" : "text-gray-500"}`}
                                        >
                                            Markdown Table
                                        </button>
                                        <button 
                                            onClick={() => setFormOutputFormat("json_chart")}
                                            className={`flex-1 py-2 text-xs font-medium rounded-md transition-all ${formOutputFormat === "json_chart" ? "bg-white dark:bg-zinc-700 shadow flex items-center justify-center gap-1.5" : "text-gray-500"}`}
                                        >
                                            JSON Chart
                                        </button>
                                    </div>
                                    <p className="text-[11px] text-indigo-500 mt-2 font-medium">
                                        🔗 Injected into System Prompt automatically by Bifrost Router at runtime.
                                    </p>
                                </div>
                            </div>
                        )}

                        {activeTab === "rag" && (
                            <div className="space-y-5">
                                {/* Toggle Layer */}
                                <div className="flex flex-wrap items-center gap-4">
                                    <label className="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" checked={formUseRag}
                                            onChange={e => setFormUseRag(e.target.checked)}
                                            className="w-4 h-4 rounded accent-purple-600" />
                                        <span className="text-sm font-medium">🔷 Vector Search</span>
                                    </label>
                                    <label className="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" checked={formUseKG}
                                            onChange={e => setFormUseKG(e.target.checked)}
                                            className="w-4 h-4 rounded accent-purple-600" />
                                        <span className="text-sm font-medium">🔮 Knowledge Graph</span>
                                    </label>
                                    <label className="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" checked={formUsePageIndex}
                                            onChange={e => setFormUsePageIndex(e.target.checked)}
                                            className="w-4 h-4 rounded accent-purple-600" />
                                        <span className="text-sm font-medium">🌿 PageIndex (Tree)</span>
                                    </label>
                                </div>

                                {/* Weight Sliders */}
                                {(formUseRag || formUseKG || formUsePageIndex) && (
                                    <div className="space-y-3 p-4 rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-900/50">
                                        <h4 className="text-sm font-semibold text-gray-700 dark:text-gray-300">Ensemble Weights</h4>
                                        <p className="text-xs text-gray-400">Adjust the contribution ratio of each search source. Total must equal 100%.</p>
                                        {[
                                            { key: "vector" as const, label: "🔷 Vector", color: "bg-blue-500", enabled: formUseRag },
                                            { key: "tree" as const, label: "🌿 Tree", color: "bg-green-500", enabled: formUsePageIndex },
                                            { key: "graph" as const, label: "🔮 Graph", color: "bg-purple-500", enabled: formUseKG },
                                        ].map(s => (
                                            <div key={s.key} className={`flex items-center gap-3 ${!s.enabled ? 'opacity-30' : ''}`}>
                                                <span className="text-sm w-24 font-medium">{s.label}</span>
                                                <input type="range" min={0} max={100} step={5}
                                                    disabled={!s.enabled}
                                                    value={Math.round(formWeights[s.key] * 100)}
                                                    onChange={e => {
                                                        const newVal = parseInt(e.target.value) / 100;
                                                        setFormWeights(prev => {
                                                            const updated = { ...prev, [s.key]: newVal };
                                                            const sum = updated.vector + updated.tree + updated.graph;
                                                            if (sum > 0) {
                                                                updated.vector /= sum; updated.tree /= sum; updated.graph /= sum;
                                                            }
                                                            return updated;
                                                        });
                                                    }}
                                                    className="flex-1 accent-purple-600" />
                                                <span className="text-sm font-mono w-12 text-right">{Math.round(formWeights[s.key] * 100)}%</span>
                                            </div>
                                        ))}
                                        <div className="flex gap-2 mt-2">
                                            <button onClick={() => setFormWeights({ vector: 0.5, tree: 0.3, graph: 0.2 })}
                                                className="text-xs px-2 py-1 rounded bg-gray-200 dark:bg-zinc-700 hover:bg-gray-300 dark:hover:bg-zinc-600">Balanced</button>
                                            <button onClick={() => setFormWeights({ vector: 0.8, tree: 0.1, graph: 0.1 })}
                                                className="text-xs px-2 py-1 rounded bg-gray-200 dark:bg-zinc-700 hover:bg-gray-300 dark:hover:bg-zinc-600">Vector Heavy</button>
                                            <button onClick={() => setFormWeights({ vector: 0.2, tree: 0.1, graph: 0.7 })}
                                                className="text-xs px-2 py-1 rounded bg-gray-200 dark:bg-zinc-700 hover:bg-gray-300 dark:hover:bg-zinc-600">Graph Heavy</button>
                                        </div>
                                    </div>
                                )}

                                {/* Advanced Settings */}
                                <div className="border border-gray-200 dark:border-zinc-700 rounded-lg">
                                    <button onClick={() => setFormShowAdvanced(!formShowAdvanced)}
                                        className="flex items-center justify-between w-full px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-zinc-800 rounded-lg">
                                        <span>▼ Advanced Settings</span>
                                        <span className="text-xs text-gray-400">{formShowAdvanced ? 'Hide' : 'Show'}</span>
                                    </button>
                                    {formShowAdvanced && (
                                        <div className="px-4 pb-4 space-y-3 border-t border-gray-200 dark:border-zinc-700 pt-3">
                                            <div className="grid grid-cols-2 gap-4">
                                                <div>
                                                    <Label className="text-xs">Top-K per Source</Label>
                                                    <Input type="number" min={1} max={50} value={formAdvanced.top_k_per_source}
                                                        onChange={e => setFormAdvanced(p => ({ ...p, top_k_per_source: parseInt(e.target.value) || 10 }))}
                                                        className="mt-1" />
                                                </div>
                                                <div>
                                                    <Label className="text-xs">Vector Alpha (Hybrid Tuning)</Label>
                                                    <Input type="number" min={0} max={1} step={0.1} value={formAdvanced.vector_alpha}
                                                        onChange={e => setFormAdvanced(p => ({ ...p, vector_alpha: parseFloat(e.target.value) || 0.7 }))}
                                                        className="mt-1" />
                                                </div>
                                                <div>
                                                    <Label className="text-xs">Vector Score Threshold</Label>
                                                    <Input type="number" min={0} max={1} step={0.05} value={formAdvanced.vector_threshold}
                                                        onChange={e => setFormAdvanced(p => ({ ...p, vector_threshold: parseFloat(e.target.value) || 0.3 }))}
                                                        className="mt-1" />
                                                </div>
                                                <div>
                                                    <Label className="text-xs">Graph Retrieval Hops</Label>
                                                    <Input type="number" min={1} max={5} value={formAdvanced.graph_hops}
                                                        onChange={e => setFormAdvanced(p => ({ ...p, graph_hops: parseInt(e.target.value) || 2 }))}
                                                        className="mt-1" />
                                                </div>
                                            </div>

                                            {/* Re-ranking Config */}
                                            <div className="mt-4 pt-3 border-t border-gray-200 dark:border-zinc-700">
                                                <label className="flex items-center gap-2 cursor-pointer mb-3">
                                                    <input type="checkbox" checked={formRerank.enabled}
                                                        onChange={e => setFormRerank(p => ({ ...p, enabled: e.target.checked }))}
                                                        className="w-4 h-4 rounded accent-purple-600" />
                                                    <span className="text-sm font-medium">Enable Re-ranking</span>
                                                </label>
                                                {formRerank.enabled && (
                                                    <div className="grid grid-cols-2 gap-4">
                                                        <div>
                                                            <Label className="text-xs">Strategy</Label>
                                                            <select value={formRerank.strategy}
                                                                onChange={e => setFormRerank(p => ({ ...p, strategy: e.target.value as any }))}
                                                                className="mt-1 w-full rounded-md border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-3 py-2 text-sm">
                                                                <option value="rrf">RRF (Reciprocal Rank Fusion)</option>
                                                                <option value="cross_encoder">Cross-Encoder Model</option>
                                                                <option value="llm">LLM-based Re-ranking</option>
                                                            </select>
                                                        </div>
                                                        <div>
                                                            <Label className="text-xs">Final Top-K</Label>
                                                            <Input type="number" min={1} max={20} value={formRerank.final_top_k}
                                                                onChange={e => setFormRerank(p => ({ ...p, final_top_k: parseInt(e.target.value) || 5 }))}
                                                                className="mt-1" />
                                                        </div>
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    )}
                                </div>

                                <p className="text-[11px] text-indigo-500 font-medium">
                                    🔗 RAG Parameters (Weights, Top-K, Alpha) automatically mount to Bifrost Runtime on deploy.
                                </p>
                                <p className="text-xs text-gray-400">
                                    RAG retrieves relevant context from your knowledge base. Knowledge Graph enables structured relationship queries.
                                    PageIndex navigates document structure for hierarchical retrieval.
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
                                            {tool === "vector_search" && <Database className={`w-5 h-5 ${formTools.includes(tool) ? "text-purple-600" : "text-gray-400"}`} />}
                                            {tool === "graph_search" && <Brain className={`w-5 h-5 ${formTools.includes(tool) ? "text-purple-600" : "text-gray-400"}`} />}
                                            {tool === "tree_search" && <LayoutGrid className={`w-5 h-5 ${formTools.includes(tool) ? "text-purple-600" : "text-gray-400"}`} />}
                                            {tool === "memvid_agent_memory_search" && <Clock className={`w-5 h-5 ${formTools.includes(tool) ? "text-purple-600" : "text-gray-400"}`} />}
                                            {tool === "search_primekg" && <Dna className={`w-5 h-5 ${formTools.includes(tool) ? "text-teal-600" : "text-gray-400"}`} />}
                                            {tool === "search_clinical_kb" && <Stethoscope className={`w-5 h-5 ${formTools.includes(tool) ? "text-rose-600" : "text-gray-400"}`} />}
                                            <div>
                                                <span className="text-sm font-medium">{toolLabels[tool] ?? tool}</span>
                                                <p className="text-xs text-gray-400 mt-1">
                                                    {tool === "vector_search" && <span className="text-indigo-600 dark:text-indigo-400 font-medium">Qdrant Vector Database</span>}
                                                    {tool === "graph_search" && <span className="text-fuchsia-600 dark:text-fuchsia-400 font-medium">Knowledge Graph Traversal</span>}
                                                    {tool === "tree_search" && <span className="text-emerald-600 dark:text-emerald-400 font-medium">Hierarchical Doc Tree</span>}
                                                    {tool === "memvid_agent_memory_search" && <span className="text-orange-600 dark:text-orange-400 font-medium">Deep Memory Retrieval</span>}
                                                    {tool === "search_primekg" && <span className="text-teal-600 dark:text-teal-400 font-medium">PrimeKG · 129K Medical Entities</span>}
                                                    {tool === "search_clinical_kb" && <span className="text-rose-600 dark:text-rose-400 font-medium">Clinical Guidelines · Sleep/ENT/Drug/CPAP</span>}
                                                </p>
                                            </div>
                                        </div>
                                    ))}
                                </div>

                                <div className="mt-8 border-t border-gray-100 dark:border-zinc-800 pt-6">
                                    <Label className="mb-2 block">MCP Servers (Optional)</Label>
                                    <p className="text-xs text-gray-400 mb-4">
                                        Attach external tools via the Model Context Protocol (e.g. Hermodr Gateway). 
                                        Provide the full HTTP endpoint URL for each server.
                                    </p>
                                    <div className="space-y-2">
                                        {formMcpServers.map((url, idx) => (
                                            <div key={idx} className="flex items-center gap-2">
                                                <Input 
                                                    value={url} 
                                                    onChange={e => {
                                                        const newMcp = [...formMcpServers];
                                                        newMcp[idx] = e.target.value;
                                                        setFormMcpServers(newMcp);
                                                    }} 
                                                    placeholder="http://hermodr.asgard.svc:9000/mcp"
                                                />
                                                <Button 
                                                    variant="ghost" 
                                                    size="icon" 
                                                    className="text-red-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950/30 flex-shrink-0"
                                                    onClick={() => setFormMcpServers(formMcpServers.filter((_, i) => i !== idx))}>
                                                    <X className="w-4 h-4" />
                                                </Button>
                                            </div>
                                        ))}
                                    </div>
                                    <Button 
                                        variant="outline" 
                                        size="sm" 
                                        className="mt-3 w-full border-dashed"
                                        onClick={() => setFormMcpServers([...formMcpServers, ""])}>
                                        <Plus className="w-4 h-4 mr-2" /> Add MCP Server
                                    </Button>
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
            {/* Left sidebar - Agent Architecture */}
            {selectedAgent && <AgentStructurePanel agent={selectedAgent} />}

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
                        <div className="flex bg-gray-100 dark:bg-zinc-800 p-0.5 rounded-lg border ml-2">
                            <button 
                                onClick={() => setUseBifrost(false)}
                                className={`px-2 py-1 text-xs rounded-md font-medium transition-colors ${!useBifrost ? 'bg-white dark:bg-zinc-700 shadow-sm text-purple-600 dark:text-purple-400' : 'text-gray-500 hover:text-gray-700 dark:hover:text-zinc-300'}`}
                            >
                                Mimir Engine
                            </button>
                            {selectedAgent?.is_published ? (
                                <button 
                                    onClick={() => setUseBifrost(true)}
                                    className={`px-2 py-1 text-xs rounded-md font-medium transition-colors ${useBifrost ? 'bg-gradient-to-r from-purple-600 to-pink-600 text-white shadow-sm' : 'text-gray-500 hover:text-gray-700 dark:hover:text-zinc-300'}`}
                                >
                                    Bifrost Runtime
                                </button>
                            ) : (
                                <button 
                                    disabled
                                    title="Publish the Agent to enable Bifrost Agentic Runtime"
                                    className="px-2 py-1 text-xs rounded-md font-medium transition-colors text-gray-400 opacity-50 cursor-not-allowed"
                                >
                                    Bifrost Runtime
                                </button>
                            )}
                        </div>
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
                                    {msg.role === "assistant" && msg.steps && msg.steps.length > 0 && (
                                        <details className="mb-3 group bg-white/5 dark:bg-black/10 rounded-lg border border-purple-200/50 dark:border-purple-800/30 overflow-hidden">
                                            <summary className="px-3 py-2 text-xs font-semibold text-purple-700 dark:text-purple-300 cursor-pointer list-none flex items-center gap-2 hover:bg-white/10 transition-colors">
                                                <Brain className="w-3.5 h-3.5 group-open:text-purple-500" />
                                                Agent Reasoning ({msg.steps.length} steps)
                                                <ChevronLeft className="w-3 h-3 ml-auto -rotate-90 group-open:rotate-90 transition-transform" />
                                            </summary>
                                            <div className="px-3 pb-3 space-y-2 border-t border-purple-100 dark:border-purple-900/30 pt-2">
                                                {msg.reasoning && (
                                                    <div className="text-[11px] text-purple-800 dark:text-purple-200 mb-2 italic border-b border-purple-100/30 pb-2">
                                                        "{msg.reasoning}"
                                                    </div>
                                                )}
                                                {msg.steps.map((step, idx) => {
                                                    const isLong = step.content.length > 150;
                                                    const displayContent = isLong ? step.content.slice(0, 150) + "…" : step.content;
                                                    const stepColor = step.step_type === "reasoning" 
                                                        ? "text-emerald-700 dark:text-emerald-300" 
                                                        : step.step_type === "self_correction" 
                                                        ? "text-amber-700 dark:text-amber-300" 
                                                        : "text-gray-700 dark:text-gray-300";
                                                    return (
                                                    <div key={idx} className="flex gap-2 text-[11px]">
                                                        <div className="flex-shrink-0 mt-0.5 text-gray-400">
                                                            {step.step_type === "tool_call" ? "🔨" : step.step_type === "self_correction" ? "⚠️" : step.step_type === "reasoning" ? "✅" : "💭"}
                                                        </div>
                                                        <div className="flex-1 min-w-0">
                                                            <div className={`${stepColor} break-words`}>{displayContent}</div>
                                                            <div className="flex items-center gap-2 mt-1 text-gray-400">
                                                                {step.tool_name && <span className="font-mono bg-black/5 dark:bg-white/5 px-1 py-0.5 rounded text-[9px]">{step.tool_name}</span>}
                                                                <span className="flex items-center gap-1"><Clock className="w-2.5 h-2.5" />{step.duration_ms}ms</span>
                                                            </div>
                                                        </div>
                                                    </div>
                                                    );
                                                })}
                                                {msg.trace_id && (
                                                    <div className="mt-3 pt-2 border-t border-purple-100/50 dark:border-purple-900/30">
                                                        <button onClick={() => handleViewTrace(msg.trace_id!)}
                                                            className="flex items-center gap-1.5 text-[10px] bg-purple-100 dark:bg-purple-900/50 hover:bg-purple-200 dark:hover:bg-purple-800 text-purple-700 dark:text-purple-300 px-2 py-1 rounded transition-colors w-full justify-center font-medium">
                                                            <ExternalLink className="w-3 h-3" /> View Laminar Trace
                                                        </button>
                                                    </div>
                                                )}
                                            </div>
                                        </details>
                                    )}
                                    {msg.role === "assistant" ? (
                                        <ReactMarkdown 
                                            remarkPlugins={[remarkGfm]}
                                            components={{
                                                table: ({ node, ...props }) => (
                                                    <div className="my-4 w-full overflow-x-auto rounded-lg border border-gray-200 dark:border-zinc-700">
                                                        <table className="w-full text-sm text-left text-gray-700 dark:text-zinc-300" {...props} />
                                                    </div>
                                                ),
                                                thead: ({ node, ...props }) => (
                                                    <thead className="bg-gray-50 dark:bg-zinc-800/50 text-xs uppercase text-gray-500 dark:text-zinc-400 font-semibold" {...props} />
                                                ),
                                                th: ({ node, ...props }) => <th className="px-4 py-3 border-b dark:border-zinc-700" {...props} />,
                                                td: ({ node, ...props }) => <td className="px-4 py-3 border-b dark:border-zinc-800" {...props} />,
                                                code: ({ node, inline, className, children, ...props }: any) => {
                                                    const match = /language-(\w+)/.exec(className || "");
                                                    const language = match ? match[1] : "";
                                                    
                                                    // Helper to handle chart JSON strings gracefully
                                                    if (!inline && (language === "json" || language === "chart")) {
                                                        try {
                                                            let jsonStr = String(children).replace(/\n$/, "");
                                                            const data = JSON.parse(jsonStr);
                                                            
                                                            // Expecting a specific structure {"chart": { "type": "line", ...}}
                                                            if (data && data.chart) {
                                                                const chartData = data.chart.data || [];
                                                                const xKey = data.chart.x_key || "name";
                                                                const series = data.chart.series || [];
                                                                
                                                                return (
                                                                    <div className="my-6 w-full h-[300px] border border-gray-200 dark:border-zinc-700 rounded-xl bg-white dark:bg-black/20 p-4">
                                                                        <h4 className="text-xs font-semibold mb-2 text-gray-500">{data.chart.title || "Data Visualization"}</h4>
                                                                        <ResponsiveContainer width="100%" height="100%">
                                                                            {data.chart.type === "bar" ? (
                                                                                <BarChart data={chartData}>
                                                                                    <CartesianGrid strokeDasharray="3 3" opacity={0.3} />
                                                                                    <XAxis dataKey={xKey} fontSize={10} />
                                                                                    <YAxis fontSize={10} />
                                                                                    <Tooltip contentStyle={{ borderRadius: '8px', fontSize: '12px' }} />
                                                                                    <Legend wrapperStyle={{ fontSize: '12px' }} />
                                                                                    {series.map((s: any, i: number) => (
                                                                                        <Bar key={i} dataKey={s.dataKey} fill={s.color || "#8884d8"} radius={[4, 4, 0, 0]} />
                                                                                    ))}
                                                                                </BarChart>
                                                                            ) : (
                                                                                <LineChart data={chartData}>
                                                                                    <CartesianGrid strokeDasharray="3 3" opacity={0.3} />
                                                                                    <XAxis dataKey={xKey} fontSize={10} />
                                                                                    <YAxis fontSize={10} />
                                                                                    <Tooltip contentStyle={{ borderRadius: '8px', fontSize: '12px' }} />
                                                                                    <Legend wrapperStyle={{ fontSize: '12px' }} />
                                                                                    {series.map((s: any, i: number) => (
                                                                                        <Line key={i} type="monotone" dataKey={s.dataKey} stroke={s.color || "#8884d8"} strokeWidth={2} dot={{ r: 3 }} activeDot={{ r: 5 }} />
                                                                                    ))}
                                                                                </LineChart>
                                                                            )}
                                                                        </ResponsiveContainer>
                                                                    </div>
                                                                );
                                                            }
                                                        } catch (e) {
                                                            // Fallback to normal code block if JSON parse fails or it's not a chart schema
                                                        }
                                                    }
                                                    
                                                    return !inline ? (
                                                        <div className="my-4 rounded-lg bg-[#1e1e1e] overflow-hidden border border-gray-700/50 shadow-md">
                                                            <div className="flex items-center justify-between px-4 py-1.5 bg-[#252526] border-b border-[#3e3e42]">
                                                                <span className="text-xs font-mono text-gray-400">{language || "Code"}</span>
                                                                <button
                                                                    onClick={() => navigator.clipboard.writeText(String(children))}
                                                                    className="text-gray-400 hover:text-white transition-colors"
                                                                >
                                                                    <Copy className="w-3.5 h-3.5" />
                                                                </button>
                                                            </div>
                                                            <pre className="p-4 overflow-x-auto text-[13px] leading-relaxed font-mono text-[#d4d4d4]">
                                                                <code className={className} {...props}>{children}</code>
                                                            </pre>
                                                        </div>
                                                    ) : (
                                                        <code className="bg-black/10 dark:bg-white/10 px-1.5 py-0.5 rounded text-[13px] font-mono text-pink-600 dark:text-pink-400" {...props}>
                                                            {children}
                                                        </code>
                                                    );
                                                }
                                            }}
                                        >
                                            {msg.content}
                                        </ReactMarkdown>
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
                                {msg.role === "assistant" && msg.reasoning && (
                                    <div className="mt-2 pt-2 border-t border-gray-200 dark:border-zinc-700 text-[11px] text-gray-500 italic">
                                        Note: {msg.reasoning}
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

                {/* Input — pl-14 avoids Next.js dev badge */}
                <div className="border-t pl-14 pr-6 py-4 bg-white dark:bg-zinc-950">
                    <div className="flex gap-3 items-end">
                        <div className="flex-1 relative">
                            <Input
                                value={chatInput}
                                onChange={e => setChatInput(e.target.value)}
                                onKeyDown={e => e.key === "Enter" && !e.shiftKey && handleSendMessage()}
                                placeholder="Type a message..."
                                className="pr-8"
                                disabled={chatSending}
                            />
                            {chatInput.length > 0 && (
                                <span className="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] text-gray-300">{chatInput.length}</span>
                            )}
                        </div>
                        <Button onClick={handleSendMessage} disabled={!chatInput.trim() || chatSending}
                            className="bg-gradient-to-r from-purple-600 to-pink-600 text-white h-10 w-10 p-0 rounded-xl shrink-0">
                            {chatSending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
                        </Button>
                    </div>
                </div>
            </div>

            {/* Right sidebar — Agent info */}
            <div className="w-80 border-l bg-gray-50/50 dark:bg-zinc-900/50 overflow-y-auto hidden lg:block">
                {/* Agent header */}
                <div className="px-5 py-4 border-b bg-white dark:bg-zinc-900">
                    <h3 className="font-semibold text-sm text-gray-700 dark:text-zinc-300 flex items-center gap-2"><Zap className="w-4 h-4 text-purple-500" /> Agent Config</h3>
                </div>
                <div className="p-4 space-y-3">
                    {/* Model & Provider */}
                    <div className="bg-white dark:bg-zinc-900 rounded-xl p-3.5 border border-gray-100 dark:border-zinc-800 space-y-2.5">
                        <div className="flex items-center justify-between">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Model</span>
                            <span className="text-xs font-mono bg-gray-50 dark:bg-zinc-800 px-2 py-0.5 rounded">{(selectedAgent?.model_id || '').split('/').pop()}</span>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Provider</span>
                            <span className="text-xs font-medium capitalize">{selectedAgent?.provider}</span>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Temp</span>
                            <span className="text-xs">{selectedAgent?.temperature ?? 0.7}</span>
                        </div>
                        <div className="flex items-center justify-between">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Max Tokens</span>
                            <span className="text-xs">{selectedAgent?.max_tokens ?? 2048}</span>
                        </div>
                    </div>

                    {/* Capabilities */}
                    <div className="bg-white dark:bg-zinc-900 rounded-xl p-3.5 border border-gray-100 dark:border-zinc-800 space-y-2">
                        <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Capabilities</span>
                        <div className="flex gap-2">
                            <span className={`text-[10px] font-medium px-2.5 py-1 rounded-full ${selectedAgent?.use_rag ? 'bg-blue-50 text-blue-600 dark:bg-blue-900/20' : 'bg-gray-100 text-gray-400 line-through'}`}>RAG</span>
                            <span className={`text-[10px] font-medium px-2.5 py-1 rounded-full ${selectedAgent?.use_knowledge_graph ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20' : 'bg-gray-100 text-gray-400 line-through'}`}>KG</span>
                        </div>
                    </div>

                    {/* Traits */}
                    {selectedAgent?.personality_traits && (selectedAgent.personality_traits as string[]).length > 0 && (
                        <div className="bg-white dark:bg-zinc-900 rounded-xl p-3.5 border border-gray-100 dark:border-zinc-800 space-y-2">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Traits</span>
                            <div className="flex flex-wrap gap-1.5">
                                {(selectedAgent.personality_traits as string[]).map(t => (
                                    <span key={t} className="text-[10px] font-medium bg-purple-50 dark:bg-purple-900/20 text-purple-600 dark:text-purple-400 px-2 py-0.5 rounded-full">{t}</span>
                                ))}
                            </div>
                        </div>
                    )}

                    {/* Tools */}
                    {selectedAgent?.tools && (selectedAgent.tools as string[]).length > 0 && (
                        <div className="bg-white dark:bg-zinc-900 rounded-xl p-3.5 border border-gray-100 dark:border-zinc-800 space-y-2">
                            <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Tools</span>
                            <div className="flex flex-wrap gap-1.5">
                                {(selectedAgent.tools as string[]).map(t => (
                                    <span key={t} className="text-[10px] font-medium bg-amber-50 dark:bg-amber-900/20 text-amber-600 dark:text-amber-400 px-2 py-0.5 rounded-full">{t}</span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>

                {/* Action buttons */}
                <div className="px-4 pb-4 space-y-2">
                    <Button size="sm" variant="outline" className="w-full" onClick={() => { if (selectedAgent) { loadAgentToForm(selectedAgent); setView("builder"); } }}>
                        <Edit className="w-3 h-3 mr-2" /> Edit Agent
                    </Button>
                    <Button size="sm" variant="outline" className="w-full border-rose-200 text-rose-700 hover:bg-rose-50 dark:border-rose-900 dark:text-rose-300 dark:hover:bg-rose-900/20"
                        onClick={() => { if (selectedAgent) openEvalModal(selectedAgent); }}>
                        <BarChart3 className="w-3 h-3 mr-2" /> Evaluate vs Benchmark
                    </Button>
                    <Button size="sm" variant="outline" className="w-full border-violet-200 text-violet-700 hover:bg-violet-50 dark:border-violet-900 dark:text-violet-300 dark:hover:bg-violet-900/20"
                        onClick={() => { if (selectedAgent) openTuneModal(selectedAgent); }}>
                        <Wand className="w-3 h-3 mr-2" /> Auto-Tune (Gemini Pro)
                    </Button>
                    {selectedAgent && !selectedAgent.is_published && (
                        <Button size="sm" className="w-full bg-green-600 hover:bg-green-700 text-white" onClick={() => handlePublish(selectedAgent.id)}>
                            <Rocket className="w-3 h-3 mr-2" /> Publish
                        </Button>
                    )}
                </div>

                {selectedAgent?.is_published && (
                    <details className="px-4 pb-4 mt-2 group">
                        <summary className="text-xs font-semibold text-slate-500 cursor-pointer hover:text-slate-700 list-none flex items-center gap-2">
                            <span className="w-4 h-4 flex items-center justify-center bg-slate-200 rounded-full text-[10px] group-open:rotate-90 transition-transform">▶</span>
                            Developer Integrations
                        </summary>
                        <div className="bg-slate-900 rounded-xl p-4 border border-slate-800 mt-2">
                            <div className="flex items-center gap-2 mb-2">
                                <Globe className="w-4 h-4 text-blue-400" />
                                <span className="text-xs font-semibold text-slate-200">API Integration</span>
                            </div>
                            <div className="space-y-3">
                                <div>
                                    <span className="text-[10px] text-slate-400 block mb-1">ENDPOINT</span>
                                    <code className="block text-[10px] font-mono text-pink-300 bg-slate-950 p-2 rounded border border-slate-800 break-all">
                                        POST http://bifrost.asgard.svc:8100/v1/agents/{selectedAgent.id}/run
                                    </code>
                                </div>
                                <div>
                                    <span className="text-[10px] text-slate-400 block mb-1">PAYLOAD</span>
                                    <pre className="text-[10px] font-mono text-emerald-300 bg-slate-950 p-2 rounded border border-slate-800 overflow-x-auto">
{`{
  "query": "Hello",
  "session_id": "optional-id"
}`}
                                    </pre>
                                </div>
                                <div className="text-[10px] text-slate-400">
                                    Headers: <code className="text-slate-300">X-Tenant-Id: {selectedAgent.tenant_id}</code><br/>
                                    Auth: <code className="text-slate-300">Bearer {selectedAgent.api_key}</code>
                                </div>
                            </div>
                        </div>
                    </details>
                )}
            </div>

            {/* ─── Eval Modal (detail view) ─────────────────────────────── */}
            {evalAgent && (
                <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={() => setEvalAgent(null)}>
                    <div className="bg-white dark:bg-zinc-900 rounded-xl shadow-2xl max-w-2xl w-full max-h-[90vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
                        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-zinc-800">
                            <div className="flex items-center gap-3">
                                <BarChart3 className="w-5 h-5 text-rose-600" />
                                <h2 className="font-semibold">Evaluate · {evalAgent.display_name || evalAgent.name}</h2>
                            </div>
                            <button onClick={() => setEvalAgent(null)} className="p-1 rounded hover:bg-gray-100 dark:hover:bg-zinc-800">
                                <X className="w-4 h-4" />
                            </button>
                        </div>

                        <div className="p-6 space-y-5">
                            <div>
                                <Label className="text-xs">Benchmark Dataset</Label>
                                {benchmarks.length === 0 ? (
                                    <p className="text-sm text-gray-500 mt-2">No benchmark datasets found in tenant <code className="text-xs bg-gray-100 dark:bg-zinc-800 px-1 py-0.5 rounded">{evalAgent.tenant_id}</code>. Run <code className="text-xs bg-gray-100 dark:bg-zinc-800 px-1 py-0.5 rounded">scripts/import_healthbench.py</code> to populate.</p>
                                ) : (
                                    <select value={evalBenchmarkId} onChange={e => setEvalBenchmarkId(e.target.value)} className="w-full mt-2 px-3 py-2 rounded-lg border border-gray-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm">
                                        {benchmarks.map(b => (
                                            <option key={b.id} value={b.id}>{b.name} · {b.total_items} items · {b.source}</option>
                                        ))}
                                    </select>
                                )}
                            </div>

                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <Label className="text-xs">Max items</Label>
                                    <Input type="number" value={evalMaxItems} onChange={e => setEvalMaxItems(Math.max(1, Math.min(525, parseInt(e.target.value) || 1)))} min={1} max={525} className="mt-2" />
                                </div>
                                <div>
                                    <Label className="text-xs">Est. duration</Label>
                                    <div className="mt-2 px-3 py-2 rounded-lg border border-gray-200 dark:border-zinc-700 bg-gray-50 dark:bg-zinc-800 text-xs text-gray-600 dark:text-zinc-400">
                                        ~{Math.round(evalMaxItems * 9)} sec ({Math.ceil(evalMaxItems * 9 / 60)} min)
                                    </div>
                                </div>
                            </div>

                            <div>
                                <Label className="text-xs">Run name <span className="text-gray-400">(editable)</span></Label>
                                <Input value={evalRunName} onChange={e => setEvalRunName(e.target.value)} className="mt-2 font-mono text-xs" placeholder="agent__model__benchmark__timestamp" />
                            </div>

                            <div>
                                <Label className="text-xs">Notes <span className="text-gray-400">(why this experiment? what changed?)</span></Label>
                                <textarea value={evalNotes} onChange={e => setEvalNotes(e.target.value)} rows={2}
                                    placeholder='e.g. "Switched from Gemini to MedGemma to test on-device medical model. Same prompt, same RAG."'
                                    className="mt-2 w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm" />
                            </div>

                            <details className="rounded-lg border border-gray-200 dark:border-zinc-700 p-3">
                                <summary className="text-xs text-gray-600 dark:text-zinc-400 cursor-pointer font-medium">📊 Experiment snapshot (will be saved with run)</summary>
                                <div className="mt-3 grid grid-cols-2 gap-2 text-[11px]">
                                    <div className="text-gray-500">Agent:</div><div className="font-mono">{evalAgent.name} #{evalAgent.id}</div>
                                    <div className="text-gray-500">Tenant:</div><div className="font-mono">{evalAgent.tenant_id}</div>
                                    <div className="text-gray-500">Model:</div><div className="font-mono">{evalAgent.model_id.split('/').pop()}</div>
                                    <div className="text-gray-500">Provider:</div><div className="font-mono">{evalAgent.provider}</div>
                                    <div className="text-gray-500">Temperature:</div><div className="font-mono">{evalAgent.temperature ?? 0.7}</div>
                                    <div className="text-gray-500">Max tokens:</div><div className="font-mono">{evalAgent.max_tokens ?? 2048}</div>
                                    <div className="text-gray-500">Top-K:</div><div className="font-mono">{evalAgent.top_k ?? 5}</div>
                                    <div className="text-gray-500">RAG:</div><div className="font-mono">{evalAgent.use_rag ? "on" : "off"}</div>
                                    <div className="text-gray-500">KG:</div><div className="font-mono">{evalAgent.use_knowledge_graph ? "on" : "off"}</div>
                                    <div className="text-gray-500">Tools:</div><div className="font-mono break-all">{Array.isArray(evalAgent.tools) ? (evalAgent.tools as string[]).join(', ') : "—"}</div>
                                </div>
                            </details>

                            <div className="rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 p-3 text-xs text-amber-800 dark:text-amber-200">
                                <p className="font-semibold mb-1">⚠️ CLI-only trigger (UI trigger coming soon)</p>
                                <p>Run from the Mimir host:</p>
                                <pre className="mt-2 bg-white/60 dark:bg-black/30 rounded p-2 text-[11px] overflow-x-auto whitespace-pre-wrap break-all">
{`AGENT_ID=${evalAgent.id} \\
AGENT_TENANT_ID=${evalAgent.tenant_id} \\
TENANT_ID=megacare \\
BENCHMARK_ID=${evalBenchmarkId} \\
MAX_ITEMS=${evalMaxItems} \\
GEMINI_API_KEY=$GEMINI_API_KEY \\
python3 scripts/run_healthbench_eval.py`}
                                </pre>
                            </div>

                            {recentRunsForAgent(evalAgent.name).length > 0 && (
                                <div>
                                    <Label className="text-xs">Recent runs (this agent)</Label>
                                    <div className="mt-2 space-y-1">
                                        {recentRunsForAgent(evalAgent.name).map(r => (
                                            <Link key={r.id} href={`/evaluations`} className="flex items-center justify-between text-xs px-3 py-2 rounded-lg bg-gray-50 dark:bg-zinc-800 hover:bg-gray-100 dark:hover:bg-zinc-700">
                                                <span className="truncate">{r.name || r.id.slice(0, 8)}</span>
                                                <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${r.status === "COMPLETED" ? "bg-green-100 text-green-700" : r.status === "RUNNING" ? "bg-blue-100 text-blue-700 animate-pulse" : "bg-gray-100 text-gray-700"}`}>
                                                    {r.completed_combinations}/{r.total_combinations} · {r.status}
                                                </span>
                                            </Link>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="px-6 py-4 border-t border-gray-200 dark:border-zinc-800 flex justify-end gap-2">
                            <Button variant="outline" onClick={() => setEvalAgent(null)}>Close</Button>
                            <Link href="/evaluations" className="inline-flex items-center gap-2 px-4 py-2 border border-gray-200 dark:border-zinc-700 rounded-lg text-sm hover:bg-gray-50 dark:hover:bg-zinc-800">
                                <ExternalLink className="w-4 h-4" /> View Results
                            </Link>
                            <Button onClick={handleStartEval} disabled={evalStarting || benchmarks.length === 0} className="bg-rose-600 hover:bg-rose-700 text-white">
                                {evalStarting ? <><Loader2 className="w-4 h-4 mr-2 animate-spin" /> Starting...</> : <><BarChart3 className="w-4 h-4 mr-2" /> Run Evaluation</>}
                            </Button>
                        </div>
                    </div>
                </div>
            )}

            {/* ─── Auto-Tune Modal ───────────────────────────────────────── */}
            {tuneAgent && (
                <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={() => !tuneLoading && setTuneAgent(null)}>
                    <div className="bg-white dark:bg-zinc-900 rounded-xl shadow-2xl max-w-3xl w-full max-h-[90vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
                        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-zinc-800">
                            <div className="flex items-center gap-3">
                                <Wand className="w-5 h-5 text-violet-600" />
                                <h2 className="font-semibold">Auto-Tune · {tuneAgent.display_name || tuneAgent.name}</h2>
                            </div>
                            <button onClick={() => !tuneLoading && setTuneAgent(null)} className="p-1 rounded hover:bg-gray-100 dark:hover:bg-zinc-800">
                                <X className="w-4 h-4" />
                            </button>
                        </div>

                        <div className="p-6 space-y-4">
                            {tuneLoading && (
                                <div className="text-center py-8">
                                    <Loader2 className="w-8 h-8 animate-spin mx-auto text-violet-600" />
                                    <p className="mt-3 text-sm text-gray-600 dark:text-zinc-400">Analyzing eval results · calling auto-tune model...</p>
                                    <p className="mt-1 text-xs text-gray-400">Typically 15-30 seconds</p>
                                </div>
                            )}
                            {tuneError && (
                                <div className="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-3 text-sm text-red-700 dark:text-red-300">
                                    ❌ {tuneError}
                                </div>
                            )}
                            {tuneResult && tuneResult.suggestions && (
                                <>
                                    <div className="text-xs text-gray-500 dark:text-zinc-400">
                                        Analyzed by <code className="font-mono bg-gray-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">{tuneResult.auto_tune_model}</code> · Run <code className="font-mono">{tuneResult.run_id?.slice(0, 8)}</code>
                                    </div>

                                    {tuneResult.current_metrics && (
                                        <div className="rounded-lg bg-gray-50 dark:bg-zinc-800/50 p-3 text-xs grid grid-cols-3 gap-2">
                                            <div><span className="text-gray-400">Accuracy:</span> <span className="font-mono">{tuneResult.current_metrics.avg_accuracy?.toFixed(2) ?? "—"}/5</span></div>
                                            <div><span className="text-gray-400">Completeness:</span> <span className="font-mono">{tuneResult.current_metrics.avg_completeness?.toFixed(2) ?? "—"}/5</span></div>
                                            <div><span className="text-gray-400">Relevance:</span> <span className="font-mono">{tuneResult.current_metrics.avg_relevance?.toFixed(2) ?? "—"}/5</span></div>
                                            <div><span className="text-gray-400">Safety:</span> <span className="font-mono">{tuneResult.current_metrics.avg_safety_score?.toFixed(2) ?? "—"}</span></div>
                                            <div><span className="text-gray-400">Unsafe:</span> <span className="font-mono">{tuneResult.current_metrics.unsafe_count ?? 0}</span></div>
                                            <div><span className="text-gray-400">Latency:</span> <span className="font-mono">{Math.round(tuneResult.current_metrics.avg_latency_ms ?? 0)}ms</span></div>
                                        </div>
                                    )}

                                    <div>
                                        <Label className="text-xs">Rationale</Label>
                                        <p className="mt-1 text-sm text-gray-700 dark:text-zinc-300 bg-violet-50 dark:bg-violet-900/10 p-3 rounded-lg">
                                            {tuneResult.rationale || tuneResult.suggestions.rationale || "—"}
                                        </p>
                                    </div>

                                    <div>
                                        <Label className="text-xs">Proposed Changes</Label>
                                        <div className="mt-2 space-y-2 text-xs">
                                            {tuneResult.suggestions.temperature != null && (
                                                <div className="flex items-center gap-2 px-3 py-2 rounded bg-gray-50 dark:bg-zinc-800/50">
                                                    <span className="text-gray-500 w-32">Temperature</span>
                                                    <span className="font-mono text-gray-400 line-through">{tuneAgent.temperature ?? 0.7}</span>
                                                    <span>→</span>
                                                    <span className="font-mono text-violet-600 font-semibold">{tuneResult.suggestions.temperature}</span>
                                                </div>
                                            )}
                                            {tuneResult.suggestions.top_k != null && (
                                                <div className="flex items-center gap-2 px-3 py-2 rounded bg-gray-50 dark:bg-zinc-800/50">
                                                    <span className="text-gray-500 w-32">Top-K</span>
                                                    <span className="font-mono text-gray-400 line-through">{tuneAgent.top_k ?? 5}</span>
                                                    <span>→</span>
                                                    <span className="font-mono text-violet-600 font-semibold">{tuneResult.suggestions.top_k}</span>
                                                </div>
                                            )}
                                            {tuneResult.suggestions.max_tokens != null && (
                                                <div className="flex items-center gap-2 px-3 py-2 rounded bg-gray-50 dark:bg-zinc-800/50">
                                                    <span className="text-gray-500 w-32">Max Tokens</span>
                                                    <span className="font-mono text-gray-400 line-through">{tuneAgent.max_tokens ?? 2048}</span>
                                                    <span>→</span>
                                                    <span className="font-mono text-violet-600 font-semibold">{tuneResult.suggestions.max_tokens}</span>
                                                </div>
                                            )}
                                            {tuneResult.suggestions.use_rag != null && tuneResult.suggestions.use_rag !== tuneAgent.use_rag && (
                                                <div className="flex items-center gap-2 px-3 py-2 rounded bg-gray-50 dark:bg-zinc-800/50">
                                                    <span className="text-gray-500 w-32">Use RAG</span>
                                                    <span className="font-mono text-gray-400 line-through">{String(tuneAgent.use_rag)}</span>
                                                    <span>→</span>
                                                    <span className="font-mono text-violet-600 font-semibold">{String(tuneResult.suggestions.use_rag)}</span>
                                                </div>
                                            )}
                                            {tuneResult.suggestions.use_knowledge_graph != null && tuneResult.suggestions.use_knowledge_graph !== tuneAgent.use_knowledge_graph && (
                                                <div className="flex items-center gap-2 px-3 py-2 rounded bg-gray-50 dark:bg-zinc-800/50">
                                                    <span className="text-gray-500 w-32">Use KG</span>
                                                    <span className="font-mono text-gray-400 line-through">{String(tuneAgent.use_knowledge_graph)}</span>
                                                    <span>→</span>
                                                    <span className="font-mono text-violet-600 font-semibold">{String(tuneResult.suggestions.use_knowledge_graph)}</span>
                                                </div>
                                            )}
                                            {(tuneResult.suggestions.add_tools && tuneResult.suggestions.add_tools.length > 0) && (
                                                <div className="px-3 py-2 rounded bg-green-50 dark:bg-green-900/20"><span className="text-gray-500">Add tools:</span> <code className="font-mono text-green-700 dark:text-green-400">{tuneResult.suggestions.add_tools.join(", ")}</code></div>
                                            )}
                                            {(tuneResult.suggestions.remove_tools && tuneResult.suggestions.remove_tools.length > 0) && (
                                                <div className="px-3 py-2 rounded bg-red-50 dark:bg-red-900/20"><span className="text-gray-500">Remove tools:</span> <code className="font-mono text-red-700 dark:text-red-400">{tuneResult.suggestions.remove_tools.join(", ")}</code></div>
                                            )}
                                        </div>
                                    </div>

                                    {tuneResult.suggestions.system_prompt && (
                                        <details className="rounded-lg border border-gray-200 dark:border-zinc-700">
                                            <summary className="px-3 py-2 cursor-pointer text-xs font-medium">📝 New System Prompt (preview)</summary>
                                            <pre className="px-3 py-2 text-[11px] whitespace-pre-wrap break-words bg-gray-50 dark:bg-zinc-800/50 max-h-60 overflow-y-auto">{tuneResult.suggestions.system_prompt}</pre>
                                        </details>
                                    )}

                                    {tuneResult.suggestions.expected_improvements && tuneResult.suggestions.expected_improvements.length > 0 && (
                                        <div className="text-xs">
                                            <Label>Expected Improvements</Label>
                                            <ul className="mt-1 list-disc list-inside text-gray-700 dark:text-zinc-300 space-y-0.5">
                                                {tuneResult.suggestions.expected_improvements.map((x, i) => (<li key={i}>{x}</li>))}
                                            </ul>
                                        </div>
                                    )}
                                </>
                            )}
                        </div>

                        <div className="px-6 py-4 border-t border-gray-200 dark:border-zinc-800 flex justify-end gap-2">
                            <Button variant="outline" onClick={() => setTuneAgent(null)} disabled={tuneLoading}>Close</Button>
                            {tuneResult?.suggestions && !tuneError && (
                                <Button onClick={applyTuneSuggestions} className="bg-violet-600 hover:bg-violet-700 text-white">
                                    <Save className="w-4 h-4 mr-2" /> Apply All Changes
                                </Button>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
