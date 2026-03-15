/**
 * Tests for src/middleware.ts — Route protection via access_token cookie.
 *
 * NextRequest/NextResponse are not available in jsdom, so we mock next/server.
 */

// Must use jest.fn() inside the factory to avoid hoisting issues
jest.mock("next/server", () => {
    const redirect = jest.fn().mockImplementation((url: any) => ({
        status: 307,
        headers: new Map([["location", typeof url === "string" ? url : url.toString()]]),
    }));
    const next = jest.fn().mockReturnValue({
        status: 200,
        headers: new Map(),
    });
    return {
        NextResponse: { redirect, next },
        NextRequest: jest.fn(),
    };
});

// Import after mock setup
import { NextResponse } from "next/server";
import { middleware } from "./middleware";

// Helper to create a mock NextRequest-like object
function mockRequest(pathname: string, hasCookie: boolean = false) {
    return {
        nextUrl: { pathname },
        url: `http://localhost:3001${pathname}`,
        cookies: {
            get: (name: string) =>
                hasCookie && name === "access_token"
                    ? { value: "jwt-token" }
                    : undefined,
        },
    } as any;
}

describe("Route protection middleware", () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    it("allows /login through without token", () => {
        middleware(mockRequest("/login", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /login/callback through without token", () => {
        middleware(mockRequest("/login/callback", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /api/auth/callback through without token", () => {
        middleware(mockRequest("/api/auth/callback", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /api/auth/refresh through without token", () => {
        middleware(mockRequest("/api/auth/refresh", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /_next/static through without token", () => {
        middleware(mockRequest("/_next/static/chunk.js", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /favicon.ico through without token", () => {
        middleware(mockRequest("/favicon.ico", false));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("redirects / to /login when no token", () => {
        middleware(mockRequest("/", false));
        expect(NextResponse.redirect).toHaveBeenCalled();
        expect(NextResponse.next).not.toHaveBeenCalled();
    });

    it("redirects /settings to /login when no token", () => {
        middleware(mockRequest("/settings", false));
        expect(NextResponse.redirect).toHaveBeenCalled();
    });

    it("redirects /sources to /login when no token", () => {
        middleware(mockRequest("/sources", false));
        expect(NextResponse.redirect).toHaveBeenCalled();
    });

    it("allows / through with valid token", () => {
        middleware(mockRequest("/", true));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /sources through with valid token", () => {
        middleware(mockRequest("/sources", true));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });

    it("allows /settings through with valid token", () => {
        middleware(mockRequest("/settings", true));
        expect(NextResponse.next).toHaveBeenCalled();
        expect(NextResponse.redirect).not.toHaveBeenCalled();
    });
});
