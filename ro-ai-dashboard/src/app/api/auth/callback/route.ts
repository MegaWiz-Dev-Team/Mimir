import { NextRequest, NextResponse } from "next/server";

/**
 * Server-side OIDC token exchange.
 *
 * Exchanges the authorization code for tokens using the client secret
 * (which is kept server-side for security).
 */

const ZITADEL_ISSUER = process.env.ZITADEL_ISSUER || process.env.NEXT_PUBLIC_ZITADEL_ISSUER || "http://localhost:8085";
const CLIENT_ID = process.env.ZITADEL_CLIENT_ID || process.env.NEXT_PUBLIC_ZITADEL_CLIENT_ID || "";
const CLIENT_SECRET = process.env.ZITADEL_CLIENT_SECRET || "";
const REDIRECT_URI = process.env.NEXT_PUBLIC_ZITADEL_REDIRECT_URI || "http://localhost:3001/login/callback";

export async function POST(request: NextRequest) {
    try {
        const { code, code_verifier } = await request.json();

        if (!code) {
            return NextResponse.json({ error: "Missing authorization code" }, { status: 400 });
        }

        // Exchange code for tokens at Zitadel token endpoint
        const tokenUrl = `${ZITADEL_ISSUER}/oauth/v2/token`;

        const params = new URLSearchParams({
            grant_type: "authorization_code",
            code,
            redirect_uri: REDIRECT_URI,
            client_id: CLIENT_ID,
            client_secret: CLIENT_SECRET,
        });

        // Add PKCE verifier if present
        if (code_verifier) {
            params.set("code_verifier", code_verifier);
        }

        const tokenRes = await fetch(tokenUrl, {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body: params.toString(),
        });

        if (!tokenRes.ok) {
            const errData = await tokenRes.json().catch(() => ({}));
            console.error("[OIDC] Token exchange failed:", errData);
            return NextResponse.json(
                { error: errData.error_description || errData.error || "Token exchange failed" },
                { status: tokenRes.status }
            );
        }

        const tokens = await tokenRes.json();

        return NextResponse.json({
            access_token: tokens.access_token,
            id_token: tokens.id_token,
            expires_in: tokens.expires_in,
            token_type: tokens.token_type,
        });
    } catch (e: any) {
        console.error("[OIDC] Callback error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
