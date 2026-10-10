import { NextRequest, NextResponse } from "next/server";
import { ID_TOKEN_COOKIE, ID_TOKEN_COOKIE_MAX_BYTES, serverMimirApiBase } from "@/lib/sso";

// Same lifetime as the user_role / user_name cookies the callback page sets.
const ID_TOKEN_COOKIE_MAX_AGE_S = 7 * 24 * 60 * 60;

export async function POST(request: NextRequest) {
    try {
        const { code, code_verifier, redirect_uri } = await request.json();

        if (!code) {
            return NextResponse.json({ error: "Missing authorization code" }, { status: 400 });
        }

        const MIMIR_API = serverMimirApiBase();
        const ssoExchangeUrl = `${MIMIR_API}/v1/auth/sso-exchange`;

        console.log(`[OIDC] Forwarding OAuth code to backend SSO exchange: ${ssoExchangeUrl}`);

        const res = await fetch(ssoExchangeUrl, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify({
                code,
                code_verifier,
                redirect_uri
            }),
        });

        if (!res.ok) {
            const errText = await res.text();
            console.error(`[OIDC] Backend SSO exchange failed: status=${res.status} body=${errText}`);
            let errData: any = {};
            try { errData = JSON.parse(errText); } catch {}
            return NextResponse.json(
                { error: errData.error_description || errData.error || `SSO Exchange failed (${res.status})` },
                { status: res.status }
            );
        }

        const { id_token, ...tokenData } = await res.json();
        console.log(`[OIDC] Backend SSO exchange successful. user_name=${tokenData.user_name} role=${tokenData.user_role} tenant=${tokenData.tenant_id}`);

        // Keep Yggdrasil's id_token out of page JavaScript (httpOnly cookie). Logout
        // sends it as id_token_hint, which tells Yggdrasil which session to end.
        const response = NextResponse.json(tokenData);
        if (typeof id_token === "string" && id_token.length > 0) {
            if (id_token.length <= ID_TOKEN_COOKIE_MAX_BYTES) {
                const proto = (request.headers.get("x-forwarded-proto") || request.nextUrl.protocol).split(",")[0].trim();
                response.cookies.set(ID_TOKEN_COOKIE, id_token, {
                    path: "/",
                    httpOnly: true,
                    sameSite: "lax",
                    secure: proto.replace(/:$/, "") === "https",
                    maxAge: ID_TOKEN_COOKIE_MAX_AGE_S,
                });
            } else {
                console.warn(`[OIDC] id_token is ${id_token.length} bytes, too large for a cookie; logout will send client_id only`);
            }
        }
        return response;
    } catch (e: any) {
        console.error("[OIDC] Callback error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
