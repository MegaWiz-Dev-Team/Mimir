'use client';

import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

interface AgentResponse {
  agent_id: number;
  agent_name: string;
  display_name: string;
  message: string;
  model_id: string;
  latency_ms: number;
  confidence_score?: number;
  sources?: string[];
  timestamp: string;
  status: 'pending' | 'success' | 'error';
}

interface ConversationPanelProps {
  odinRequest: string;
  agentResponses: AgentResponse[];
  odinSynthesis?: string;
  loading?: boolean;
}

export const ConversationPanel: React.FC<ConversationPanelProps> = ({
  odinRequest,
  agentResponses,
  odinSynthesis,
  loading = false,
}) => {
  const [expandedResponse, setExpandedResponse] = useState<number | null>(null);

  return (
    <div className="space-y-4 h-full overflow-y-auto">
      {/* Odin Request */}
      <Card className="bg-red-950/20 border-red-800/50">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm font-bold text-red-400">
            Odin (Master Orchestrator)
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-slate-100">{odinRequest}</p>
          <div className="mt-2 flex gap-2 text-xs text-slate-400">
            <span>🔄 Model: gemini-3.5-flash</span>
            <span>⚡ Dispatching to specialists...</span>
          </div>
        </CardContent>
      </Card>

      {/* Specialist Responses */}
      <div className="space-y-3">
        {agentResponses.map((response, idx) => (
          <Card
            key={idx}
            className={`cursor-pointer transition-all ${
              expandedResponse === idx
                ? 'border-teal-500/50 bg-teal-950/20'
                : 'border-slate-700/50 hover:border-slate-600/70'
            }`}
            onClick={() =>
              setExpandedResponse(expandedResponse === idx ? null : idx)
            }
          >
            <CardHeader className="pb-2">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <CardTitle className="text-xs font-bold text-teal-400">
                    {response.display_name}
                  </CardTitle>
                  <p className="text-xs text-slate-400 mt-1">
                    {response.model_id}
                  </p>
                </div>
                <div className="text-right">
                  <div
                    className={`text-xs font-bold ${
                      response.status === 'success'
                        ? 'text-green-400'
                        : response.status === 'error'
                          ? 'text-red-400'
                          : 'text-yellow-400'
                    }`}
                  >
                    {response.latency_ms}ms
                  </div>
                  {response.confidence_score && (
                    <div className="text-xs text-slate-400 mt-1">
                      {Math.round(response.confidence_score * 100)}% confident
                    </div>
                  )}
                </div>
              </div>
            </CardHeader>

            {expandedResponse === idx && (
              <CardContent className="space-y-3">
                <div className="bg-slate-800/50 rounded p-3">
                  <p className="text-sm text-slate-100 whitespace-pre-wrap">
                    {response.message}
                  </p>
                </div>

                {response.sources && response.sources.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-xs font-semibold text-slate-400">Sources:</p>
                    <div className="space-y-1">
                      {response.sources.map((source, i) => (
                        <div
                          key={i}
                          className="text-xs text-blue-400 bg-slate-800/30 p-2 rounded truncate"
                        >
                          📄 {source}
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                <div className="text-xs text-slate-500 border-t border-slate-700 pt-2">
                  {response.timestamp}
                </div>
              </CardContent>
            )}
          </Card>
        ))}
      </div>

      {/* Odin Synthesis */}
      {odinSynthesis && (
        <Card className="bg-red-950/20 border-red-800/50">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-bold text-red-400">
              Odin (Synthesized Response)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-slate-100">{odinSynthesis}</p>
          </CardContent>
        </Card>
      )}

      {loading && agentResponses.length === 0 && (
        <div className="flex items-center justify-center py-8">
          <div className="animate-pulse text-slate-400">
            Waiting for agent responses...
          </div>
        </div>
      )}
    </div>
  );
};
