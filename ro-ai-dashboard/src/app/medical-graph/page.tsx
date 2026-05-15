"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCw, Network, Loader2, AlertCircle } from "lucide-react";
import Link from "next/link";

interface GraphStats {
  nodes: number;
  relationships: number;
  status: string;
}

interface GraphData {
  nodes: Array<{
    id: string;
    name: string;
    type: string;
  }>;
  links: Array<{
    source: string;
    target: string;
    type: string;
  }>;
}

const ENTITY_COLORS: Record<string, string> = {
  Condition: "#FF6B6B",
  Treatment: "#4ECDC4",
  Symptom: "#FFE66D",
  Gene: "#95E1D3",
  Disease: "#F38181",
  Protein: "#B4A7FF",
  Compound: "#FF9999",
  Phenotype: "#FFD700",
  Other: "#CCCCCC",
};

export default function MedicalGraphPage() {
  const [stats, setStats] = useState<GraphStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadStats = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch("https://mimir.asgard.internal/api/graph/stats", {
        method: "GET",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error(`API error: ${response.status}`);
      }

      const data = await response.json();
      setStats(data);
    } catch (err) {
      console.error("Failed to load medical graph stats:", err);
      setError(err instanceof Error ? err.message : "Failed to load graph data");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStats();
  }, []);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Medical Knowledge Graph</h1>
          <p className="text-gray-600 mt-2">
            Semantic visualization of Thai medical records and biomedical entities from PrimeKG
          </p>
        </div>
        <Button onClick={loadStats} disabled={loading} variant="outline" size="sm">
          <RefreshCw className="w-4 h-4 mr-2" />
          {loading ? "Loading..." : "Refresh"}
        </Button>
      </div>

      {/* Error Message */}
      {error && (
        <Card className="border-red-200 bg-red-50">
          <CardContent className="pt-6 flex items-start gap-3">
            <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
            <div>
              <h3 className="font-semibold text-red-900">Error Loading Graph</h3>
              <p className="text-sm text-red-700 mt-1">{error}</p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Statistics Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium text-gray-600 flex items-center gap-2">
              <Network className="w-4 h-4" />
              Total Nodes
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {loading ? <Loader2 className="w-6 h-6 animate-spin" /> : stats?.nodes || "—"}
            </div>
            <p className="text-xs text-gray-500 mt-2">
              Thai medical (9) + PrimeKG biomedical (198+)
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium text-gray-600">Relationships</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {loading ? <Loader2 className="w-6 h-6 animate-spin" /> : stats?.relationships || "—"}
            </div>
            <p className="text-xs text-gray-500 mt-2">
              Entity connections and references
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium text-gray-600">Status</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              <span className="inline-block w-3 h-3 bg-green-500 rounded-full mr-2"></span>
              {stats?.status === "operational" ? "Active" : "—"}
            </div>
            <p className="text-xs text-gray-500 mt-2">
              Neo4j backend
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Legend */}
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Entity Types</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {Object.entries(ENTITY_COLORS).map(([type, color]) => (
              <div key={type} className="flex items-center gap-2">
                <div
                  className="w-3 h-3 rounded-full"
                  style={{ backgroundColor: color }}
                ></div>
                <span className="text-sm text-gray-700">{type}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Graph Visualization */}
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Interactive Visualization</CardTitle>
          <CardDescription>
            Drag nodes to explore connections. Hover for details.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="bg-gray-50 rounded-lg border border-gray-200 overflow-hidden">
            <iframe
              src="https://mimir.asgard.internal/graph"
              className="w-full border-0"
              style={{ height: "600px" }}
              title="Medical Knowledge Graph Visualization"
            />
          </div>
        </CardContent>
      </Card>

      {/* Information */}
      <Card className="bg-blue-50 border-blue-200">
        <CardHeader>
          <CardTitle className="text-sm flex items-center gap-2">
            <AlertCircle className="w-4 h-4" />
            About This Graph
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-sm text-gray-700">
          <p>
            <strong>Data Sources:</strong>
          </p>
          <ul className="list-disc list-inside space-y-1 ml-2">
            <li>Thai medical OCR records (Syn) - 30 documents → 9 medical entities</li>
            <li>PrimeKG biomedical database - 198 genes, diseases, phenotypes</li>
            <li>Semantic relationships connecting entities</li>
          </ul>
          <p className="mt-3">
            <strong>Use Cases:</strong> Disease-symptom-treatment chains, gene-disease associations, biomedical entity relationships
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
