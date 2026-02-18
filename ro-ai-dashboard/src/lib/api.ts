import { PipelineRun, RunDetails, QAResult, EvaluationReport } from "@/types/pipeline";

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api";

export async function fetchRuns(): Promise<PipelineRun[]> {
    const res = await fetch(`${API_BASE_URL}/pipeline/runs`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch runs");
    return res.json();
}

export async function fetchRunDetails(id: string): Promise<RunDetails> {
    const res = await fetch(`${API_BASE_URL}/pipeline/runs/${id}`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch run details");
    return res.json();
}

export async function fetchStepQA(stepId: number): Promise<QAResult[]> {
    const res = await fetch(`${API_BASE_URL}/pipeline/steps/${stepId}/qa`, { cache: "no-store" });
    if (!res.ok) throw new Error("Failed to fetch QA results");
    return res.json();
}

export async function fetchStepReport(stepId: number): Promise<EvaluationReport> {
    const res = await fetch(`${API_BASE_URL}/pipeline/steps/${stepId}/report`, { cache: "no-store" });
    // Report might be 404 if not ready, handle gracefully in UI or here
    if (res.status === 404) return { id: 0, coverage_score: 0, reasoning: "Not available", atomic_facts: [], missing_facts: [] };
    if (!res.ok) throw new Error("Failed to fetch report");
    return res.json();
}

export async function triggerRun(provider: string = "ollama", model: string = "llama3.2", testRun: boolean = false) {
    const res = await fetch(`${API_BASE_URL}/pipeline/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, model, test_run: testRun }),
    });
    if (!res.ok) throw new Error("Failed to trigger run");
    return res.json();
}

export async function retryStep(stepId: number) {
    const res = await fetch(`${API_BASE_URL}/pipeline/steps/${stepId}/retry`, {
        method: "POST",
    });
    if (!res.ok) throw new Error("Failed to retry step");
    return res;
}
