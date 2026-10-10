/**
 * @jest-environment node
 *
 * Tests for /api/auth/logout — clears every auth cookie and redirects to
 * Yggdrasil's end_session endpoint. The backend /auth/sso-config call is a
 * mocked fetch: these tests never reach the network.
 */
import { NextRequest } from "next/server";
import { GET } from "./route";
import { AUTH_COOKIES } from "@/lib/sso";

const SSO_CONFIG = {
    issuer: "https://sso.example.test",
    client_id: "client-from-config",
    redirect_uri: "https://mimir.example.test/login/callback",
};

let fetchMock: jest.SpyInstance;

function mockSsoConfig(body: unknown, status = 200) {
    fetchMock.mockResolvedValue(
        new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } })
    );
}

function logoutRequest(headers: Record<string, string> = {}, cookie = ""): NextRequest {
    return new NextRequest("http://0.0.0.0:3000/api/auth/logout", {
        headers: { host: "mimir.example.test", "x-forwarded-proto": "https", ...(cookie ? { cookie } : {}), ...headers },
    });
}

function location(res: Response): URL {
    const loc = res.headers.get("location");
    expect(loc).toBeTruthy();
    return new URL(loc as string);
}

beforeEach(() => {
    fetchMock = jest.spyOn(global, "fetch").mockImplementation(() => {
        throw new Error("unexpected fetch — mock the sso-config response first");
    });
    jest.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
    jest.restoreAllMocks();
});

describe("GET /api/auth/logout", () => {
    it("redirects to the configured issuer's end_session with client_id and post_logout_redirect_uri", async () => {
        mockSsoConfig(SSO_CONFIG);

        const res = await GET(logoutRequest());

        expect(res.status).toBeGreaterThanOrEqual(300);
        expect(res.status).toBeLessThan(400);
        const target = location(res);
        expect(`${target.origin}${target.pathname}`).toBe("https://sso.example.test/oidc/v1/end_session");
        expect(target.searchParams.get("client_id")).toBe("client-from-config");
        expect(target.searchParams.get("post_logout_redirect_uri")).toBe("https://mimir.example.test/login");
        expect(target.searchParams.has("id_token_hint")).toBe(false);
    });

    it("asks the backend for sso-config on the server-side mimir-api base, nothing else", async () => {
        mockSsoConfig(SSO_CONFIG);

        await GET(logoutRequest());

        expect(fetchMock).toHaveBeenCalledTimes(1);
        expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/v1\/auth\/sso-config$/);
    });

    it("sends the stored id_token as id_token_hint", async () => {
        mockSsoConfig(SSO_CONFIG);

        const res = await GET(logoutRequest({}, "access_token=a; id_token=header.payload.sig"));

        expect(location(res).searchParams.get("id_token_hint")).toBe("header.payload.sig");
    });

    it("clears every auth cookie, the httpOnly id_token included", async () => {
        mockSsoConfig(SSO_CONFIG);

        const res = await GET(logoutRequest({}, "access_token=a; refresh_token=r; id_token=t; tenant_id=x"));

        const setCookies = res.headers.getSetCookie();
        for (const name of AUTH_COOKIES) {
            const header = setCookies.find((c) => c.startsWith(`${name}=`));
            expect(header).toBeDefined();
            expect(header).toMatch(/Max-Age=0/i);
            expect(header).toMatch(/Path=\//i);
        }
        expect(setCookies.find((c) => c.startsWith("id_token="))).toMatch(/HttpOnly/i);
    });

    it("builds the redirect from the browser's host when the config still points at the local defaults", async () => {
        mockSsoConfig({
            issuer: "http://localhost:8085",
            client_id: "client-from-config",
            redirect_uri: "http://localhost:3001/login/callback",
        });

        const res = await GET(
            new NextRequest("http://0.0.0.0:3000/api/auth/logout", { headers: { host: "10.0.0.5:30001" } })
        );

        const target = location(res);
        expect(`${target.origin}${target.pathname}`).toBe("http://10.0.0.5:30085/oidc/v1/end_session");
        expect(target.searchParams.get("post_logout_redirect_uri")).toBe("http://10.0.0.5:30001/login");
    });

    it("prefers the forwarded host and scheme set by the ingress", async () => {
        mockSsoConfig({ ...SSO_CONFIG, redirect_uri: "http://localhost:3001/login/callback" });

        const res = await GET(
            logoutRequest({ host: "mimir-dashboard:3000", "x-forwarded-host": "mimir.example.test", "x-forwarded-proto": "https" })
        );

        expect(location(res).searchParams.get("post_logout_redirect_uri")).toBe("https://mimir.example.test/login");
    });

    it("never invents a client id: without one it clears cookies and explains on /login", async () => {
        mockSsoConfig({ ...SSO_CONFIG, client_id: "" });

        const res = await GET(logoutRequest({}, "access_token=a"));

        const target = location(res);
        expect(`${target.origin}${target.pathname}`).toBe("https://mimir.example.test/login");
        expect(target.searchParams.get("error")).toMatch(/SSO session could not be ended/);
        expect(res.headers.getSetCookie().find((c) => c.startsWith("access_token="))).toMatch(/Max-Age=0/i);
    });

    it("still clears cookies when the backend cannot be reached", async () => {
        fetchMock.mockRejectedValue(new Error("connect ECONNREFUSED"));

        const res = await GET(logoutRequest({}, "access_token=a"));

        expect(location(res).pathname).toBe("/login");
        expect(location(res).searchParams.get("error")).toMatch(/SSO session could not be ended/);
        expect(res.headers.getSetCookie().find((c) => c.startsWith("access_token="))).toMatch(/Max-Age=0/i);
    });
});
