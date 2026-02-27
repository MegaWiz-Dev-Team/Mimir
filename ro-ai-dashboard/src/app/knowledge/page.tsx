"use client";

import { BookOpen } from "lucide-react";

export default function KnowledgePage() {
    return (
        <div className="container mx-auto p-8">
            <div className="flex flex-col items-center justify-center min-h-[60vh] text-center">
                <div className="w-20 h-20 rounded-full bg-blue-50 dark:bg-blue-900/30 flex items-center justify-center mb-6">
                    <BookOpen className="w-10 h-10 text-blue-600 dark:text-blue-400" />
                </div>
                <h1 className="text-3xl font-bold mb-3 bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                    Knowledge Base
                </h1>
                <p className="text-gray-500 dark:text-zinc-400 text-lg mb-6 max-w-md">
                    Browse, search, and manage your knowledge graph. Explore connections between documents and entities.
                </p>
                <div className="inline-flex items-center px-4 py-2 rounded-full bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400 text-sm font-medium">
                    🚧 Coming in Sprint 10
                </div>
            </div>
        </div>
    );
}
