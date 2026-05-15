import {
  BarChart3,
  Target,
  Brain,
  GitCompare,
  Database,
  Workflow,
  Beaker,
  Sparkles,
} from "lucide-react";

export type EvalTabId = "runs" | "matrix" | "ai-analysis" | "performance" | "extraction" | "retrieval" | "pipeline" | "ocr";

export interface EvalTab {
  id: EvalTabId;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  endpoint: string | null;
  badge?: string;
}

export interface EvalTabGroup {
  label: string;
  color: "zinc" | "blue" | "purple";
  tabs: EvalTab[];
}

export const EVAL_TAB_GROUPS: EvalTabGroup[] = [
  {
    label: "LLM Eval",
    color: "zinc",
    tabs: [
      { id: "runs", label: "Runs", icon: BarChart3, endpoint: null },
      { id: "matrix", label: "Run Detail", icon: Target, endpoint: null },
      { id: "ai-analysis", label: "AI Analysis", icon: Brain, endpoint: null },
      { id: "performance", label: "Compare", icon: GitCompare, endpoint: "feedback-summary" },
    ],
  },
  {
    label: "Pipeline",
    color: "blue",
    tabs: [
      { id: "extraction", label: "Extraction", icon: Database, endpoint: "extraction-summary" },
      { id: "retrieval", label: "Retrieval", icon: Workflow, endpoint: "retrieval-summary" },
      { id: "pipeline", label: "E2E Pipeline", icon: Beaker, endpoint: "pipeline-scorecard" },
    ],
  },
  {
    label: "Vision",
    color: "purple",
    tabs: [
      {
        id: "ocr",
        label: "OCR Benchmark",
        icon: Sparkles,
        endpoint: "ocr-summary",
        badge: "NEW",
      },
    ],
  },
];

export function getAllTabs(): EvalTab[] {
  return EVAL_TAB_GROUPS.flatMap((group) => group.tabs);
}

export function getTabById(id: EvalTabId): EvalTab | undefined {
  return getAllTabs().find((tab) => tab.id === id);
}

export function getGroupColor(color: string): string {
  const colors: Record<string, string> = {
    zinc: "bg-zinc-100 dark:bg-zinc-900",
    blue: "bg-blue-50 dark:bg-blue-950",
    purple: "bg-gradient-to-r from-purple-50 to-pink-50 dark:from-purple-950 dark:to-pink-950",
  };
  return colors[color] || colors.zinc;
}
