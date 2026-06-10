// Proxy → mimir-sleep-api /chat (Eir Assistant → eir-sleep id=75 via Bifrost).
export const runtime = "nodejs";
export const maxDuration = 120;

const API = process.env.SLEEP_API_URL || "http://mimir-sleep-api.asgard.svc:8910";

export async function POST(req: Request) {
  const body = await req.text();
  const r = await fetch(`${API}/chat`, {
    method: "POST",
    body,
    headers: { "content-type": "application/json" },
  });
  return new Response(r.body, {
    status: r.status,
    headers: { "content-type": r.headers.get("content-type") || "application/json" },
  });
}
