// Eval-family registry — the single place a new evaluation family is declared.
//
// Adding NER, coding, STT, etc. is ONE entry here: the unified Scoreboard,
// run-detail metric cards and A/B compare are all family-agnostic (they read
// evx_metric.name / slice_dim / is_primary + this registry). No new page.
//
// This delivers what eval-tab-registry.tsx was meant to: families as config,
// not as bespoke UI surfaces.

export type EvalFamily =
    | "qa"
    | "rag"
    | "ocr"
    | "ocr_layout"
    | "ner"
    | "coding";

export interface FamilySpec {
    /** Human label for the family chip/section. */
    label: string;
    /** Metric name carried as is_primary in evx_metric (for display only). */
    primaryMetric: string;
    /** Lower-is-better metrics (CER/WER/leak_rate/max_abs_diff) invert colour + sort. */
    higherIsBetter: boolean;
    /** Slice dimensions worth breaking down in run detail. */
    sliceDimensions: string[];
    /** Format the primary value for display. */
    format: (v: number) => string;
    /** Optional gate: a value that "passes" (green) vs "fails" (red). */
    gate?: (v: number) => boolean;
}

const pct = (v: number) => `${(v * 100).toFixed(1)}%`;
const ratio = (v: number) => v.toFixed(3);
const points = (v: number) => v.toFixed(1); // 0-100 composite (HealthBench-style)

export const EVAL_FAMILIES: Record<EvalFamily, FamilySpec> = {
    qa: {
        label: "QA / Agent",
        primaryMetric: "overall_score",
        higherIsBetter: true,
        sliceDimensions: ["specialty", "difficulty"],
        format: points, // overall_score is a 0-100 composite, not a 1-5 rubric
        gate: (v) => v >= 50,
    },
    rag: {
        label: "RAG / Retrieval",
        primaryMetric: "hit_rate",
        higherIsBetter: true,
        sliceDimensions: ["channel"],
        format: pct,
        gate: (v) => v >= 0.75, // S1 Hit Rate@3 target
    },
    ocr: {
        label: "OCR (text)",
        primaryMetric: "cer",
        higherIsBetter: false,
        sliceDimensions: ["doc_type"],
        format: ratio,
        gate: (v) => v <= 0.1,
    },
    ocr_layout: {
        label: "OCR (layout)",
        primaryMetric: "ap50",
        higherIsBetter: true,
        sliceDimensions: ["region_type"],
        format: ratio,
    },
    ner: {
        label: "NER / PII",
        primaryMetric: "recall",
        higherIsBetter: true,
        sliceDimensions: ["entity_type", "ocr_noise"],
        format: pct,
        gate: (v) => v >= 0.98, // PII recall gate — a miss is a leak
    },
    coding: {
        label: "Medical coding",
        primaryMetric: "code_acc",
        higherIsBetter: true,
        sliceDimensions: ["code_system"],
        format: pct,
    },
};

/** Fallback so an unknown family from the API still renders sanely. */
export function familySpec(family: string): FamilySpec {
    return (
        EVAL_FAMILIES[family as EvalFamily] ?? {
            label: family,
            primaryMetric: "score",
            higherIsBetter: true,
            sliceDimensions: [],
            format: (v: number) => String(v),
        }
    );
}
