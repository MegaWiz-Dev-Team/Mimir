"use client";

import { BarChart3 } from "lucide-react";

export default function CoveragePage() {
    return (
        <div className="container mx-auto p-8">
            <div className="flex flex-col items-center justify-center min-h-[60vh] text-center">
                <div className="w-20 h-20 rounded-full bg-emerald-50 dark:bg-emerald-900/30 flex items-center justify-center mb-6">
                    <BarChart3 className="w-10 h-10 text-emerald-600 dark:text-emerald-400" />
                </div>
                <h1 className="text-3xl font-bold mb-3 bg-gradient-to-r from-emerald-600 to-teal-600 bg-clip-text text-transparent">
                    Coverage Analytics
                </h1>
                <p className="text-gray-500 dark:text-zinc-400 text-lg mb-6 max-w-md">
                    Track knowledge coverage, identify gaps, and monitor data quality metrics across all your sources.
                </p>
                <div className="inline-flex items-center px-4 py-2 rounded-full bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400 text-sm font-medium">
                    🚧 Coming in Sprint 12
                </div>
            </div>
        </div>
    );
}
