"use client";

import React from "react";

export type StorageMode = "markdown" | "sql";

interface StorageModeSelectorProps {
    value: StorageMode;
    onChange: (mode: StorageMode) => void;
}

export function StorageModeSelector({ value, onChange }: StorageModeSelectorProps) {
    return (
        <fieldset className="space-y-3">
            <legend className="text-sm font-medium mb-2">Storage Mode</legend>
            <div className="space-y-2">
                <label className="flex items-center gap-3 p-3 rounded-md border border-border hover:bg-accent/30 cursor-pointer transition-colors">
                    <input
                        type="radio"
                        name="storage-mode"
                        value="markdown"
                        checked={value === "markdown"}
                        onChange={() => onChange("markdown")}
                        className="accent-primary"
                    />
                    <div>
                        <span className="text-sm font-medium">Markdown</span>
                        <p className="text-xs text-muted-foreground">Convert tabular data to Markdown table format (default)</p>
                    </div>
                </label>
                <label className="flex items-center gap-3 p-3 rounded-md border border-border hover:bg-accent/30 cursor-pointer transition-colors">
                    <input
                        type="radio"
                        name="storage-mode"
                        value="sql"
                        checked={value === "sql"}
                        onChange={() => onChange("sql")}
                        className="accent-primary"
                    />
                    <div>
                        <span className="text-sm font-medium">SQL Table</span>
                        <p className="text-xs text-muted-foreground">Create a dynamic SQL table with auto-detected column types</p>
                    </div>
                </label>
            </div>
        </fieldset>
    );
}
