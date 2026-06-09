import { NextRequest, NextResponse } from "next/server";

// Server-side proxy to the in-cluster analytics-api (ClusterIP, not browser-reachable).
// ADR-024 P3. Override host with ANALYTICS_API_URL in non-cluster/dev runs.
const ANALYTICS_API =
    process.env.ANALYTICS_API_URL || "http://analytics-api.asgard.svc:8091";

export async function POST(req: NextRequest) {
    try {
        const body = await req.json();
        const resp = await fetch(`${ANALYTICS_API}/api/v1/analytics/query`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await resp.json().catch(() => ({ error: "non-JSON response from analytics-api" }));
        return NextResponse.json(data, { status: resp.status });
    } catch (e) {
        return NextResponse.json({ error: `analytics-api unreachable: ${String(e)}` }, { status: 502 });
    }
}
