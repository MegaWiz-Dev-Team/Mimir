import { NextRequest, NextResponse } from "next/server";

/**
 * Server-side logout: clear all auth cookies and redirect to /login.
 * Uses the Host header to construct the redirect URL so it works
 * correctly behind NodePort (container listens on 0.0.0.0:3000 but
 * browser accesses via localhost:30001).
 */
export async function GET(request: NextRequest) {
    // Use Referer or Host header to get the browser-facing origin
    const referer = request.headers.get("referer");
    let redirectUrl = "/login";

    if (referer) {
        try {
            const origin = new URL(referer).origin;
            redirectUrl = `${origin}/login`;
        } catch {}
    } else {
        // Fallback: use Host header
        const host = request.headers.get("host") || "localhost:30001";
        redirectUrl = `http://${host}/login`;
    }

    const response = NextResponse.redirect(redirectUrl);

    // Clear all auth-related cookies
    response.cookies.delete("access_token");
    response.cookies.delete("refresh_token");
    response.cookies.delete("tenant_id");
    response.cookies.delete("user_role");
    response.cookies.delete("user_name");

    return response;
}
