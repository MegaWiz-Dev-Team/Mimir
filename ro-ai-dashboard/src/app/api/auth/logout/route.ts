import { NextRequest, NextResponse } from "next/server";
import {
    AUTH_COOKIES,
    BrowserLocation,
    ID_TOKEN_COOKIE,
    ISSUER_FALLBACK,
    SsoConfig,
    browserIssuer,
    endSessionUrl,
    postLogoutRedirectUri,
    serverMimirApiBase,
} from "@/lib/sso";

/**
 * Server-side logout (the navbar's Logout button navigates here):
 * 1. Clear every auth cookie, including the httpOnly id_token that
 *    client-side Cookies.remove cannot reach.
 * 2. Redirect to Yggdrasil's end_session endpoint so the SSO session ends too.
 *    Without this, /login signs the user straight back in.
 * 3. Yggdrasil redirects back to <dashboard>/login via post_logout_redirect_uri.
 *
 * Issuer, client id and redirect origin come from the backend's /auth/sso-config,
 * the same source the login page builds the authorize redirect from, so login and
 * logout always talk to the same issuer about the same client.
 *
 * Yggdrasil (Zitadel) only redirects back without its own "logged out" screen when
 * post_logout_redirect_uri is registered in the app's Post Logout URIs.
 */

export const dynamic = "force-dynamic";

const SSO_CONFIG_TIMEOUT_MS = 3000;

function firstHeaderValue(value: string | null): string {
    return (value || "").split(",")[0].trim();
}

/** The origin the browser used, as the ingress forwarded it. */
function requestLocation(request: NextRequest): BrowserLocation {
    const host =
        firstHeaderValue(request.headers.get("x-forwarded-host")) ||
        firstHeaderValue(request.headers.get("host")) ||
        request.nextUrl.host;
    const scheme =
        firstHeaderValue(request.headers.get("x-forwarded-proto")) ||
        request.nextUrl.protocol.replace(/:$/, "");
    const protocol = `${scheme}:`;
    return { protocol, host, hostname: new URL(`${protocol}//${host}`).hostname };
}

async function loadSsoConfig(): Promise<SsoConfig> {
    const res = await fetch(`${serverMimirApiBase()}/v1/auth/sso-config`, {
        cache: "no-store",
        signal: AbortSignal.timeout(SSO_CONFIG_TIMEOUT_MS),
    });
    if (!res.ok) {
        throw new Error(`sso-config returned ${res.status}`);
    }
    return res.json();
}

export async function GET(request: NextRequest) {
    const loc = requestLocation(request);
    const idToken = request.cookies.get(ID_TOKEN_COOKIE)?.value;

    let target: string;
    try {
        const config = await loadSsoConfig();
        if (!config.client_id) {
            throw new Error("SSO client id is not configured");
        }
        target = endSessionUrl({
            issuer: browserIssuer(config.issuer || ISSUER_FALLBACK, loc),
            clientId: config.client_id,
            postLogoutRedirectUri: postLogoutRedirectUri(config.redirect_uri || "", loc),
            idTokenHint: idToken,
        });
    } catch (e: any) {
        // Mimir's cookies are still cleared below; only the SSO session is left.
        // Say so on /login instead of silently signing the user back in.
        console.error(`[OIDC] Logout could not build the end_session redirect: ${e?.message || e}`);
        const login = new URL("/login", `${loc.protocol}//${loc.host}`);
        login.searchParams.set(
            "error",
            "You are signed out of Mimir, but the SSO session could not be ended. Close the browser to finish signing out."
        );
        target = login.toString();
    }

    const response = NextResponse.redirect(target);
    const secure = loc.protocol === "https:";
    for (const name of AUTH_COOKIES) {
        response.cookies.set(name, "", {
            path: "/",
            maxAge: 0,
            httpOnly: name === ID_TOKEN_COOKIE,
            sameSite: "lax",
            secure,
        });
    }
    return response;
}
