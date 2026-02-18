"use client";

import { useEffect, useState } from "react";
import { fetchRuns, triggerRun } from "@/lib/api";
import { PipelineRun } from "@/types/pipeline";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/components/ui/status-badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PlayCircle, RefreshCw } from "lucide-react";
import Link from "next/link";

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Label } from "@/components/ui/label";

export default function Dashboard() {
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [triggering, setTriggering] = useState(false);
  const [provider, setProvider] = useState("ollama");
  const [model, setModel] = useState("llama3.2");

  const loadRuns = async () => {
    setLoading(true);
    try {
      const data = await fetchRuns();
      setRuns(data);
    } catch (error) {
      console.error("Failed to load runs", error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadRuns();
    // Auto-refresh every 5 seconds
    const interval = setInterval(loadRuns, 5000);
    return () => clearInterval(interval);
  }, []);

  // Update default model when provider changes
  useEffect(() => {
    if (provider === "ollama") setModel("llama3.2");
    if (provider === "gemini") setModel("gemini-2.5-flash");
  }, [provider]);

  const handleRun = async () => {
    setTriggering(true);
    try {
      await triggerRun(provider, model, false);
      // Wait a bit and refresh
      setTimeout(loadRuns, 1000);
    } catch (error) {
      alert("Failed to start run");
    } finally {
      setTriggering(false);
    }
  };

  return (
    <div className="container mx-auto p-8">
      <div className="flex justify-between items-end mb-8">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">RO-AI Pipeline Monitor</h1>
          <p className="text-muted-foreground">Manage and track Q/A generation pipelines.</p>
        </div>

        <div className="flex items-end gap-3">
          <div className="grid w-[140px] gap-1.5">
            <Label htmlFor="provider" className="text-xs">Provider</Label>
            <Select value={provider} onValueChange={setProvider}>
              <SelectTrigger id="provider" className="h-9">
                <SelectValue placeholder="Provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="ollama">Ollama (Local)</SelectItem>
                <SelectItem value="gemini">Gemini (Cloud)</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="grid w-[180px] gap-1.5">
            <Label htmlFor="model" className="text-xs">Model</Label>
            <Select value={model} onValueChange={setModel}>
              <SelectTrigger id="model" className="h-9">
                <SelectValue placeholder="Model" />
              </SelectTrigger>
              <SelectContent>
                {provider === "ollama" ? (
                  <>
                    <SelectItem value="llama3.2">Llama 3.2</SelectItem>
                    <SelectItem value="mistral">Mistral</SelectItem>
                  </>
                ) : (
                  <>
                    <SelectItem value="gemini-2.5-flash">Gemini 2.5 Flash</SelectItem>
                    <SelectItem value="gemini-flash-latest">Gemini Flash Latest</SelectItem>
                    <SelectItem value="gemini-3-flash-preview">Gemini 3.0 Flash Preview</SelectItem>
                    <SelectItem value="gemini-3-pro-preview">Gemini 3.0 Pro Preview</SelectItem>
                  </>
                )}
              </SelectContent>
            </Select>
          </div>

          <div className="flex gap-2">
            <Button variant="outline" size="sm" className="h-9" onClick={loadRuns} disabled={loading}>
              <RefreshCw className={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
              Refresh
            </Button>
            <Button size="sm" className="h-9" onClick={handleRun} disabled={triggering}>
              <PlayCircle className="mr-2 h-4 w-4" />
              {triggering ? "Starting..." : "Run Pipeline"}
            </Button>
          </div>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3 mb-8">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Runs</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{runs.length}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Success Rate</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {runs.length > 0
                ? Math.round(
                  (runs.filter((r) => r.status === "COMPLETED").length / runs.length) * 100
                )
                : 0}
              %
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Active Runs</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {runs.filter((r) => r.status === "RUNNING").length}
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Recent Runs</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Run ID</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Provider</TableHead>
                <TableHead>Model</TableHead>
                <TableHead>Started At</TableHead>
                <TableHead>Duration</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {runs.map((run) => (
                <TableRow key={run.id}>
                  <TableCell className="font-mono text-xs">{run.id.substring(0, 8)}...</TableCell>
                  <TableCell>
                    <StatusBadge status={run.status} />
                  </TableCell>
                  <TableCell>{run.provider}</TableCell>
                  <TableCell>{run.model}</TableCell>
                  <TableCell suppressHydrationWarning>{new Date(run.started_at).toLocaleString()}</TableCell>
                  <TableCell>
                    {run.finished_at
                      ? `${(
                        (new Date(run.finished_at).getTime() - new Date(run.started_at).getTime()) /
                        1000
                      ).toFixed(1)}s`
                      : "-"}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button asChild variant="ghost" size="sm">
                      <Link href={`/runs/${run.id}`}>Details</Link>
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
              {runs.length === 0 && !loading && (
                <TableRow>
                  <TableCell colSpan={7} className="text-center h-24 text-muted-foreground">
                    No runs found.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
