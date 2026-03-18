"use client";

import { useState, useCallback } from "react";
import { Label } from "@/components/ui/label";

interface WeightSliderProps {
  weights: { vector: number; tree: number; graph: number };
  onChange: (weights: { vector: number; tree: number; graph: number }) => void;
  disabled?: boolean;
}

const SLIDER_CONFIG = [
  { key: "vector" as const, label: "Vector", icon: "🔷", color: "#3B82F6", trackClass: "bg-blue-500" },
  { key: "tree" as const, label: "Tree", icon: "🌿", color: "#22C55E", trackClass: "bg-green-500" },
  { key: "graph" as const, label: "Graph", icon: "🔮", color: "#A855F7", trackClass: "bg-purple-500" },
];

export function WeightSlider({ weights, onChange, disabled = false }: WeightSliderProps) {
  const handleChange = useCallback(
    (key: "vector" | "tree" | "graph", value: number) => {
      const newWeights = { ...weights, [key]: value / 100 };

      // Normalize so they sum to 1.0
      const otherKeys = SLIDER_CONFIG.filter((s) => s.key !== key).map((s) => s.key);
      const remaining = 1.0 - newWeights[key];
      const otherSum = otherKeys.reduce((sum, k) => sum + weights[k], 0);

      if (otherSum > 0) {
        otherKeys.forEach((k) => {
          newWeights[k] = (weights[k] / otherSum) * remaining;
        });
      } else {
        otherKeys.forEach((k, i) => {
          newWeights[k] = remaining / otherKeys.length;
        });
      }

      onChange(newWeights);
    },
    [weights, onChange]
  );

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <Label className="text-sm font-medium">Source Weights</Label>
        <span className="text-xs text-muted-foreground">
          Total: {((weights.vector + weights.tree + weights.graph) * 100).toFixed(0)}%
        </span>
      </div>

      {/* Weight bar visualization */}
      <div className="flex h-2 rounded-full overflow-hidden bg-muted">
        {SLIDER_CONFIG.map((config) => (
          <div
            key={config.key}
            className={`${config.trackClass} transition-all duration-200`}
            style={{ width: `${weights[config.key] * 100}%` }}
          />
        ))}
      </div>

      {/* Individual sliders */}
      {SLIDER_CONFIG.map((config) => (
        <div key={config.key} className="flex items-center gap-3">
          <span className="text-sm w-20 flex items-center gap-1">
            {config.icon} {config.label}
          </span>
          <input
            type="range"
            min={0}
            max={100}
            step={5}
            value={Math.round(weights[config.key] * 100)}
            onChange={(e) => handleChange(config.key, parseInt(e.target.value))}
            disabled={disabled}
            className="flex-1 h-1.5 rounded-lg appearance-none cursor-pointer accent-current"
            style={{ accentColor: config.color }}
          />
          <span className="text-sm font-mono w-12 text-right text-muted-foreground">
            {(weights[config.key] * 100).toFixed(0)}%
          </span>
        </div>
      ))}

      {/* Preset buttons */}
      <div className="flex gap-1.5 pt-1">
        <PresetButton
          label="Balanced"
          onClick={() => onChange({ vector: 0.34, tree: 0.33, graph: 0.33 })}
          disabled={disabled}
        />
        <PresetButton
          label="Vector Heavy"
          onClick={() => onChange({ vector: 0.7, tree: 0.15, graph: 0.15 })}
          disabled={disabled}
        />
        <PresetButton
          label="Graph Heavy"
          onClick={() => onChange({ vector: 0.15, tree: 0.15, graph: 0.7 })}
          disabled={disabled}
        />
      </div>
    </div>
  );
}

function PresetButton({
  label,
  onClick,
  disabled,
}: {
  label: string;
  onClick: () => void;
  disabled: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="text-[10px] px-2 py-1 rounded-md border border-border bg-muted/50 hover:bg-muted text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
    >
      {label}
    </button>
  );
}
