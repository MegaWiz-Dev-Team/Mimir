import React from "react";
import { Brain, Database, LayoutGrid, Link as LinkIcon, Cpu, Zap, Activity } from "lucide-react";
import { AgentConfigResponse } from "@/lib/api";

interface AgentStructurePanelProps {
    agent: AgentConfigResponse | null;
}

export function AgentStructurePanel({ agent }: AgentStructurePanelProps) {
    if (!agent) {
        return (
            <div className="w-64 border-r border-gray-100 dark:border-zinc-800 bg-gray-50/50 dark:bg-zinc-900/50 flex flex-col flex-shrink-0 animate-pulse p-6">
                <div className="h-4 bg-gray-200 dark:bg-zinc-700 rounded w-24 mb-4" />
                <div className="h-32 bg-gray-200 dark:bg-zinc-700 rounded w-full" />
            </div>
        );
    }

    return (
        <div className="w-72 border-r border-gray-100 dark:border-zinc-800 bg-gray-50/30 dark:bg-zinc-900/30 flex flex-col flex-shrink-0 overflow-y-auto overflow-x-hidden">
            <div className="p-5 border-b border-gray-100 dark:border-zinc-800">
                <div className="flex items-center gap-3 mb-1">
                    <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white shadow-sm flex-shrink-0">
                        <Activity className="w-4 h-4" />
                    </div>
                    <div>
                        <h2 className="text-sm font-semibold truncate max-w-[180px]">{agent.display_name || agent.name}</h2>
                        <p className="text-[11px] text-gray-500 truncate mt-0.5 font-mono">{agent.id}</p>
                    </div>
                </div>
            </div>

            <div className="p-4 space-y-6">
                {/* section: Brain */}
                <div className="space-y-3">
                    <div className="flex items-center gap-2 text-indigo-600 dark:text-indigo-400">
                        <Brain className="w-4 h-4" />
                        <h3 className="text-xs font-bold uppercase tracking-wider">Core Brain</h3>
                    </div>
                    <div className="pl-6 space-y-2 relative before:absolute before:left-[11px] before:top-[-10px] before:bottom-2 before:w-[2px] before:bg-indigo-100 dark:before:bg-indigo-900/30">
                        <div className="relative group">
                            <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-indigo-100 dark:bg-indigo-900/30" />
                            <div className="bg-white dark:bg-zinc-900 border border-indigo-100 dark:border-indigo-900/50 p-2 rounded-lg shadow-sm">
                                <p className="text-xs font-medium text-gray-700 dark:text-gray-200">Model</p>
                                <p className="text-[11px] text-indigo-500 font-mono mt-0.5">{agent.model_id}</p>
                            </div>
                        </div>
                        <div className="relative group">
                            <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-indigo-100 dark:bg-indigo-900/30" />
                            <div className="bg-white dark:bg-zinc-900 border border-indigo-100 dark:border-indigo-900/50 p-2 rounded-lg shadow-sm">
                                <p className="text-xs font-medium text-gray-700 dark:text-gray-200">Provider</p>
                                <p className="text-[11px] text-gray-500 mt-0.5">{agent.provider}</p>
                            </div>
                        </div>
                    </div>
                </div>

                {/* section: Knowledge Base */}
                {(agent.use_rag || agent.use_knowledge_graph || (agent as any).use_pageindex) && (
                    <div className="space-y-3">
                        <div className="flex items-center gap-2 text-emerald-600 dark:text-emerald-400">
                            <Database className="w-4 h-4" />
                            <h3 className="text-xs font-bold uppercase tracking-wider">Knowledge Source</h3>
                        </div>
                        <div className="pl-6 space-y-2 relative before:absolute before:left-[11px] before:top-[-10px] before:bottom-2 before:w-[2px] before:bg-emerald-100 dark:before:bg-emerald-900/30">
                            {agent.use_rag && (
                                <div className="relative group">
                                    <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-emerald-100 dark:bg-emerald-900/30" />
                                    <div className="bg-white dark:bg-zinc-900 border border-emerald-100 dark:border-emerald-900/50 p-2 rounded-lg shadow-sm flex items-center justify-between">
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-200">Vector Engine</span>
                                        <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                                    </div>
                                </div>
                            )}
                            {agent.use_knowledge_graph && (
                                <div className="relative group">
                                    <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-emerald-100 dark:bg-emerald-900/30" />
                                    <div className="bg-white dark:bg-zinc-900 border border-emerald-100 dark:border-emerald-900/50 p-2 rounded-lg shadow-sm flex items-center justify-between">
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-200">Neo4j Knowledge Graph</span>
                                        <div className="w-2 h-2 rounded-full bg-purple-500" />
                                    </div>
                                </div>
                            )}
                            {(agent as any).use_pageindex && (
                                <div className="relative group">
                                    <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-emerald-100 dark:bg-emerald-900/30" />
                                    <div className="bg-white dark:bg-zinc-900 border border-emerald-100 dark:border-emerald-900/50 p-2 rounded-lg shadow-sm flex items-center justify-between">
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-200">Page Tree Index</span>
                                        <LayoutGrid className="w-3 h-3 text-emerald-500" />
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {/* section: Abilities & MCP */}
                <div className="space-y-3">
                    <div className="flex items-center gap-2 text-fuchsia-600 dark:text-fuchsia-400">
                        <Zap className="w-4 h-4" />
                        <h3 className="text-xs font-bold uppercase tracking-wider">Abilities</h3>
                    </div>
                    <div className="pl-6 space-y-2 relative before:absolute before:left-[11px] before:top-[-10px] before:bottom-2 before:w-[2px] before:bg-fuchsia-100 dark:before:bg-fuchsia-900/30">
                        {(!agent.tools || agent.tools.length === 0) && (!agent.mcp_servers || agent.mcp_servers.length === 0) && (
                             <p className="text-[11px] text-gray-400 italic">No external abilities configured.</p>
                        )}
                        {(agent.tools || []).map((tool, idx) => (
                             <div key={idx} className="relative group">
                                 <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-fuchsia-100 dark:bg-fuchsia-900/30" />
                                 <div className="bg-white dark:bg-zinc-900 border border-fuchsia-100 dark:border-fuchsia-900/50 p-1.5 px-2 rounded-lg shadow-sm flex items-center gap-2">
                                     <Cpu className="w-3 h-3 text-fuchsia-500" />
                                     <span className="text-xs font-mono text-gray-700 dark:text-gray-200 truncate">{tool}</span>
                                 </div>
                             </div>
                        ))}
                        {(agent.mcp_servers || []).map((mcpUrl, idx) => {
                             try {
                                const url = new URL(mcpUrl);
                                const displayUrl = url.host + url.pathname;
                                return (
                                    <div key={`mcp-${idx}`} className="relative group">
                                        <span className="absolute left-[-20px] top-1/2 -mt-px w-5 h-[2px] bg-fuchsia-100 dark:bg-fuchsia-900/30" />
                                        <div className="bg-gradient-to-r from-fuchsia-50 to-orange-50 dark:from-fuchsia-900/20 dark:to-orange-900/20 border border-fuchsia-200 dark:border-fuchsia-800 p-2 rounded-lg shadow-sm">
                                            <div className="flex items-center gap-2 mb-1">
                                                <LinkIcon className="w-3 h-3 text-orange-500" />
                                                <span className="text-[10px] font-bold text-orange-600 dark:text-orange-400 uppercase tracking-wider">MCP Bridge</span>
                                            </div>
                                            <p className="text-[11px] font-mono text-gray-600 dark:text-gray-300 truncate" title={mcpUrl}>{displayUrl}</p>
                                        </div>
                                    </div>
                                );
                             } catch {
                                return null;
                             }
                        })}
                    </div>
                </div>

                {/* section: Traits */}
                {agent.personality_traits && agent.personality_traits.length > 0 && (
                    <div className="space-y-3">
                        <div className="flex items-center gap-2 text-rose-500 dark:text-rose-400">
                            <Activity className="w-4 h-4" />
                            <h3 className="text-xs font-bold uppercase tracking-wider">Persona</h3>
                        </div>
                        <div className="flex flex-wrap gap-2 pl-2">
                            {agent.personality_traits.map((trait, idx) => (
                                <span key={idx} className="px-2 py-1 bg-rose-50 dark:bg-rose-900/20 text-rose-600 dark:text-rose-400 rounded-full text-[10px] font-medium border border-rose-100 dark:border-rose-900/50 shadow-sm">
                                    {trait}
                                </span>
                            ))}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
