"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Cookies from "js-cookie";

/**
 * OIDC Callback — exchanges authorization code for tokens.
 *
 * Flow: Yggdrasil redirects here with ?code=...&state=...
 *       → verify state matches
 *       → POST to /api/auth/callback (server-side token exchange)
 *       → store access_token in cookie
 *       → redirect to dashboard
 */

export default function CallbackPage() {
    const router = useRouter();
    const [error, setError] = useState("");
    const [status, setStatus] = useState("Exchanging authorization code...");

    useEffect(() => {
        const params = new URLSearchParams(window.location.search);
        const code = params.get("code");
        const state = params.get("state");
        const errorParam = params.get("error");

        if (errorParam) {
            setError(params.get("error_description") || errorParam);
            return;
        }

        if (!code) {
            setError("No authorization code received");
            return;
        }

        // Verify state
        const savedState = sessionStorage.getItem("oidc_state");
        if (state !== savedState) {
            setError("Invalid state parameter — possible CSRF attack");
            return;
        }

        const codeVerifier = sessionStorage.getItem("oidc_code_verifier") || "";

        // Exchange code for token via server-side API route
        (async () => {
            try {
                setStatus("Exchanging tokens...");
                const res = await fetch("/api/auth/callback", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ code, code_verifier: codeVerifier, redirect_uri: `${window.location.origin}/login/callback` }),
                });

                if (!res.ok) {
                    const data = await res.json().catch(() => ({}));
                    throw new Error(data.error || `Token exchange failed (${res.status})`);
                }

                const { access_token, id_token, refresh_token, expires_in } = await res.json();

                // Store token in cookie (use id_token or access_token)
                const token = access_token || id_token;
                if (token) {
                    const days = expires_in ? expires_in / 86400 : 1;
                    Cookies.set("access_token", token, { expires: days });
                }

                // Store refresh_token for silent token refresh
                if (refresh_token) {
                    Cookies.set("refresh_token", refresh_token, { expires: 30 }); // 30 days
                }

                // Clean up PKCE values
                sessionStorage.removeItem("oidc_code_verifier");
                sessionStorage.removeItem("oidc_state");

                setStatus("Login successful! Redirecting...");
                router.push("/");
                router.refresh();
            } catch (e: any) {
                setError(e.message);
            }
        })();
    }, [router]);

    return (
        <div className="flex items-center justify-center min-h-screen bg-gray-50 dark:bg-zinc-950">
            <div className="w-full max-w-md p-8 bg-white dark:bg-zinc-900 rounded-xl shadow-lg border border-gray-200 dark:border-zinc-800">
                <div className="text-center mb-6">
                    <h1 className="text-2xl font-bold bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                        Project-Mimir
                    </h1>
                </div>

                {error ? (
                    <div className="space-y-4">
                        <div className="p-3 bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 rounded-lg text-sm">
                            {error}
                        </div>
                        <button
                            onClick={() => (window.location.href = "/login")}
                            className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors"
                        >
                            Back to Login
                        </button>
                    </div>
                ) : (
                    <div className="text-center space-y-4">
                        <div className="flex justify-center">
                            <div className="animate-spin h-8 w-8 border-4 border-blue-500 border-t-transparent rounded-full" />
                        </div>
                        <p className="text-gray-500 dark:text-zinc-400 text-sm">{status}</p>
                    </div>
                )}
            </div>
        </div>
    );
}
