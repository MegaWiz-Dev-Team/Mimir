import { NextRequest, NextResponse } from "next/server";

/**
 * Server-side token refresh using refresh_token.
 *
 * Called by authFetch() when access_token expires (401).
 * Uses the refresh_token stored in httpOnly cookie to get a new access_token.
 */

const YGGDRASIL_ISSUER = process.env.YGGDRASIL_ISSUER || process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";
const CLIENT_ID = process.env.YGGDRASIL_CLIENT_ID || process.env.NEXT_PUBLIC_YGGDRASIL_CLIENT_ID || "";
const CLIENT_SECRET = process.env.YGGDRASIL_CLIENT_SECRET || "";

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

        const tokenRes = await fetch(tokenUrl, {
            method: "POST",
            headers: {
                "Content-Type": "application/x-www-form-urlencoded",
                "Host": "localhost:30085",
            },
            body: params.toString(),
        });

        if (!tokenRes.ok) {
            const errData = await tokenRes.json().catch(() => ({}));
            console.error("[OIDC] Token refresh failed:", errData);
            return NextResponse.json(
                { error: errData.error_description || errData.error || "Token refresh failed" },
                { status: tokenRes.status }
            );
        }

        const tokens = await tokenRes.json();

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
