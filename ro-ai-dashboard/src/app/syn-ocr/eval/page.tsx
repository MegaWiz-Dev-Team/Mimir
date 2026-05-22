"use client";

// OCR Eval — tabbed viewer over the two complementary eval axes:
//   • Text   — CER/WER per engine          (ocr_eval_*)
//   • Layout — region detection mAP / IoU   (ocr_layout_eval_*)
//
// Tenant is shared across tabs (sets the X-Tenant-Id sent per request); each
// domain (medical / insurance / wellness) sees its own runs, asgard_platform
// holds cross-cutting engineering benchmarks (the default).

import { useState } from "react";
import { OCR_EVAL_TENANTS } from "@/lib/ocr-eval-api";
import TextEvalTab from "@/components/ocr-eval/TextEvalTab";
import LayoutEvalTab from "@/components/ocr-eval/LayoutEvalTab";

type Tab = "text" | "layout";

export default function OcrEvalPage() {
    const [tenant, setTenant] = useState<string>(OCR_EVAL_TENANTS[0]);
    const [tab, setTab] = useState<Tab>("text");

    return (
        <div className="p-6 max-w-6xl mx-auto">
            <div className="flex items-center justify-between mb-1">
                <h1 className="text-2xl font-bold">OCR Eval</h1>
                <label className="flex items-center gap-2 text-xs text-gray-500">
                    Tenant
                    <select
                        value={tenant}
                        onChange={(e) => setTenant(e.target.value)}
                        className="px-2 py-1.5 text-sm border border-gray-300 rounded-md"
                    >
                        {OCR_EVAL_TENANTS.map((t) => (
                            <option key={t} value={t}>
                                {t}
                            </option>
                        ))}
                    </select>
                </label>
            </div>
            <p className="text-sm text-gray-500 mb-4">
                Two complementary axes of OCR quality. Text = did we read the characters
                right (CER/WER); Layout = did we find the regions right (mAP/IoU).
            </p>

            {/* Tabs */}
            <div className="flex gap-1 border-b border-gray-200 mb-5">
                <TabButton active={tab === "text"} onClick={() => setTab("text")}>
                    Text (CER/WER)
                </TabButton>
                <TabButton active={tab === "layout"} onClick={() => setTab("layout")}>
                    Layout (mAP)
                </TabButton>
            </div>

            {tab === "text" ? <TextEvalTab tenant={tenant} /> : <LayoutEvalTab tenant={tenant} />}
        </div>
    );
}

function TabButton({
    active,
    onClick,
    children,
}: {
    active: boolean;
    onClick: () => void;
    children: React.ReactNode;
}) {
    return (
        <button
            onClick={onClick}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${
                active
                    ? "border-blue-600 text-blue-600"
                    : "border-transparent text-gray-500 hover:text-gray-700"
            }`}
        >
            {children}
        </button>
    );
}
