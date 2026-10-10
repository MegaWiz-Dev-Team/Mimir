import { browserIssuer, browserRedirectUri, endSessionUrl, postLogoutRedirectUri } from "./sso";

const NODEPORT = { protocol: "http:", hostname: "10.0.0.5", host: "10.0.0.5:30001" };
const INGRESS = { protocol: "https:", hostname: "mimir.example.test", host: "mimir.example.test" };

describe("sso helpers", () => {
    it("browserIssuer keeps a real issuer and maps the localhost:8085 default to NodePort 30085", () => {
        expect(browserIssuer("https://sso.example.test", INGRESS)).toBe("https://sso.example.test");
        expect(browserIssuer("http://localhost:8085", NODEPORT)).toBe("http://10.0.0.5:30085");
    });

    it("browserRedirectUri keeps a configured URI and maps the localhost:3001 default to this dashboard", () => {
        expect(browserRedirectUri("https://mimir.example.test/login/callback", NODEPORT)).toBe(
            "https://mimir.example.test/login/callback"
        );
        expect(browserRedirectUri("http://localhost:3001/login/callback", NODEPORT)).toBe(
            "http://10.0.0.5:30001/login/callback"
        );
    });

    it("postLogoutRedirectUri is /login on the origin login redirects back to", () => {
        expect(postLogoutRedirectUri("https://mimir.example.test/login/callback", NODEPORT)).toBe(
            "https://mimir.example.test/login"
        );
        expect(postLogoutRedirectUri("http://localhost:3001/login/callback", NODEPORT)).toBe("http://10.0.0.5:30001/login");
        expect(postLogoutRedirectUri("", INGRESS)).toBe("https://mimir.example.test/login");
    });

    it("endSessionUrl targets /oidc/v1/end_session and encodes its parameters", () => {
        const url = new URL(
            endSessionUrl({
                issuer: "https://sso.example.test/",
                clientId: "c1",
                postLogoutRedirectUri: "https://mimir.example.test/login",
                idTokenHint: "a.b.c",
            })
        );
        expect(`${url.origin}${url.pathname}`).toBe("https://sso.example.test/oidc/v1/end_session");
        expect(url.searchParams.get("client_id")).toBe("c1");
        expect(url.searchParams.get("post_logout_redirect_uri")).toBe("https://mimir.example.test/login");
        expect(url.searchParams.get("id_token_hint")).toBe("a.b.c");
    });

    it("endSessionUrl leaves id_token_hint out when there is none", () => {
        const url = new URL(
            endSessionUrl({ issuer: "https://sso.example.test", clientId: "c1", postLogoutRedirectUri: "https://x.test/login" })
        );
        expect(url.searchParams.has("id_token_hint")).toBe(false);
    });
});
