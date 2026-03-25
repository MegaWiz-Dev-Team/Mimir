"use client";

import { Plus, RefreshCw, MessageSquare } from "lucide-react";
import { Button } from "@/components/ui/button";
import Link from "next/link";

interface QuickActionsProps {
    onSyncAll: () => void;
    syncing: boolean;
}

export function QuickActions({ onSyncAll, syncing }: QuickActionsProps) {
    return (
        <div className="flex items-center gap-3 flex-wrap">
            <Button asChild>
                <Link href="/sources">
                    <Plus className="w-4 h-4 mr-2" />
                    Add Source
                </Link>
            </Button>
            <Button variant="outline" onClick={onSyncAll} disabled={syncing}>
                <RefreshCw className={`w-4 h-4 mr-2 ${syncing ? "animate-spin" : ""}`} />
                {syncing ? "Syncing..." : "Sync All Sources"}
            </Button>
            {/* <Button variant="outline" asChild>
                <Link href="/playground">
                    <MessageSquare className="w-4 h-4 mr-2" />
                    Open Playground
                </Link>
            </Button> */}
        </div>
    );
}
