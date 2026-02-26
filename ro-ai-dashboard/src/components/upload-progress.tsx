"use client";

import React from "react";
import { CheckCircle2, Loader2, FileIcon } from "lucide-react";

export interface UploadFileStatus {
    name: string;
    progress: number;
    status: "pending" | "uploading" | "complete" | "error";
}

interface UploadProgressProps {
    files: UploadFileStatus[];
}

export function UploadProgress({ files }: UploadProgressProps) {
    if (files.length === 0) return null;

    return (
        <div className="space-y-2">
            <h4 className="text-sm font-medium text-muted-foreground">Upload Progress</h4>
            <div className="rounded-md border border-border divide-y divide-border">
                {files.map((file, i) => (
                    <div key={i} className="px-3 py-2">
                        <div className="flex items-center justify-between mb-1">
                            <div className="flex items-center gap-2 text-sm min-w-0">
                                {file.status === "complete" ? (
                                    <CheckCircle2 className="w-4 h-4 text-green-500 shrink-0" />
                                ) : file.status === "uploading" ? (
                                    <Loader2 className="w-4 h-4 text-blue-500 animate-spin shrink-0" />
                                ) : (
                                    <FileIcon className="w-4 h-4 text-muted-foreground shrink-0" />
                                )}
                                <span className="truncate">{file.name}</span>
                            </div>
                            <span className="text-xs font-mono text-muted-foreground ml-2 shrink-0">{file.progress}%</span>
                        </div>
                        <div
                            role="progressbar"
                            aria-valuenow={file.progress}
                            aria-valuemin={0}
                            aria-valuemax={100}
                            className="h-1.5 bg-muted rounded-full overflow-hidden"
                        >
                            <div
                                className={`h-full rounded-full transition-all duration-300 ${file.status === "complete"
                                        ? "bg-green-500"
                                        : file.status === "error"
                                            ? "bg-red-500"
                                            : "bg-blue-500"
                                    }`}
                                style={{ width: `${file.progress}%` }}
                            />
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
