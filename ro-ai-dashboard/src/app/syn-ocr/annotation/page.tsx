"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/ui/status-badge";
import { Loader2, ChevronLeft, Copy, Check, AlertTriangle } from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";
import Link from "next/link";

const API_BASE = API_BASE_URL;

interface Dataset {
  id: string;
  name: string;
  version: number;
  total_cases: number;
  completed: number;
  in_progress: number;
  pending: number;
}

interface AnnotationTask {
  id: string;
  case_id: string;
  case_id_label: string;
  image_path: string;
  status: string;
  annotator_id?: string;
  confidence?: string;
  ground_truth?: string;
}

interface TaskDetail extends AnnotationTask {
  issues?: string;
  notes?: string;
}

export default function AnnotationPage() {
  const [view, setView] = useState<"datasets" | "annotate">("datasets");
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [selectedDataset, setSelectedDataset] = useState<Dataset | null>(null);
  const [tasks, setTasks] = useState<AnnotationTask[]>([]);
  const [currentTask, setCurrentTask] = useState<TaskDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [imageUrl, setImageUrl] = useState<string>("");
  const [groundTruth, setGroundTruth] = useState("");
  const [confidence, setConfidence] = useState("high");
  const [issues, setIssues] = useState<string[]>([]);
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const [copied, setCopied] = useState(false);

  // Load datasets on mount
  useEffect(() => {
    loadDatasets();
  }, []);

  const loadDatasets = useCallback(async () => {
    setLoading(true);
    try {
      const res = await authFetch(`${API_BASE}/ocr-annotation/datasets`);
      if (res.ok) {
        const data = await res.json();
        setDatasets(data.datasets || []);
      }
    } catch (e) {
      console.error("Failed to load datasets:", e);
    }
    setLoading(false);
  }, []);

  const startAnnotating = useCallback(async (dataset: Dataset) => {
    setLoading(true);
    try {
      const res = await authFetch(`${API_BASE}/ocr-annotation/tasks?dataset_id=${dataset.id}&status=`);
      if (res.ok) {
        const data = await res.json();
        setTasks(data.tasks || []);
        setSelectedDataset(dataset);

        // Load first pending task
        const pendingTask = data.tasks?.find((t: AnnotationTask) => t.status === "pending" || t.status === "in_progress");
        if (pendingTask) {
          loadTask(pendingTask.id);
        }
        setView("annotate");
      }
    } catch (e) {
      console.error("Failed to load tasks:", e);
    }
    setLoading(false);
  }, []);

  const loadTask = useCallback(async (taskId: string) => {
    setLoading(true);
    try {
      const res = await authFetch(`${API_BASE}/ocr-annotation/tasks/${taskId}`);
      if (res.ok) {
        const data = await res.json();
        const task = data.task;
        setCurrentTask(task);
        setGroundTruth(task.ground_truth || "");
        setConfidence(task.confidence || "high");
        setIssues(task.issues ? JSON.parse(task.issues) : []);
        setNotes(task.notes || "");

        // Set image URL for proxy
        const imgRes = await authFetch(`${API_BASE}/ocr-annotation/tasks/${taskId}/image`);
        if (imgRes.ok) {
          const blob = await imgRes.blob();
          setImageUrl(URL.createObjectURL(blob));
        }
      }
    } catch (e) {
      console.error("Failed to load task:", e);
    }
    setLoading(false);
  }, []);

  const handleSaveAnnotation = async (final: boolean) => {
    if (!currentTask) return;
    setSaving(true);

    try {
      const res = await authFetch(`${API_BASE}/ocr-annotation/tasks/${currentTask.id}/save`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ground_truth: groundTruth,
          confidence: confidence,
          issues: issues,
          notes: notes,
          final_submit: final,
        }),
      });

      if (res.ok) {
        if (final) {
          // Move to next task
          const currentIdx = tasks.findIndex(t => t.id === currentTask.id);
          const nextTask = tasks[currentIdx + 1];
          if (nextTask) {
            loadTask(nextTask.id);
          } else {
            setView("datasets");
            loadDatasets();
          }
        }
      }
    } catch (e) {
      console.error("Failed to save annotation:", e);
      alert("Error saving annotation");
    }
    setSaving(false);
  };

  const toggleIssue = (issue: string) => {
    setIssues(prev =>
      prev.includes(issue)
        ? prev.filter(i => i !== issue)
        : [...prev, issue]
    );
  };

  const copyToGroundTruth = () => {
    // In real impl, would get OCR preview from task data
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const getProgressPercent = () => {
    if (!selectedDataset) return 0;
    return Math.round((selectedDataset.completed / selectedDataset.total_cases) * 100);
  };

  const getTaskIndex = () => {
    if (!currentTask || !selectedDataset) return 0;
    const completed = selectedDataset.completed;
    const inProgress = selectedDataset.in_progress;
    return completed + (inProgress > 0 ? 1 : 0);
  };

  // View 1: Dataset List
  if (view === "datasets") {
    return (
      <div className="min-h-screen bg-gradient-to-br from-slate-50 to-slate-100 dark:from-slate-950 dark:to-slate-900 p-8">
        <div className="max-w-4xl mx-auto">
          <div className="mb-8">
            <h1 className="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-2">OCR Annotation</h1>
            <p className="text-gray-600 dark:text-gray-400">Create ground truth data for OCR benchmarks</p>
          </div>

          {loading ? (
            <div className="flex justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : datasets.length === 0 ? (
            <Card>
              <CardContent className="text-center py-12 text-gray-500">
                No datasets available. Create a dataset in evaluations first.
              </CardContent>
            </Card>
          ) : (
            <div className="space-y-4">
              {datasets.map(dataset => {
                const progressPercent = Math.round((dataset.completed / dataset.total_cases) * 100);
                return (
                  <Card key={dataset.id} className="hover:shadow-lg transition-shadow">
                    <CardHeader className="pb-3">
                      <div className="flex items-start justify-between">
                        <div>
                          <CardTitle className="text-lg">{dataset.name}</CardTitle>
                          <p className="text-sm text-gray-500 dark:text-gray-400">v{dataset.version} • {dataset.total_cases} images</p>
                        </div>
                        <Button
                          onClick={() => startAnnotating(dataset)}
                          disabled={dataset.pending === 0 && dataset.in_progress === 0}
                          className="bg-blue-600 hover:bg-blue-700"
                        >
                          Annotate →
                        </Button>
                      </div>
                    </CardHeader>
                    <CardContent>
                      <div className="space-y-3">
                        {/* Progress bar */}
                        <div>
                          <div className="flex justify-between mb-1 text-sm">
                            <span className="font-medium">{progressPercent}% Complete</span>
                            <span className="text-gray-500">{dataset.completed}/{dataset.total_cases}</span>
                          </div>
                          <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                            <div
                              className="bg-green-600 h-2 rounded-full transition-all"
                              style={{ width: `${progressPercent}%` }}
                            ></div>
                          </div>
                        </div>

                        {/* Status breakdown */}
                        <div className="flex gap-4 text-sm">
                          <div>
                            <span className="text-gray-500">In Progress: </span>
                            <span className="font-medium text-blue-600">{dataset.in_progress}</span>
                          </div>
                          <div>
                            <span className="text-gray-500">Pending: </span>
                            <span className="font-medium text-yellow-600">{dataset.pending}</span>
                          </div>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                );
              })}
            </div>
          )}
        </div>
      </div>
    );
  }

  // View 2: Single Annotation
  if (view === "annotate" && currentTask) {
    const taskIndex = getTaskIndex();
    const progressPercent = getProgressPercent();

    return (
      <div className="min-h-screen bg-white dark:bg-slate-950 p-8">
        <div className="max-w-6xl mx-auto">
          {/* Header with back button */}
          <div className="flex items-center justify-between mb-8 pb-6 border-b border-gray-200 dark:border-gray-800">
            <div className="flex items-center gap-4">
              <Button
                variant="ghost"
                onClick={() => setView("datasets")}
                className="gap-2"
              >
                <ChevronLeft className="w-4 h-4" /> Back
              </Button>
              <div>
                <h2 className="text-2xl font-bold">{currentTask.case_id_label}</h2>
                <p className="text-sm text-gray-500">{taskIndex} of {selectedDataset?.total_cases}</p>
              </div>
            </div>
            <div className="text-right">
              <div className="w-32 bg-gray-200 dark:bg-gray-700 rounded-full h-2 mb-2">
                <div
                  className="bg-blue-600 h-2 rounded-full transition-all"
                  style={{ width: `${progressPercent}%` }}
                ></div>
              </div>
              <p className="text-sm font-medium">{progressPercent}% Complete</p>
            </div>
          </div>

          {loading ? (
            <div className="flex justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-8">
              {/* Image Column */}
              <div>
                <div className="aspect-square bg-gray-100 dark:bg-gray-800 rounded-lg overflow-hidden flex items-center justify-center">
                  {imageUrl ? (
                    <img src={imageUrl} alt={currentTask.case_id_label} className="w-full h-full object-contain" />
                  ) : (
                    <div className="text-gray-500">Loading image...</div>
                  )}
                </div>
                <p className="text-xs text-gray-500 mt-2">{currentTask.image_path}</p>
              </div>

              {/* Form Column */}
              <div className="space-y-6">
                {/* Ground Truth */}
                <div>
                  <label className="block text-sm font-semibold mb-2">📝 Ground Truth</label>
                  <textarea
                    value={groundTruth}
                    onChange={(e) => setGroundTruth(e.target.value)}
                    placeholder="Type the correct text from the image..."
                    className="w-full h-32 p-3 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  />
                </div>

                {/* OCR Preview (placeholder) */}
                <div>
                  <label className="block text-sm font-semibold mb-2">OCR Preview</label>
                  <div className="p-3 bg-gray-100 dark:bg-gray-800 rounded-lg text-sm text-gray-600 dark:text-gray-400 italic">
                    [OCR output would appear here]
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={copyToGroundTruth}
                    className="mt-2 gap-2 text-xs"
                  >
                    {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                    {copied ? "Copied!" : "Copy to Ground Truth"}
                  </Button>
                </div>

                {/* Confidence */}
                <div>
                  <label className="block text-sm font-semibold mb-2">Confidence</label>
                  <select
                    value={confidence}
                    onChange={(e) => setConfidence(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100"
                  >
                    <option value="high">High</option>
                    <option value="medium">Medium</option>
                    <option value="low">Low</option>
                  </select>
                </div>

                {/* Issues */}
                <div>
                  <label className="block text-sm font-semibold mb-2">Issues</label>
                  <div className="space-y-2">
                    {["Handwritten", "Blurry", "Partial", "Damaged"].map(issue => (
                      <label key={issue} className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={issues.includes(issue)}
                          onChange={() => toggleIssue(issue)}
                          className="w-4 h-4 rounded"
                        />
                        <span className="text-sm">{issue}</span>
                      </label>
                    ))}
                  </div>
                </div>

                {/* Notes */}
                <div>
                  <label className="block text-sm font-semibold mb-2">Notes</label>
                  <input
                    type="text"
                    value={notes}
                    onChange={(e) => setNotes(e.target.value)}
                    placeholder="Optional notes..."
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100"
                  />
                </div>

                {/* Actions */}
                <div className="pt-4 border-t border-gray-200 dark:border-gray-800 space-y-2">
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      onClick={() => setView("datasets")}
                      disabled={saving}
                    >
                      Skip
                    </Button>
                    <Button
                      variant="outline"
                      onClick={() => handleSaveAnnotation(false)}
                      disabled={saving || !groundTruth.trim()}
                    >
                      {saving ? "Saving..." : "Save Draft"}
                    </Button>
                    <Button
                      onClick={() => handleSaveAnnotation(true)}
                      disabled={saving || !groundTruth.trim()}
                      className="bg-green-600 hover:bg-green-700 ml-auto"
                    >
                      {saving ? "Saving..." : "Complete →"}
                    </Button>
                  </div>
                  <p className="text-xs text-gray-500 text-center">
                    Annotated by: {currentTask.annotator_id || "you"}
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  return null;
}
