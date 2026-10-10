/**
 * @jest-environment node
 *
 * Tests for /api/auth/callback — the id_token from the SSO exchange is kept in
 * an httpOnly cookie for logout and is not handed to page JavaScript.
 * The backend sso-exchange call is a mocked fetch: no network.
 */
import { NextRequest } from "next/server";
import { POST } from "./route";

const EXCHANGE = {
    access_token: "mimir-jwt",
    id_token: "header.payload.sig",
    refresh_token: "refresh",
    expires_in: 3600,
    token_type: "Bearer",
    user_role: "admin",
    user_name: "Test User",
    tenant_id: "tenant-1",
};

let fetchMock: jest.SpyInstance;

function callbackRequest(proto = "https"): NextRequest {
    return new NextRequest("http://0.0.0.0:3000/api/auth/callback", {
        method: "POST",
        headers: { "Content-Type": "application/json", host: "mimir.example.test", "x-forwarded-proto": proto },
        body: JSON.stringify({ code: "abc", code_verifier: "v", redirect_uri: "https://mimir.example.test/login/callback" }),
    });
}

beforeEach(() => {
    fetchMock = jest.spyOn(global, "fetch").mockImplementation(() => {
        throw new Error("unexpected fetch — mock the sso-exchange response first");
    });
    jest.spyOn(console, "log").mockImplementation(() => {});
    jest.spyOn(console, "warn").mockImplementation(() => {});
});

afterEach(() => {
    jest.restoreAllMocks();
});

describe("POST /api/auth/callback", () => {
    it("stores the id_token in an httpOnly cookie and leaves it out of the JSON body", async () => {
        fetchMock.mockResolvedValue(new Response(JSON.stringify(EXCHANGE), { status: 200 }));

        const res = await POST(callbackRequest());

        const body = await res.json();
        expect(body.access_token).toBe("mimir-jwt");
        expect(body.tenant_id).toBe("tenant-1");
        expect(body).not.toHaveProperty("id_token");

        const cookie = res.headers.getSetCookie().find((c) => c.startsWith("id_token="));
        expect(cookie).toBeDefined();
        expect(cookie).toContain("id_token=header.payload.sig");
        expect(cookie).toMatch(/HttpOnly/i);
        expect(cookie).toMatch(/SameSite=lax/i);
        expect(cookie).toMatch(/Secure/i);
        expect(cookie).toMatch(/Path=\//i);
    });

    it("does not mark the cookie Secure on plain http (K3s NodePort)", async () => {
        fetchMock.mockResolvedValue(new Response(JSON.stringify(EXCHANGE), { status: 200 }));

        const res = await POST(callbackRequest("http"));

        const cookie = res.headers.getSetCookie().find((c) => c.startsWith("id_token="));
        expect(cookie).toBeDefined();
        expect(cookie).not.toMatch(/Secure/i);
    });

    it("skips an id_token too large for a cookie", async () => {
        fetchMock.mockResolvedValue(
            new Response(JSON.stringify({ ...EXCHANGE, id_token: "x".repeat(5000) }), { status: 200 })
        );

        const res = await POST(callbackRequest());

        expect(res.status).toBe(200);
        expect(res.headers.getSetCookie().find((c) => c.startsWith("id_token="))).toBeUndefined();
    });

    it("sets no id_token cookie when the exchange returned none", async () => {
        const withoutIdToken: Partial<typeof EXCHANGE> = { ...EXCHANGE };
        delete withoutIdToken.id_token;
        fetchMock.mockResolvedValue(new Response(JSON.stringify(withoutIdToken), { status: 200 }));

        const res = await POST(callbackRequest());

        expect(res.status).toBe(200);
        expect(res.headers.getSetCookie().find((c) => c.startsWith("id_token="))).toBeUndefined();
    });
});
