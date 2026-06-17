import { NextRequest, NextResponse } from "next/server";

// Server-side proxy to the in-cluster analytics-api geo/stats ops (mimir-geo).
// ADR-024 P4. Body: { op: "geo/h3" | "geo/buffer" | "stats/moran" | ..., ...args }.
// Override host with ANALYTICS_API_URL in non-cluster/dev runs.
const ANALYTICS_API =
    process.env.ANALYTICS_API_URL || "http://analytics-api.asgard.svc:8091";

export async function POST(req: NextRequest) {
    try {
        const { op, ...args } = await req.json();
        if (!/^(geo|stats)\/[a-z_]+$/.test(op || "")) {
            return NextResponse.json({ error: "bad op (expect geo/* or stats/*)" }, { status: 400 });
        }
        const resp = await fetch(`${ANALYTICS_API}/api/v1/analytics/${op}`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(args),
        });
        const data = await resp.json().catch(() => ({ error: "non-JSON response from analytics-api" }));
        return NextResponse.json(data, { status: resp.status });
    } catch (e) {
        return NextResponse.json({ error: `analytics-api unreachable: ${String(e)}` }, { status: 502 });
    }
}
