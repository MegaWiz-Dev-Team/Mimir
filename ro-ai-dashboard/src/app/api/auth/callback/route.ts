import { NextRequest, NextResponse } from "next/server";
import http from "node:http";

/**
 * Server-side OIDC token exchange.
 *
 * Uses Node.js http module instead of fetch because Zitadel requires
 * the Host header to match its externalDomain (localhost:30085),
 * and Node.js fetch strips custom Host headers for security.
 */

const YGGDRASIL_ISSUER = process.env.YGGDRASIL_ISSUER || process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";
const CLIENT_ID = process.env.YGGDRASIL_CLIENT_ID || process.env.NEXT_PUBLIC_YGGDRASIL_CLIENT_ID || "";
const CLIENT_SECRET = process.env.YGGDRASIL_CLIENT_SECRET || "";
const REDIRECT_URI = process.env.NEXT_PUBLIC_YGGDRASIL_REDIRECT_URI || "http://localhost:3001/login/callback";

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
        const { code, code_verifier, redirect_uri } = await request.json();

        if (!code) {
            return NextResponse.json({ error: "Missing authorization code" }, { status: 400 });
        }

        const tokenUrl = `${YGGDRASIL_ISSUER}/oauth/v2/token`;

        const params = new URLSearchParams({
            grant_type: "authorization_code",
            code,
            redirect_uri: redirect_uri || REDIRECT_URI,
            client_id: CLIENT_ID,
            client_secret: CLIENT_SECRET,
        });

        if (code_verifier) {
            params.set("code_verifier", code_verifier);
        }

        console.log(`[OIDC] Token exchange: url=${tokenUrl} client_id=${CLIENT_ID} redirect_uri=${redirect_uri || REDIRECT_URI}`);

        const result = await httpPost(tokenUrl, params.toString(), {
            "Content-Type": "application/x-www-form-urlencoded",
            "Host": "localhost:30085",
        });

        if (result.status >= 400) {
            console.error(`[OIDC] Token exchange failed: status=${result.status} body=${result.body}`);
            let errData: any = {};
            try { errData = JSON.parse(result.body); } catch {}
            return NextResponse.json(
                { error: errData.error_description || errData.error || `Token exchange failed (${result.status})` },
                { status: result.status }
            );
        }

        const tokens = JSON.parse(result.body);

        return NextResponse.json({
            access_token: tokens.access_token,
            id_token: tokens.id_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            token_type: tokens.token_type,
        });
    } catch (e: any) {
        console.error("[OIDC] Callback error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
