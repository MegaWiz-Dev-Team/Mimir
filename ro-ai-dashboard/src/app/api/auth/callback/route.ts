import { NextRequest, NextResponse } from "next/server";

export async function POST(request: NextRequest) {
    try {
        const { code, code_verifier, redirect_uri } = await request.json();

        if (!code) {
            return NextResponse.json({ error: "Missing authorization code" }, { status: 400 });
        }

        const MIMIR_API = process.env.MIMIR_API_URL || process.env.NEXT_PUBLIC_API_URL || "http://mimir-api.asgard.svc:8080/api";
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

        const tokenData = await res.json();
        console.log(`[OIDC] Backend SSO exchange successful. user_name=${tokenData.user_name} role=${tokenData.user_role} tenant=${tokenData.tenant_id}`);
        
        return NextResponse.json(tokenData);
    } catch (e: any) {
        console.error("[OIDC] Callback error:", e);
        return NextResponse.json({ error: e.message || "Internal error" }, { status: 500 });
    }
}
