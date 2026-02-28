"use client";

import { useState } from "react";
import { Clock, ChevronDown } from "lucide-react";

export type ScheduleOption = "Manual" | "Every 15m" | "Hourly" | "Daily" | "Weekly";

const SCHEDULE_OPTIONS: { value: ScheduleOption; label: string; interval?: number }[] = [
    { value: "Manual", label: "Manual (no auto-refresh)" },
    { value: "Every 15m", label: "Every 15 minutes", interval: 900 },
    { value: "Hourly", label: "Every hour", interval: 3600 },
    { value: "Daily", label: "Every 24 hours", interval: 86400 },
    { value: "Weekly", label: "Every 7 days", interval: 604800 },
];

interface CronScheduleSelectorProps {
    value: ScheduleOption;
    onChange: (value: ScheduleOption) => void;
    disabled?: boolean;
}

export function CronScheduleSelector({ value, onChange, disabled }: CronScheduleSelectorProps) {
    const [open, setOpen] = useState(false);

    return (
        <div className="relative" data-testid="cron-schedule-selector">
            <button
                type="button"
                disabled={disabled}
                onClick={() => setOpen(!open)}
                className="flex items-center gap-2 px-3 py-2 text-sm border rounded-lg bg-white dark:bg-zinc-900 dark:border-zinc-700 hover:bg-gray-50 dark:hover:bg-zinc-800 transition-colors w-full disabled:opacity-50"
            >
                <Clock className="w-4 h-4 text-blue-500" />
                <span className="flex-1 text-left">{value}</span>
                <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${open ? "rotate-180" : ""}`} />
            </button>

            {open && (
                <div className="absolute z-50 mt-1 w-full bg-white dark:bg-zinc-900 border dark:border-zinc-700 rounded-lg shadow-lg py-1">
                    {SCHEDULE_OPTIONS.map((opt) => (
                        <button
                            key={opt.value}
                            type="button"
                            data-testid={`schedule-option-${opt.value}`}
                            className={`w-full text-left px-3 py-2 text-sm hover:bg-blue-50 dark:hover:bg-blue-950/30 transition-colors ${value === opt.value ? "bg-blue-50 text-blue-700 dark:bg-blue-950/30 dark:text-blue-400 font-medium" : ""
                                }`}
                            onClick={() => {
                                onChange(opt.value);
                                setOpen(false);
                            }}
                        >
                            <div>{opt.value}</div>
                            <div className="text-xs text-gray-400">{opt.label}</div>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

export { SCHEDULE_OPTIONS };
