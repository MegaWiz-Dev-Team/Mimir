'use client';

import React from 'react';
import { CheckCircle, Clock, AlertCircle } from 'lucide-react';

interface TimelineStep {
  step: number;
  agent_name: string;
  agent_id: number;
  action: string;
  start_time: string;
  end_time: string;
  duration_ms: number;
  status: 'success' | 'pending' | 'error';
  output_preview?: string;
}

interface DispatchTimelineProps {
  steps: TimelineStep[];
  totalDuration?: number;
}

export const DispatchTimeline: React.FC<DispatchTimelineProps> = ({
  steps,
  totalDuration,
}) => {
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'success':
        return (
          <CheckCircle className="w-5 h-5 text-green-400" />
        );
      case 'pending':
        return (
          <Clock className="w-5 h-5 text-yellow-400 animate-spin" />
        );
      case 'error':
        return (
          <AlertCircle className="w-5 h-5 text-red-400" />
        );
      default:
        return null;
    }
  };

  const getTierColor = (step: number) => {
    if (step === 1) return 'from-red-600 to-red-400';
    if (step === 2) return 'from-teal-600 to-teal-400';
    return 'from-blue-600 to-blue-400';
  };

  return (
    <div className="space-y-6 py-4">
      {steps.map((step, idx) => (
        <div key={idx} className="flex gap-4">
          {/* Timeline connector */}
          <div className="flex flex-col items-center">
            <div
              className={`w-3 h-3 rounded-full bg-gradient-to-br ${getTierColor(
                step.step
              )}`}
            />
            {idx < steps.length - 1 && (
              <div className="w-1 h-12 bg-slate-700 mt-2" />
            )}
          </div>

          {/* Step content */}
          <div className="flex-1 pb-4">
            <div className="flex items-start justify-between">
              <div>
                <h4 className="font-semibold text-sm text-slate-100">
                  Step {step.step}: {step.agent_name}
                </h4>
                <p className="text-xs text-slate-400 mt-1">{step.action}</p>
              </div>
              <div className="flex items-center gap-2">
                {getStatusIcon(step.status)}
                <span className="text-sm font-bold text-slate-300">
                  {step.duration_ms}ms
                </span>
              </div>
            </div>

            {step.output_preview && (
              <div className="mt-2 bg-slate-800/50 rounded p-2 border border-slate-700/50">
                <p className="text-xs text-slate-300 line-clamp-2">
                  {step.output_preview}
                </p>
              </div>
            )}

            <div className="flex gap-4 mt-2 text-xs text-slate-500">
              <span>Start: {new Date(step.start_time).toLocaleTimeString()}</span>
              <span>End: {new Date(step.end_time).toLocaleTimeString()}</span>
            </div>
          </div>
        </div>
      ))}

      {totalDuration && (
        <div className="pt-4 border-t border-slate-700">
          <div className="bg-slate-800/50 rounded p-3">
            <p className="text-xs text-slate-400">Total Duration</p>
            <p className="text-lg font-bold text-teal-400">
              {totalDuration}ms
            </p>
          </div>
        </div>
      )}
    </div>
  );
};
