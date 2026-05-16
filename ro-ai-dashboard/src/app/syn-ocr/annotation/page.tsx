"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Loader2, ChevronLeft } from "lucide-react";
import { authFetch, API_BASE_URL } from "@/lib/api";

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

interface TaskDetail {
  id: string;
  case_id_label: string;
  image_path: string;
  ground_truth?: string;
  confidence?: string;
  issues?: string;
  notes?: string;
}

export default function AnnotationPage() {
  const [view, setView] = useState<"datasets" | "annotate">("datasets");
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [currentTask, setCurrentTask] = useState<TaskDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [groundTruth, setGroundTruth] = useState("");
  const [confidence, setConfidence] = useState("high");
  const [issues, setIssues] = useState<string[]>([]);
  const [notes, setNotes] = useState("");
  const [imageUrl, setImageUrl] = useState<string>("");

  useEffect(() => {
    loadDatasets();
  }, []);

  const loadDatasets = async () => {
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
  };

  const startAnnotating = async (dataset: Dataset) => {
    setLoading(true);
    try {
      const res = await authFetch(`${API_BASE}/ocr-annotation/tasks?dataset_id=${dataset.id}`);
      if (res.ok) {
        const data = await res.json();
        const task = data.tasks?.[0];
        if (task) {
          await loadTask(task.id);
          setView("annotate");
        }
      }
    } catch (e) {
      console.error("Failed to load tasks:", e);
    }
    setLoading(false);
  };

  const loadTask = async (taskId: string) => {
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

        // Load image
        try {
          const imgRes = await authFetch(`${API_BASE}/ocr-annotation/tasks/${taskId}/image`);
          if (imgRes.ok) {
            const blob = await imgRes.blob();
            setImageUrl(URL.createObjectURL(blob));
          }
        } catch {
          // Image failed to load, continue without it
        }
      }
    } catch (e) {
      console.error("Failed to load task:", e);
    }
    setLoading(false);
  };

  const toggleIssue = (issue: string) => {
    setIssues(prev =>
      prev.includes(issue)
        ? prev.filter(i => i !== issue)
        : [...prev, issue]
    );
  };

  // View 1: Dataset List
  if (view === "datasets") {
    return (
      <div className="min-h-screen bg-gradient-to-br from-slate-50 to-slate-100 dark:from-slate-950 dark:to-slate-900 p-8">
        <div className="max-w-4xl mx-auto">
          <h1 className="text-3xl font-bold mb-8">OCR Annotation</h1>

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
            <div className="space-y-4" data-testid="dataset-list">
              {datasets.map(dataset => {
                const progressPercent = Math.round((dataset.completed / dataset.total_cases) * 100);
                return (
                  <Card key={dataset.id} data-testid="dataset-item" className="hover:shadow-lg transition-shadow">
                    <CardHeader className="pb-3">
                      <div className="flex items-start justify-between">
                        <div>
                          <CardTitle className="text-lg" data-testid="dataset-name">{dataset.name}</CardTitle>
                          <p className="text-sm text-gray-500">v{dataset.version} • {dataset.total_cases} images</p>
                        </div>
                        <Button
                          onClick={() => startAnnotating(dataset)}
                          disabled={dataset.pending === 0 && dataset.in_progress === 0}
                          className="bg-blue-600 hover:bg-blue-700"
                          data-testid="annotate-btn"
                        >
                          Annotate →
                        </Button>
                      </div>
                    </CardHeader>
                    <CardContent>
                      <div className="space-y-3">
                        <div data-testid="progress-bar">
                          <div className="flex justify-between mb-1 text-sm">
                            <span className="font-medium">{progressPercent}% Complete</span>
                            <span className="text-gray-500">{dataset.completed}/{dataset.total_cases}</span>
                          </div>
                          <div className="w-full bg-gray-200 rounded-full h-2">
                            <div
                              className="bg-green-600 h-2 rounded-full transition-all"
                              style={{ width: `${progressPercent}%` }}
                            ></div>
                          </div>
                        </div>

                        <div className="flex gap-4 text-sm" data-testid="status-counts">
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

  // View 2: Annotation
  if (view === "annotate" && currentTask) {
    return (
      <div className="min-h-screen bg-white dark:bg-slate-950 p-8" data-testid="annotation-editor">
        <div className="max-w-6xl mx-auto">
          <div className="flex items-center justify-between mb-8 pb-6 border-b" data-testid="task-header">
            <div className="flex items-center gap-4">
              <Button
                variant="ghost"
                onClick={() => setView("datasets")}
                data-testid="back-to-datasets"
              >
                <ChevronLeft className="w-4 h-4" /> Back
              </Button>
              <div>
                <h2 className="text-2xl font-bold" data-testid="task-id">{currentTask.case_id_label}</h2>
                <p className="text-sm text-gray-500" data-testid="task-progress">Task detail</p>
              </div>
            </div>
          </div>

          {loading ? (
            <div className="flex justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-8">
              <div data-testid="image-preview">
                <div className="aspect-square bg-gray-100 rounded-lg overflow-hidden flex items-center justify-center">
                  {imageUrl ? (
                    <img src={imageUrl} alt={currentTask.case_id_label} className="w-full h-full object-contain" />
                  ) : (
                    <div className="text-gray-500">Loading image...</div>
                  )}
                </div>
              </div>

              <div className="space-y-6">
                <div>
                  <label className="block text-sm font-semibold mb-2">📝 Ground Truth</label>
                  <textarea
                    value={groundTruth}
                    onChange={(e) => setGroundTruth(e.target.value)}
                    placeholder="Type the correct text..."
                    className="w-full h-32 p-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500"
                    data-testid="ground-truth-input"
                  />
                </div>

                <div>
                  <label className="block text-sm font-semibold mb-2">Confidence</label>
                  <select
                    value={confidence}
                    onChange={(e) => setConfidence(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg"
                    data-testid="confidence-select"
                  >
                    <option value="high">High</option>
                    <option value="medium">Medium</option>
                    <option value="low">Low</option>
                  </select>
                </div>

                <div>
                  <label className="block text-sm font-semibold mb-2">Issues</label>
                  <div className="space-y-2">
                    {["Handwritten", "Blurry", "Partial", "Damaged"].map(issue => {
                      const testIdMap: Record<string, string> = {
                        "Handwritten": "issue-handwritten",
                        "Blurry": "issue-blurry",
                        "Partial": "issue-partial",
                        "Damaged": "issue-damaged",
                      };
                      return (
                        <label key={issue} className="flex items-center gap-2">
                          <input
                            type="checkbox"
                            checked={issues.includes(issue)}
                            onChange={() => toggleIssue(issue)}
                            className="w-4 h-4 rounded"
                            data-testid={testIdMap[issue]}
                          />
                          <span className="text-sm">{issue}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>

                <div>
                  <label className="block text-sm font-semibold mb-2">Notes</label>
                  <input
                    type="text"
                    value={notes}
                    onChange={(e) => setNotes(e.target.value)}
                    placeholder="Optional notes..."
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg"
                    data-testid="notes-input"
                  />
                </div>

                <div className="pt-4 border-t space-y-2">
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      onClick={() => setView("datasets")}
                      data-testid="skip-btn"
                    >
                      Skip
                    </Button>
                    <Button
                      variant="outline"
                      disabled={!groundTruth.trim()}
                      data-testid="save-draft-btn"
                    >
                      Save Draft
                    </Button>
                    <Button
                      onClick={() => setView("datasets")}
                      disabled={!groundTruth.trim()}
                      className="bg-green-600 hover:bg-green-700 ml-auto"
                      data-testid="complete-btn"
                    >
                      Complete →
                    </Button>
                  </div>
                  <p className="text-xs text-gray-500 text-center" data-testid="annotator-info">
                    Annotated by: you
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
