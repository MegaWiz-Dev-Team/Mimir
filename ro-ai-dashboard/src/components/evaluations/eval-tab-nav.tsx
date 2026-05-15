"use client";

import { EVAL_TAB_GROUPS, EvalTabId, getGroupColor } from "./eval-tab-registry";

interface EvalTabNavProps {
  activeTab: EvalTabId;
  onChange: (tabId: EvalTabId) => void;
}

export function EvalTabNav({ activeTab, onChange }: EvalTabNavProps) {
  return (
    <div className="space-y-4">
      {EVAL_TAB_GROUPS.map((group) => (
        <div key={group.label}>
          <h3 className="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider mb-2 px-2">
            {group.label}
          </h3>
          <div className={`${getGroupColor(group.color)} border rounded-lg p-2 flex flex-wrap gap-2`}>
            {group.tabs.map((tab) => {
              const Icon = tab.icon;
              const isActive = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  onClick={() => onChange(tab.id)}
                  className={`flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-all ${
                    isActive
                      ? "bg-white dark:bg-zinc-800 shadow-md border border-gray-300 dark:border-gray-600 text-gray-900 dark:text-gray-100"
                      : "text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-300 hover:bg-white/50 dark:hover:bg-white/10"
                  }`}
                >
                  <Icon className="w-4 h-4" />
                  <span>{tab.label}</span>
                  {tab.badge && (
                    <span className="ml-1 px-2 py-0.5 text-xs font-bold bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-full">
                      {tab.badge}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
