"use client";

import { Badge } from "@/components/ui/badge";

interface SourceBadgeProps {
  sourceType: "vector" | "tree" | "graph" | string;
  score?: number;
  className?: string;
}

const SOURCE_CONFIG: Record<string, { label: string; color: string; icon: string; bgClass: string; textClass: string; borderClass: string }> = {
  vector: {
    label: "Vector",
    color: "#3B82F6",
    icon: "🔷",
    bgClass: "bg-blue-500/10",
    textClass: "text-blue-400",
    borderClass: "border-blue-500/30",
  },
  graph: {
    label: "Graph",
    color: "#A855F7",
    icon: "🔮",
    bgClass: "bg-purple-500/10",
    textClass: "text-purple-400",
    borderClass: "border-purple-500/30",
  },
  tree: {
    label: "Tree",
    color: "#22C55E",
    icon: "🌿",
    bgClass: "bg-green-500/10",
    textClass: "text-green-400",
    borderClass: "border-green-500/30",
  },
};

const DEFAULT_CONFIG = {
  label: "Unknown",
  color: "#6B7280",
  icon: "❓",
  bgClass: "bg-gray-500/10",
  textClass: "text-gray-400",
  borderClass: "border-gray-500/30",
};

export function SourceBadge({ sourceType, score, className = "" }: SourceBadgeProps) {
  const config = SOURCE_CONFIG[sourceType] || DEFAULT_CONFIG;

  return (
    <Badge
      variant="outline"
      className={`text-xs font-medium px-2 py-0.5 ${config.bgClass} ${config.textClass} ${config.borderClass} ${className}`}
    >
      <span className="mr-1">{config.icon}</span>
      {config.label}
      {score !== undefined && (
        <span className="ml-1 opacity-70">
          {(score * 100).toFixed(0)}%
        </span>
      )}
    </Badge>
  );
}

export function SourceBadgeGroup({ sources }: { sources: { source_type: string; score?: number }[] }) {
  if (!sources || sources.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1">
      {sources.map((s, i) => (
        <SourceBadge key={i} sourceType={s.source_type} score={s.score} />
      ))}
    </div>
  );
}

export function SourceLegend() {
  return (
    <div className="flex gap-3 text-xs text-muted-foreground">
      {Object.entries(SOURCE_CONFIG).map(([key, config]) => (
        <div key={key} className="flex items-center gap-1">
          <span>{config.icon}</span>
          <span className={config.textClass}>{config.label}</span>
        </div>
      ))}
    </div>
  );
}
