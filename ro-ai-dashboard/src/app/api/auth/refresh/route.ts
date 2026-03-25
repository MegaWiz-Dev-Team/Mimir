import { NextRequest, NextResponse } from "next/server";
import http from "node:http";

/**
 * Server-side token refresh using refresh_token.
 *
 * Uses Node.js http module to set custom Host header for Zitadel.
 */

const YGGDRASIL_ISSUER = process.env.YGGDRASIL_ISSUER || process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";
const CLIENT_ID = process.env.YGGDRASIL_CLIENT_ID || process.env.NEXT_PUBLIC_YGGDRASIL_CLIENT_ID || "";
const CLIENT_SECRET = process.env.YGGDRASIL_CLIENT_SECRET || "";

function httpPost(url: string, body: string, headers: Record<string, string>): Promise<{ status: number; body: string }> {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const req = http.request(
            {
                hostname: parsed.hostname,
                port: parsed.port || 80,
                path: parsed.pathname + parsed.search,
                method: "POST",
                headers: {
                    ...headers,
                    "Content-Length": Buffer.byteLength(body),
                },
            },
            (res) => {
                let data = "";
                res.on("data", (chunk) => (data += chunk));
                res.on("end", () => resolve({ status: res.statusCode || 500, body: data }));
            }
        );
        req.on("error", reject);
        req.write(body);
        req.end();
    });
}

export async function POST(request: NextRequest) {
    try {
        const { refresh_token } = await request.json();

        if (!refresh_token) {
            return NextResponse.json({ error: "Missing refresh token" }, { status: 400 });
        }

        const tokenUrl = `${YGGDRASIL_ISSUER}/oauth/v2/token`;

        const params = new URLSearchParams({
            grant_type: "refresh_token",
            refresh_token,
            client_id: CLIENT_ID,
            client_secret: CLIENT_SECRET,
        });

        const result = await httpPost(tokenUrl, params.toString(), {
            "Content-Type": "application/x-www-form-urlencoded",
            "Host": "localhost:30085",
        });

        if (result.status >= 400) {
            const errData = JSON.parse(result.body).catch?.(() => ({})) || {};
            console.error("[OIDC] Token refresh failed:", result.body);
            return NextResponse.json(
                { error: errData.error_description || errData.error || "Token refresh failed" },
                { status: result.status }
            );
        }

        const tokens = JSON.parse(result.body);

        return NextResponse.json({
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
        });
    } catch (e: any) {
        console.error("[OIDC] Refresh error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
