/**
 * Yggdrasil (Zitadel) SSO helpers shared by the login page and the logout route.
 *
 * The login page builds the authorize redirect from the backend's
 * `/auth/sso-config`. Logout must end the session at the same issuer, for the
 * same client, so both use the rules below instead of keeping their own copy.
 */

export type SsoConfig = {
    issuer: string;
    client_id: string;
    redirect_uri: string;
};

/** The parts of a browser location the rewrites below need (window.location has them). */
export type BrowserLocation = {
    protocol: string; // "https:" — with the trailing colon, like window.location.protocol
    hostname: string;
    host: string; // hostname[:port]
};

export const ISSUER_FALLBACK = process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";

/** Browser-facing issuer: a config left at localhost:8085 means the K3s NodePort 30085 on the host the browser used. */
export function browserIssuer(issuer: string, loc: BrowserLocation): string {
    if (issuer.includes("localhost:8085")) {
        return `${loc.protocol}//${loc.hostname}:30085`;
    }
    return issuer;
}

/** Browser-facing OIDC redirect URI: a config left at localhost:3001 means "this dashboard". */
export function browserRedirectUri(redirectUri: string, loc: BrowserLocation): string {
    if (redirectUri.includes("localhost:3001")) {
        return `${loc.protocol}//${loc.host}/login/callback`;
    }
    return redirectUri;
}

/** Where Yggdrasil sends the browser after logout: /login on the same origin login redirects back to. */
export function postLogoutRedirectUri(redirectUri: string, loc: BrowserLocation): string {
    try {
        return new URL("/login", browserRedirectUri(redirectUri, loc)).toString();
    } catch {
        return `${loc.protocol}//${loc.host}/login`;
    }
}

/** OIDC RP-initiated logout URL. Zitadel serves it at /oidc/v1/end_session (see its discovery document). */
export function endSessionUrl(opts: {
    issuer: string;
    clientId: string;
    postLogoutRedirectUri: string;
    idTokenHint?: string;
}): string {
    const url = new URL(`${opts.issuer.replace(/\/+$/, "")}/oidc/v1/end_session`);
    url.searchParams.set("client_id", opts.clientId);
    url.searchParams.set("post_logout_redirect_uri", opts.postLogoutRedirectUri);
    if (opts.idTokenHint) {
        url.searchParams.set("id_token_hint", opts.idTokenHint);
    }
    return url.toString();
}

/** Server-side base URL of mimir-api (in-cluster), as the token-exchange route uses it. */
export function serverMimirApiBase(): string {
    return process.env.MIMIR_API_URL || process.env.NEXT_PUBLIC_API_URL || "http://mimir-api.asgard.svc:8080/api";
}

/** Server route that clears the session cookies and ends the SSO session (see app/api/auth/logout). */
export const LOGOUT_ROUTE = "/api/auth/logout";

/** Cookies the dashboard keeps for a signed-in user. Logout clears every one of them. */
export const AUTH_COOKIES = ["access_token", "refresh_token", "id_token", "tenant_id", "user_role", "user_name"] as const;

/** httpOnly cookie holding Yggdrasil's id_token, used only as id_token_hint at logout. */
export const ID_TOKEN_COOKIE = "id_token";

/** Browsers drop a cookie over ~4096 bytes; keep the id_token only when it fits. */
export const ID_TOKEN_COOKIE_MAX_BYTES = 3800;
