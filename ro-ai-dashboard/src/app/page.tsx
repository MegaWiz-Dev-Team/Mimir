"use client";

import { useEffect, useState, useMemo } from "react";
import { fetchStats, fetchSources, syncAllSources, StatsResponse, DataSource, SourceHealth as SourceHealthType } from "@/lib/api";
import { DashboardStats } from "@/components/dashboard/DashboardStats";
import { RecentActivity } from "@/components/dashboard/RecentActivity";
import { SourceHealth } from "@/components/dashboard/SourceHealth";
import { PipelineStatusTable } from "@/components/dashboard/PipelineStatusTable";
import { QuickActions } from "@/components/dashboard/QuickActions";

export default function Dashboard() {
  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [sources, setSources] = useState<DataSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);

  const loadData = async () => {
    try {
      const [statsData, sourcesData] = await Promise.all([
        fetchStats().catch(() => null),
        fetchSources().catch(() => []),
      ]);
      setStats(statsData);
      setSources(sourcesData);
    } catch (error) {
      console.warn("[Dashboard] Failed to load data:", error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, []);

  const handleSyncAll = async () => {
    setSyncing(true);
    try {
      await syncAllSources();
      setTimeout(loadData, 2000);
    } catch (error) {
      console.warn("[Dashboard] Sync all failed:", error);
    } finally {
      setSyncing(false);
    }
  };

  // Compute fallback stats from sources when stats API is unavailable
  const effectiveStats: StatsResponse | null = useMemo(() => {
    if (stats) return stats;
    if (sources.length === 0) return null;

    const healthy = sources.filter((s) => s.last_sync_status === "COMPLETED").length;
    const failed = sources.filter((s) => s.last_sync_status === "FAILED").length;
    const running = sources.filter((s) => s.last_sync_status === "RUNNING").length;
    const pending = sources.length - healthy - failed - running;
    const totalChunks = sources.reduce((sum, s) => sum + (s.total_chunks ?? 0), 0);
    const sourcesWithChunks = sources.filter((s) => (s.total_chunks ?? 0) > 0).length;

    return {
      total_sources: sources.length,
      total_chunks: totalChunks,
      qa_pairs: 0,
      vector_coverage: 0, // Vectorization not implemented yet — will be computed from Qdrant stats
      source_health: { healthy, failed, pending, running },
    };
  }, [stats, sources]);

  // Compute source health from sources data as fallback
  const effectiveHealth: SourceHealthType | null = useMemo(() => {
    if (stats?.source_health) return stats.source_health;
    if (sources.length === 0) return null;

    return {
      healthy: sources.filter((s) => s.last_sync_status === "COMPLETED").length,
      failed: sources.filter((s) => s.last_sync_status === "FAILED").length,
      running: sources.filter((s) => s.last_sync_status === "RUNNING").length,
      pending: sources.filter(
        (s) => !s.last_sync_status || s.last_sync_status === "PENDING"
      ).length,
    };
  }, [stats, sources]);

  return (
    <div className="container mx-auto px-6 py-8 space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">Your knowledge base at a glance</p>
      </div>

      {/* KPI Cards — uses fallback from sources if stats API fails */}
      <DashboardStats stats={effectiveStats} loading={loading} />

      {/* Recent Activity + Source Health */}
      <div className="grid gap-6 lg:grid-cols-5">
        <div className="lg:col-span-3">
          <RecentActivity sources={sources} loading={loading} />
        </div>
        <div className="lg:col-span-2">
          <SourceHealth health={effectiveHealth} loading={loading} />
        </div>
      </div>

      {/* Pipeline Status Table */}
      <PipelineStatusTable sources={sources} loading={loading} />

      {/* Quick Actions */}
      <div className="rounded-xl border bg-card p-5 shadow-sm">
        <h3 className="text-base font-semibold mb-3">Quick Actions</h3>
        <QuickActions onSyncAll={handleSyncAll} syncing={syncing} />
      </div>
    </div>
  );
}
