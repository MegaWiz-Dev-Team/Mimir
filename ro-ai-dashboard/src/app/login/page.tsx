"use client";

import { useEffect, useState } from "react";

/**
 * Mimir Dashboard Login — Yggdrasil OIDC Redirect
 *
 * Hard cutover: replaces username/password with Yggdrasil SSO.
 * Redirects immediately to Yggdrasil authorize endpoint.
 */

import { API_BASE_URL } from "@/lib/api";

const YGGDRASIL_ISSUER_FALLBACK = process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";

function generateCodeVerifier(): string {
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    return btoa(String.fromCharCode(...array))
        .replace(/\+/g, "-")
        .replace(/\//g, "_")
        .replace(/=/g, "");
}

async function generateCodeChallenge(verifier: string): Promise<{ challenge: string, method: string }> {
    if (typeof crypto !== 'undefined' && crypto.subtle) {
        const encoder = new TextEncoder();
        const data = encoder.encode(verifier);
        const digest = await crypto.subtle.digest("SHA-256", data);
        const challenge = btoa(String.fromCharCode(...new Uint8Array(digest)))
            .replace(/\+/g, "-")
            .replace(/\//g, "_")
            .replace(/=/g, "");
        return { challenge, method: "S256" };
    }
    return { challenge: verifier, method: "plain" };
}

function generateState(): string {
    const array = new Uint8Array(16);
    crypto.getRandomValues(array);
    return Array.from(array, (b) => b.toString(16).padStart(2, "0")).join("");
}

export default function LoginPage() {
    const [error, setError] = useState("");

    useEffect(() => {
        const params = new URLSearchParams(window.location.search);
        const errMsg = params.get("error");
        if (errMsg) {
            setError(decodeURIComponent(errMsg));
            return;
        }

        (async () => {
            try {
                let ssoConfig: { issuer: string, client_id: string, redirect_uri: string };
                try {
                    const res = await fetch(`${API_BASE_URL}/auth/sso-config`);
                    if (!res.ok) throw new Error("Failed to load SSO configuration");
                    ssoConfig = await res.json();
                } catch (e) {
                    throw new Error("Unable to contact backend for SSO configuration");
                }

                if (!ssoConfig.client_id) {
                    setError("SSO Client ID is not configured in Vault. Please contact Administrator.");
                    return;
                }

                const codeVerifier = generateCodeVerifier();
                const { challenge: codeChallenge, method } = await generateCodeChallenge(codeVerifier);
                const state = generateState();

                sessionStorage.setItem("oidc_code_verifier", codeVerifier);
                sessionStorage.setItem("oidc_state", state);
                
                let issuer = ssoConfig.issuer || YGGDRASIL_ISSUER_FALLBACK;
                if (issuer.includes("localhost:8085")) {
                    issuer = `${window.location.protocol}//${window.location.hostname}:30085`;
                }

                let redirectUri = ssoConfig.redirect_uri;
                if (redirectUri.includes("localhost:3001")) {
                    redirectUri = `${window.location.protocol}//${window.location.host}/login/callback`;
                }

                const authUrl = new URL(`${issuer}/oauth/v2/authorize`);
                authUrl.searchParams.set("client_id", ssoConfig.client_id);
                authUrl.searchParams.set("redirect_uri", redirectUri);
                authUrl.searchParams.set("response_type", "code");
                authUrl.searchParams.set("scope", "openid profile email offline_access urn:zitadel:iam:org:project:roles urn:zitadel:iam:org:project:id:365685843395920403:aud");
                authUrl.searchParams.set("state", state);
                authUrl.searchParams.set("code_challenge", codeChallenge);
                authUrl.searchParams.set("code_challenge_method", method);

                window.location.href = authUrl.toString();
            } catch (e: any) {
                setError(e.message || "Failed to initiate login");
            }
        })();
    }, []);

    return (
        <div className="flex items-center justify-center min-h-screen bg-gray-50 dark:bg-zinc-950">
            <div className="w-full max-w-md p-8 bg-white dark:bg-zinc-900 rounded-xl shadow-lg border border-gray-200 dark:border-zinc-800">
                <div className="text-center mb-8">
                    <h1 className="text-2xl font-bold bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                        Asgard AI Platform
                    </h1>
                    <p className="text-gray-500 dark:text-zinc-400 mt-2">
                        {error ? "Login Error" : "Redirecting to Asgard SSO..."}
                    </p>
                </div>

                {error ? (
                    <div className="space-y-4">
                        <div className="p-3 bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 rounded-lg text-sm">
                            {error}
                        </div>
                        <button
                            onClick={() => {
                                setError("");
                                window.location.href = "/login";
                            }}
                            className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors"
                        >
                            Try Again
                        </button>
                    </div>
                ) : (
                    <div className="flex justify-center">
                        <div className="animate-spin h-8 w-8 border-4 border-blue-500 border-t-transparent rounded-full" />
                    </div>
                )}
            </div>
        </div>
    );
}
