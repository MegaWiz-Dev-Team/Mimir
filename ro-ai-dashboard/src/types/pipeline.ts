export type PipelineStatus = "RUNNING" | "COMPLETED" | "FAILED";
export type StepStatus = "PENDING" | "RUNNING" | "COMPLETED" | "FAILED";
export type StepType = "FULL_PROCESS" | "EXTRACT" | "GENERATE" | "VERIFY";

export interface PipelineRun {
  id: string;
  status: PipelineStatus;
  provider: string;
  model: string;
  started_at: string;
  finished_at: string | null;
}

export interface PipelineStep {
  id: number;
  file_name: string;
  chunk_index: number;
  status: StepStatus;
  step_type: StepType;
}

export interface RunDetails extends PipelineRun {
  steps: PipelineStep[];
}

export interface QAResult {
  id: number;
  question: string;
  answer: string;
  context: string | null;
}

export interface EvaluationReport {
  id: number;
  coverage_score: number;
  reasoning: string | null;
  atomic_facts: string[]; // List of facts
  missing_facts: string[]; // List of missing facts
}
