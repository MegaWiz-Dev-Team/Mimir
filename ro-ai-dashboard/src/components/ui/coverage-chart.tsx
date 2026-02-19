"use client";

import { RadialBarChart, RadialBar, PolarAngleAxis, ResponsiveContainer } from "recharts";

interface CoverageChartProps {
    score: number; // 0 to 1
}

export function CoverageChart({ score }: CoverageChartProps) {
    const percentage = Math.round(score * 100);
    const data = [{ name: "Coverage", value: percentage, fill: "#22c55e" }];

    return (
        <div className="relative h-[200px] w-full flex items-center justify-center">
            <ResponsiveContainer width="100%" height="100%">
                <RadialBarChart
                    innerRadius="70%"
                    outerRadius="100%"
                    barSize={15}
                    data={data}
                    startAngle={90}
                    endAngle={-270}
                >
                    <PolarAngleAxis type="number" domain={[0, 100]} angleAxisId={0} tick={false} />
                    <RadialBar
                        background
                        dataKey="value"
                        cornerRadius={10}
                    />
                </RadialBarChart>
            </ResponsiveContainer>
            <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
                <span className="text-4xl font-bold">{percentage}%</span>
                <span className="text-xs text-muted-foreground uppercase tracking-widest mt-1">Coverage</span>
            </div>
        </div>
    );
}
