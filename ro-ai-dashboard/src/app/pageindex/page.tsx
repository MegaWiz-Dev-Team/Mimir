"use client";

import { useState, useEffect } from "react";
import { fetchSources, DataSource, generatePageIndexTree } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { FileText, Layers, ListTree, Search, ChevronRight, ChevronDown, Hash, ArrowRight, RefreshCw, Loader2 } from "lucide-react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

interface PageIndexNode {
    node_id: string;
    title: string;
    summary: string;
    start_index: number;
    end_index: number;
    nodes?: PageIndexNode[];
}

// Tree view node component
function TreeNode({ node, defaultOpen = false }: { node: PageIndexNode; defaultOpen?: boolean }) {
    const [isOpen, setIsOpen] = useState(defaultOpen);
    const hasChildren = node.nodes && node.nodes.length > 0;

    return (
        <div className="w-full">
            <div 
                className={`flex items-start gap-2 p-2 rounded-md hover:bg-muted/50 transition-colors ${hasChildren ? "cursor-pointer" : ""}`}
                onClick={() => hasChildren && setIsOpen(!isOpen)}
            >
                {/* Node icon / expander */}
                <div className="mt-0.5 shrink-0 text-muted-foreground hover:text-foreground">
                    {hasChildren ? (
                        isOpen ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />
                    ) : (
                        <Hash className="w-4 h-4 text-blue-500/50" />
                    )}
                </div>

                {/* Node details */}
                <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                        <span className="font-medium text-sm text-foreground">{node.title}</span>
                        <span className="text-xs bg-muted px-1.5 py-0.5 rounded text-muted-foreground font-mono">
                            Chunks {node.start_index} - {node.end_index}
                        </span>
                    </div>
                    {node.summary && (
                        <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                            {node.summary}
                        </p>
                    )}
                </div>
            </div>

            {/* Render children if open */}
            {hasChildren && isOpen && (
                <div className="ml-5 pl-2 border-l border-border mt-1 space-y-1">
                    {node.nodes!.map((child) => (
                        <TreeNode key={child.node_id} node={child} defaultOpen={false} />
                    ))}
                </div>
            )}
        </div>
    );
}

export default function PageIndexViewer() {
    const [sources, setSources] = useState<DataSource[]>([]);
    const [selectedSourceId, setSelectedSourceId] = useState<number | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [searchQuery, setSearchQuery] = useState("");
    const [regenerating, setRegenerating] = useState(false);

    // Detect dark mode automatically via tailwind / layout scope
    useEffect(() => {
        loadData();
    }, []);

    const loadData = async () => {
        try {
            const data = await fetchSources();
            // Filter sources that might have page index
            setSources(data);
            
            // Auto-select first source with a valid tree
            const withTree = data.find(s => s.pageindex_tree);
            if (withTree) {
                setSelectedSourceId(withTree.id);
            }
        } catch (error) {
            console.error("Failed to fetch sources", error);
        } finally {
            setIsLoading(false);
        }
    };

    // Derived states
    const filteredSources = sources.filter(s => 
        s.name.toLowerCase().includes(searchQuery.toLowerCase()) || 
        (s.source_type && s.source_type.toLowerCase().includes(searchQuery.toLowerCase()))
    );

    const selectedSource = sources.find(s => s.id === selectedSourceId);

    // Parse the tree
    let rootNode: PageIndexNode | null = null;
    if (selectedSource?.pageindex_tree) {
        try {
            rootNode = typeof selectedSource.pageindex_tree === 'string' 
                ? JSON.parse(selectedSource.pageindex_tree) 
                : selectedSource.pageindex_tree;
        } catch (e) {
            console.error("Failed to parse page index tree", e);
        }
    }

    return (
        <div className="container mx-auto p-6 lg:p-8 space-y-6 max-h-screen flex flex-col">
            {/* Header */}
            <div className="shrink-0 flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold flex items-center gap-2">
                        <ListTree className="w-6 h-6 text-emerald-600" />
                        Page Index
                    </h1>
                    <p className="text-muted-foreground text-sm mt-1">
                        View hierarchical, semantic tree structures extracted from your knowledge sources.
                    </p>
                </div>
                
                <Link href="/sources">
                    <Button variant="outline" size="sm" className="gap-2">
                        <ArrowRight className="w-4 h-4" /> Go to Sources
                    </Button>
                </Link>
            </div>

            {/* Split View */}
            <div className="flex-1 flex flex-col lg:flex-row gap-6 min-h-0">
                {/* Left Panel: Source List */}
                <Card className="w-full lg:w-1/3 flex flex-col shrink-0 overflow-hidden">
                    <CardHeader className="py-4 border-b bg-muted/20 shrink-0">
                        <CardTitle className="text-sm font-medium flex items-center gap-2">
                            <Layers className="w-4 h-4 text-blue-600" /> All Sources
                        </CardTitle>
                        <div className="relative mt-2">
                            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                            <input 
                                type="text" 
                                placeholder="Filter sources..." 
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                className="w-full bg-background border rounded-md pl-9 pr-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                            />
                        </div>
                    </CardHeader>
                    
                    <div className="flex-1 overflow-y-auto p-2 space-y-1">
                        {isLoading ? (
                            <div className="p-4 text-center text-sm text-muted-foreground animate-pulse">
                                Loading sources...
                            </div>
                        ) : filteredSources.length === 0 ? (
                            <div className="p-4 text-center text-sm text-muted-foreground">
                                No sources found.
                            </div>
                        ) : (
                            filteredSources.map((source) => {
                                const isSelected = source.id === selectedSourceId;
                                const hasTree = !!source.pageindex_tree;
                                
                                return (
                                    <button
                                        key={source.id}
                                        onClick={() => setSelectedSourceId(source.id)}
                                        className={`w-full text-left flex items-center gap-3 p-3 rounded-md transition-colors ${
                                            isSelected 
                                                ? "bg-emerald-50 text-emerald-900 border border-emerald-200 dark:bg-emerald-900/20 dark:text-emerald-300 dark:border-emerald-800" 
                                                : "hover:bg-muted"
                                        }`}
                                    >
                                        <div className={`shrink-0 w-8 h-8 rounded-full flex items-center justify-center ${
                                            hasTree 
                                                ? "bg-emerald-100 text-emerald-600 dark:bg-emerald-900/50 dark:text-emerald-400" 
                                                : "bg-gray-100 text-gray-400 dark:bg-gray-800 dark:text-gray-500"
                                            }`}
                                        >
                                            <FileText className="w-4 h-4" />
                                        </div>
                                        <div className="flex-1 min-w-0">
                                            <p className="text-sm font-medium truncate">{source.name}</p>
                                            <div className="flex items-center gap-2 mt-0.5">
                                                <span className="text-xs text-muted-foreground truncate max-w-[120px]">
                                                    {source.source_type}
                                                </span>
                                                {hasTree ? (
                                                    <span className="shrink-0 inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/50 dark:text-emerald-400">
                                                        Indexed
                                                    </span>
                                                ) : (
                                                    <span className="shrink-0 inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400">
                                                        No Index
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                    </button>
                                );
                            })
                        )}
                    </div>
                </Card>

                {/* Right Panel: Tree View */}
                <Card className="flex-1 flex flex-col overflow-hidden">
                    <CardHeader className="py-4 border-b bg-muted/20 shrink-0">
                        <div className="flex items-center justify-between">
                            <div>
                                <CardTitle className="text-sm font-medium flex items-center gap-2">
                                    <ListTree className="w-4 h-4 text-emerald-600" /> Semantic Tree
                                </CardTitle>
                                {selectedSource && (
                                    <CardDescription className="mt-1 truncate max-w-md">
                                        Showing hierarchy for: <strong>{selectedSource.name}</strong>
                                    </CardDescription>
                                )}
                            </div>
                            
                            {!rootNode && selectedSource && (
                                <Link href="/sources">
                                    <Button size="sm" variant="secondary" className="text-xs">
                                        Extract Page Index
                                    </Button>
                                </Link>
                            )}
                            {selectedSource && (
                                <Button
                                    size="sm"
                                    variant={rootNode ? "outline" : "default"}
                                    className="text-xs"
                                    disabled={regenerating}
                                    onClick={async () => {
                                        if (!selectedSourceId) return;
                                        setRegenerating(true);
                                        try {
                                            await generatePageIndexTree(selectedSourceId);
                                            // Wait a bit then reload
                                            setTimeout(async () => {
                                                await loadData();
                                                setRegenerating(false);
                                            }, 3000);
                                        } catch (error) {
                                            console.error("Failed to regenerate tree", error);
                                            setRegenerating(false);
                                        }
                                    }}
                                >
                                    {regenerating ? (
                                        <><Loader2 className="w-3 h-3 mr-1 animate-spin" /> Generating...</>
                                    ) : (
                                        <><RefreshCw className="w-3 h-3 mr-1" /> {rootNode ? 'Re-generate Tree' : 'Generate Tree'}</>
                                    )}
                                </Button>
                            )}
                        </div>
                    </CardHeader>
                    
                    <div className="flex-1 overflow-y-auto p-4 lg:p-6 bg-slate-50/50 dark:bg-zinc-950/50">
                        {!selectedSourceId ? (
                            <div className="h-full flex flex-col items-center justify-center text-muted-foreground">
                                <ListTree className="w-12 h-12 mb-4 opacity-20" />
                                <p>Select a source to view its Page Index</p>
                            </div>
                        ) : !rootNode ? (
                            <div className="h-full flex flex-col items-center justify-center text-muted-foreground space-y-4">
                                <div className="w-16 h-16 rounded-full bg-muted flex items-center justify-center">
                                    <ListTree className="w-8 h-8 opacity-50" />
                                </div>
                                <div className="text-center">
                                    <h3 className="font-medium text-foreground">No Tree Generated</h3>
                                    <p className="text-sm mt-1 max-w-sm">
                                        This source does not have a semantic tree yet. Go to Data Sources, open the settings for this source, and hit 'Run Full Auto-Pipeline' with PageIndex enabled.
                                    </p>
                                </div>
                            </div>
                        ) : (
                            <div className="bg-white dark:bg-zinc-900 border rounded-lg p-2 lg:p-4 shadow-sm">
                                <TreeNode node={rootNode} defaultOpen={true} />
                            </div>
                        )}
                    </div>
                </Card>
            </div>
        </div>
    );
}
