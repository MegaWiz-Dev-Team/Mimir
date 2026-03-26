import { NextRequest, NextResponse } from "next/server";

/**
 * Server-side logout: clear all auth cookies and redirect to /login.
 * This is more reliable than client-side Cookies.remove() which can fail
 * due to path/domain mismatches.
 */
export async function GET(request: NextRequest) {
    const loginUrl = new URL("/login", request.url);
    const response = NextResponse.redirect(loginUrl);

    // Clear all auth-related cookies
    response.cookies.delete("access_token");
    response.cookies.delete("refresh_token");
    response.cookies.delete("tenant_id");
    response.cookies.delete("user_role");
    response.cookies.delete("user_name");

    return response;
}
