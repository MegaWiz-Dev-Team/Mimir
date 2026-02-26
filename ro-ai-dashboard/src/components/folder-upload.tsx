"use client";

import React, { useRef, useState } from "react";
import { FolderOpen, FileIcon } from "lucide-react";
import { Button } from "@/components/ui/button";

interface FolderUploadProps {
    onFilesSelected: (files: File[]) => void;
}

export function FolderUpload({ onFilesSelected }: FolderUploadProps) {
    const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
    const inputRef = useRef<HTMLInputElement>(null);

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const fileList = e.target.files;
        if (!fileList) return;

        const files = Array.from(fileList);
        setSelectedFiles(files);
        onFilesSelected(files);
    };

    return (
        <div className="space-y-3">
            <div className="flex flex-col items-center justify-center p-6 rounded-lg border-2 border-dashed border-border hover:border-primary/50 transition-colors">
                <FolderOpen className="w-10 h-10 mb-3 text-muted-foreground" />
                <p className="font-medium text-sm mb-2">Upload an entire folder</p>
                <p className="text-xs text-muted-foreground mb-3">All files in the folder will be recursively scanned</p>
                <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => inputRef.current?.click()}
                >
                    Select Folder
                </Button>
                <input
                    ref={inputRef}
                    type="file"
                    data-testid="folder-input"
                    className="hidden"
                    onChange={handleChange}
                    {...({ webkitdirectory: "", directory: "" } as any)}
                    multiple
                />
            </div>

            {selectedFiles.length > 0 && (
                <div className="rounded-md border border-border divide-y divide-border">
                    {selectedFiles.map((file, i) => (
                        <div key={i} className="flex items-center gap-2 px-3 py-2 text-sm">
                            <FileIcon className="w-4 h-4 text-muted-foreground shrink-0" />
                            <span className="truncate">{file.name}</span>
                            <span className="text-xs text-muted-foreground ml-auto shrink-0">
                                {(file.size / 1024).toFixed(1)} KB
                            </span>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
