"use client";

import React from "react";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { StorageModeSelector, StorageMode } from "./storage-mode-selector";
import { IngressType } from "./ingress-type-selector";

export interface AdvancedSettingsData {
    ocrEnabled: boolean;
    useHeaderRow: boolean;
    storageMode: StorageMode;
}

interface AdvancedSettingsProps {
    ingressType: IngressType;
    domain?: string;
    settings: AdvancedSettingsData;
    onSettingsChange: (settings: AdvancedSettingsData) => void;
}

export function AdvancedSettings({ ingressType, domain, settings, onSettingsChange }: AdvancedSettingsProps) {
    const showOcr = domain === "medical";
    const showTabularSettings = ingressType === "file";
    const showPdfSettings = ingressType === "file";

    return (
        <Accordion type="single" collapsible defaultValue="advanced">
            <AccordionItem value="advanced">
                <AccordionTrigger className="text-sm font-medium">
                    Advanced Settings
                </AccordionTrigger>
                <AccordionContent>
                    <div className="space-y-4 pt-2">
                        {/* PDF settings */}
                        {showPdfSettings && (
                            <div className="space-y-3">
                                {showOcr && (
                                    <label className="flex items-center justify-between p-3 rounded-md border border-border">
                                        <div>
                                            <span className="text-sm font-medium">Text & OCR</span>
                                            <p className="text-xs text-muted-foreground">
                                                Enable optical character recognition for scanned PDFs
                                            </p>
                                        </div>
                                        <input
                                            type="checkbox"
                                            checked={settings.ocrEnabled}
                                            onChange={(e) =>
                                                onSettingsChange({ ...settings, ocrEnabled: e.target.checked })
                                            }
                                            className="accent-primary w-4 h-4"
                                        />
                                    </label>
                                )}
                            </div>
                        )}

                        {/* Tabular settings */}
                        {showTabularSettings && (
                            <div className="space-y-3">
                                <label className="flex items-center justify-between p-3 rounded-md border border-border">
                                    <div>
                                        <span className="text-sm font-medium">Use Row 1 as Header</span>
                                        <p className="text-xs text-muted-foreground">
                                            Treat the first row of the file as column headers
                                        </p>
                                    </div>
                                    <input
                                        type="checkbox"
                                        checked={settings.useHeaderRow}
                                        onChange={(e) =>
                                            onSettingsChange({ ...settings, useHeaderRow: e.target.checked })
                                        }
                                        className="accent-primary w-4 h-4"
                                    />
                                </label>
                                <StorageModeSelector
                                    value={settings.storageMode}
                                    onChange={(mode) =>
                                        onSettingsChange({ ...settings, storageMode: mode })
                                    }
                                />
                            </div>
                        )}

                        {/* If no special settings apply */}
                        {!showPdfSettings && !showTabularSettings && (
                            <p className="text-sm text-muted-foreground">
                                No additional settings available for this source type.
                            </p>
                        )}
                    </div>
                </AccordionContent>
            </AccordionItem>
        </Accordion>
    );
}
