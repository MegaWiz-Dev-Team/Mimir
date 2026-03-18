"use client";

import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";

interface GraphIngestionStatus {
  totalEntities: number;
  totalRelations: number;
  status: "idle" | "running" | "completed" | "error";
  lastRunAt?: string;
  entitiesByType?: { type: string; count: number }[];
}

interface GraphStatusProps {
  apiBase?: string;
  tenantId?: string;
  refreshInterval?: number; // ms
}

export function GraphStatus({
  apiBase = "http://localhost:8080",
  tenantId = "default_tenant",
  refreshInterval = 30000,
}: GraphStatusProps) {
  const [status, setStatus] = useState<GraphIngestionStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchStatus() {
      try {
        const resp = await fetch(`${apiBase}/api/v1/graph/stats`, {
          headers: { "X-Tenant-Id": tenantId },
        });
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const data = await resp.json();
        setStatus({
          totalEntities: data.total_entities || 0,
          totalRelations: data.total_relations || 0,
          status: data.total_entities > 0 ? "completed" : "idle",
          entitiesByType: data.entities_by_type,
        });
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Connection failed");
        setStatus(null);
      } finally {
        setLoading(false);
      }
    }

    fetchStatus();
    const interval = setInterval(fetchStatus, refreshInterval);
    return () => clearInterval(interval);
  }, [apiBase, tenantId, refreshInterval]);

  const getStatusBadge = () => {
    if (loading) return <StatusPill color="yellow" label="Checking..." />;
    if (error) return <StatusPill color="red" label="Offline" />;
    if (!status) return <StatusPill color="gray" label="Unknown" />;

    switch (status.status) {
      case "running":
        return <StatusPill color="blue" label="Extracting..." pulse />;
      case "completed":
        return <StatusPill color="green" label="Ready" />;
      case "error":
        return <StatusPill color="red" label="Error" />;
      default:
        return <StatusPill color="gray" label="No Data" />;
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium flex items-center gap-1.5">
          🔮 Knowledge Graph
        </span>
        {getStatusBadge()}
      </div>

      {status && !error && (
        <div className="grid grid-cols-2 gap-2 text-xs">
          <div className="p-2 bg-muted rounded-md text-center">
            <div className="font-medium text-foreground text-sm">
              {status.totalEntities.toLocaleString()}
            </div>
            <div className="text-muted-foreground uppercase tracking-wider text-[9px]">
              Entities
            </div>
          </div>
          <div className="p-2 bg-muted rounded-md text-center">
            <div className="font-medium text-foreground text-sm">
              {status.totalRelations.toLocaleString()}
            </div>
            <div className="text-muted-foreground uppercase tracking-wider text-[9px]">
              Relations
            </div>
          </div>
        </div>
      )}

      {status?.entitiesByType && status.entitiesByType.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {status.entitiesByType.slice(0, 5).map((t) => (
            <Badge
              key={t.type}
              variant="outline"
              className="text-[10px] px-1.5 py-0 h-5"
            >
              {t.type}: {t.count}
            </Badge>
          ))}
        </div>
      )}

      {error && (
        <p className="text-xs text-red-400">{error}</p>
      )}
    </div>
  );
}

function StatusPill({
  color,
  label,
  pulse = false,
}: {
  color: "green" | "yellow" | "red" | "blue" | "gray";
  label: string;
  pulse?: boolean;
}) {
  const colorMap = {
    green: "bg-green-500/10 text-green-500 border-green-500/20",
    yellow: "bg-yellow-500/10 text-yellow-500 border-yellow-500/20",
    red: "bg-red-500/10 text-red-500 border-red-500/20",
    blue: "bg-blue-500/10 text-blue-500 border-blue-500/20",
    gray: "bg-gray-500/10 text-gray-500 border-gray-500/20",
  };

  return (
    <Badge
      variant="outline"
      className={`text-[10px] px-1.5 h-5 ${colorMap[color]} ${pulse ? "animate-pulse" : ""}`}
    >
      {label}
    </Badge>
  );
}
