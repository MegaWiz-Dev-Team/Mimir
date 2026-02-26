"use client";

import React, { useCallback, useState } from "react";
import { useDropzone, FileRejection } from "react-dropzone";
import { Upload, AlertCircle, FileIcon, X } from "lucide-react";

const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50MB
const ACCEPTED_EXTENSIONS: Record<string, string[]> = {
    "application/pdf": [".pdf"],
    "text/csv": [".csv"],
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": [".xlsx"],
    "text/plain": [".txt"],
    "application/json": [".json"],
    "text/markdown": [".md"],
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document": [".docx"],
};

interface UploadDropzoneProps {
    onFilesAdded: (files: File[]) => void;
}

export function UploadDropzone({ onFilesAdded }: UploadDropzoneProps) {
    const [errors, setErrors] = useState<string[]>([]);

    const onDrop = useCallback(
        (acceptedFiles: File[], rejections: FileRejection[]) => {
            const newErrors: string[] = [];

            rejections.forEach((rejection) => {
                rejection.errors.forEach((error) => {
                    if (error.code === "file-too-large") {
                        newErrors.push(`File too large: ${rejection.file.name} exceeds 50MB limit`);
                    } else if (error.code === "file-invalid-type") {
                        newErrors.push(`Unsupported file type: ${rejection.file.name}`);
                    } else {
                        newErrors.push(`${rejection.file.name}: ${error.message}`);
                    }
                });
            });

            setErrors(newErrors);

            if (acceptedFiles.length > 0) {
                onFilesAdded(acceptedFiles);
            }
        },
        [onFilesAdded]
    );

    const { getRootProps, getInputProps, isDragActive } = useDropzone({
        onDrop,
        accept: ACCEPTED_EXTENSIONS,
        maxSize: MAX_FILE_SIZE,
    });

    return (
        <div className="space-y-3">
            <div
                {...getRootProps()}
                data-testid="upload-dropzone"
                className={`
                    flex flex-col items-center justify-center p-8 rounded-lg border-2 border-dashed cursor-pointer
                    transition-all duration-200
                    ${isDragActive
                        ? "border-primary bg-primary/5 scale-[1.02]"
                        : "border-border hover:border-primary/50 hover:bg-accent/30"
                    }
                `}
            >
                <input {...getInputProps()} />
                <Upload className={`w-10 h-10 mb-3 ${isDragActive ? "text-primary" : "text-muted-foreground"}`} />
                {isDragActive ? (
                    <p className="text-primary font-medium">Drop files here...</p>
                ) : (
                    <>
                        <p className="font-medium text-sm">Drag & drop files here</p>
                        <p className="text-xs text-muted-foreground mt-1">
                            or click to browse • Max 50MB • PDF, CSV, XLSX, TXT, JSON, MD, DOCX
                        </p>
                    </>
                )}
            </div>

            {errors.length > 0 && (
                <div className="space-y-1">
                    {errors.map((error, i) => (
                        <div key={i} className="flex items-center gap-2 text-sm text-red-500 bg-red-50 dark:bg-red-950/30 rounded-md p-2">
                            <AlertCircle className="w-4 h-4 shrink-0" />
                            <span>{error}</span>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
