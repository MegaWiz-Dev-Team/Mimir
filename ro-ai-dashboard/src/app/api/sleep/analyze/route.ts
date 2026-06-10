// Proxy → mimir-sleep-api /analyze (Asgard Nótt, ADR-025 S3). Server-side so the
// browser stays same-origin and the in-cluster service DNS is used.
export const runtime = "nodejs";
export const maxDuration = 120;

const API = process.env.SLEEP_API_URL || "http://mimir-sleep-api.asgard.svc:8910";

export async function POST(req: Request) {
  const r = await fetch(`${API}/analyze`, {
    method: "POST",
    body: req.body,
    headers: { "content-type": req.headers.get("content-type") || "" },
    // @ts-expect-error Node fetch streaming upload
    duplex: "half",
  });
  return new Response(r.body, {
    status: r.status,
    headers: { "content-type": r.headers.get("content-type") || "application/json" },
  });
}
