import { NextRequest, NextResponse } from "next/server";
import http from "node:http";

/**
 * Server-side OIDC token exchange.
 *
 * Uses Node.js http module to set custom Host header for Zitadel.
 * After token exchange, fetches userinfo to get project roles.
 */

const YGGDRASIL_ISSUER = process.env.YGGDRASIL_ISSUER || process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";
const CLIENT_ID = process.env.YGGDRASIL_CLIENT_ID || process.env.NEXT_PUBLIC_YGGDRASIL_CLIENT_ID || "";
const CLIENT_SECRET = process.env.YGGDRASIL_CLIENT_SECRET || "";
const REDIRECT_URI = process.env.NEXT_PUBLIC_YGGDRASIL_REDIRECT_URI || "http://localhost:3001/login/callback";

function httpRequest(method: string, url: string, body: string | null, headers: Record<string, string>): Promise<{ status: number; body: string }> {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const opts: http.RequestOptions = {
            hostname: parsed.hostname,
            port: parsed.port || 80,
            path: parsed.pathname + parsed.search,
            method,
            headers: { ...headers },
        };
        if (body) {
            (opts.headers as Record<string, string>)["Content-Length"] = String(Buffer.byteLength(body));
        }
        const req = http.request(opts, (res) => {
            let data = "";
            res.on("data", (chunk) => (data += chunk));
            res.on("end", () => resolve({ status: res.statusCode || 500, body: data }));
        });
        req.on("error", reject);
        if (body) req.write(body);
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

        console.log(`[OIDC] Token exchange: url=${tokenUrl} client_id=${CLIENT_ID}`);

        const tokenResult = await httpRequest("POST", tokenUrl, params.toString(), {
            "Content-Type": "application/x-www-form-urlencoded",
            "Host": "localhost:30085",
        });

        if (tokenResult.status >= 400) {
            const errText = tokenResult.body;
            console.error(`[OIDC] Token exchange failed: status=${tokenResult.status} body=${errText}`);
            let errData: any = {};
            try { errData = JSON.parse(errText); } catch {}
            return NextResponse.json(
                { error: errData.error_description || errData.error || `Token exchange failed (${tokenResult.status})` },
                { status: tokenResult.status }
            );
        }

        const tokens = JSON.parse(tokenResult.body);

        // Fetch userinfo to get project roles (id_token doesn't include them)
        let userRole = "viewer";
        let userName = "";
        if (tokens.access_token) {
            try {
                const userinfoUrl = `${YGGDRASIL_ISSUER}/oidc/v1/userinfo`;
                const userinfoResult = await httpRequest("GET", userinfoUrl, null, {
                    "Authorization": `Bearer ${tokens.access_token}`,
                    "Host": "localhost:30085",
                });
                if (userinfoResult.status === 200) {
                    const userinfo = JSON.parse(userinfoResult.body);
                    console.log("[OIDC] userinfo:", JSON.stringify(userinfo));
                    userName = userinfo.name || userinfo.preferred_username || userinfo.email || "";
                    
                    // Extract roles from Zitadel userinfo
                    const projectRoles = userinfo["urn:zitadel:iam:org:project:roles"];
                    if (projectRoles && typeof projectRoles === "object") {
                        if ("SuperAdmin" in projectRoles) userRole = "SuperAdmin";
                        else if ("admin" in projectRoles) userRole = "admin";
                    }
                }
            } catch (e) {
                console.error("[OIDC] Failed to fetch userinfo:", e);
            }
        }

        console.log(`[OIDC] Token exchange successful. userRole=${userRole} userName=${userName}`);

        return NextResponse.json({
            access_token: tokens.access_token,
            id_token: tokens.id_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            token_type: tokens.token_type,
            user_role: userRole,
            user_name: userName,
        });
    } catch (e: any) {
        console.error("[OIDC] Callback error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
