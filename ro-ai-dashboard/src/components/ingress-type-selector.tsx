"use client";

import React from "react";
import { Globe, FileText, FileSpreadsheet, Plug } from "lucide-react";

export type IngressType = "web" | "document" | "tabular" | "mcp";

interface IngressOption {
    type: IngressType;
    icon: React.ReactNode;
    title: string;
    description: string;
}

const OPTIONS: IngressOption[] = [
    {
        type: "web",
        icon: <Globe className="w-8 h-8 text-blue-500" />,
        title: "Web Scraper",
        description: "Fetch and extract content from a URL",
    },
    {
        type: "document",
        icon: <FileText className="w-8 h-8 text-orange-500" />,
        title: "Document Upload",
        description: "Upload PDF, DOCX, or text files",
    },
    {
        type: "tabular",
        icon: <FileSpreadsheet className="w-8 h-8 text-green-500" />,
        title: "Tabular Data",
        description: "Upload CSV or Excel spreadsheets",
    },
    {
        type: "mcp",
        icon: <Plug className="w-8 h-8 text-purple-500" />,
        title: "MCP Connection",
        description: "Connect via Model Context Protocol",
    },
];

interface IngressTypeSelectorProps {
    onSelect: (type: IngressType) => void;
}

export function IngressTypeSelector({ onSelect }: IngressTypeSelectorProps) {
    return (
        <div className="grid grid-cols-2 gap-4">
            {OPTIONS.map((option) => (
                <button
                    key={option.type}
                    type="button"
                    onClick={() => onSelect(option.type)}
                    className="flex flex-col items-center gap-3 p-6 rounded-lg border-2 border-border bg-card hover:border-primary hover:bg-accent/50 transition-all duration-200 cursor-pointer text-center group"
                >
                    <div className="p-3 rounded-full bg-muted group-hover:bg-background transition-colors">
                        {option.icon}
                    </div>
                    <div>
                        <h3 className="font-semibold text-sm">{option.title}</h3>
                        <p className="text-xs text-muted-foreground mt-1">{option.description}</p>
                    </div>
                </button>
            ))}
        </div>
    );
}
