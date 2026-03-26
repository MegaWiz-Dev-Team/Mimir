import { NextRequest, NextResponse } from "next/server";

/**
 * Server-side logout:
 * 1. Clear all auth cookies
 * 2. Redirect to Zitadel's end_session endpoint to kill the SSO session
 * 3. Zitadel then redirects back to our /login page via post_logout_redirect_uri
 *
 * Without step 2, the user gets auto-re-authenticated by Zitadel's active session.
 */
export async function GET(request: NextRequest) {
    // Build the post-logout redirect URL using browser-facing origin
    const referer = request.headers.get("referer");
    let origin = "http://localhost:30001";
    if (referer) {
        try { origin = new URL(referer).origin; } catch {}
    } else {
        const host = request.headers.get("host");
        if (host) origin = `http://${host}`;
    }

    const response = NextResponse.redirect(
        `http://localhost:30085/oidc/v2/end_session?` +
        `client_id=365685875977339411&` +
        `post_logout_redirect_uri=${encodeURIComponent(origin + "/login")}`
    );

    // Clear all auth-related cookies
    response.cookies.delete("access_token");
    response.cookies.delete("refresh_token");
    response.cookies.delete("tenant_id");
    response.cookies.delete("user_role");
    response.cookies.delete("user_name");

    return response;
}
