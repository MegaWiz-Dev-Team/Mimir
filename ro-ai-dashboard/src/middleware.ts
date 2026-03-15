import { NextRequest, NextResponse } from "next/server";

/**
 * Next.js Edge Middleware — Route protection via access_token cookie.
 *
 * Unauthenticated requests are redirected to /login.
 * Public paths (login, callback, static assets) bypass the check.
 */

const PUBLIC_PATHS = [
    "/login",
    "/api/auth",
    "/_next",
    "/favicon.ico",
];

function isPublicPath(pathname: string): boolean {
    return PUBLIC_PATHS.some((p) => pathname.startsWith(p));
}

export function middleware(request: NextRequest) {
    const { pathname } = request.nextUrl;

    // Allow public paths through
    if (isPublicPath(pathname)) {
        return NextResponse.next();
    }

    // Check for access_token cookie
    const token = request.cookies.get("access_token")?.value;

    if (!token) {
        const loginUrl = new URL("/login", request.url);
        return NextResponse.redirect(loginUrl);
    }

    return NextResponse.next();
}

export const config = {
    matcher: [
        /*
         * Match all request paths except:
         * - _next/static (static files)
         * - _next/image (image optimization)
         * - favicon.ico
         */
        "/((?!_next/static|_next/image|favicon.ico).*)",
    ],
};
