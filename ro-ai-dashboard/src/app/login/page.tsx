"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Cookies from "js-cookie";

export default function LoginPage() {
    const router = useRouter();
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [error, setError] = useState("");
    const [isLoading, setIsLoading] = useState(false);

    async function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        setIsLoading(true);
        setError("");

        try {
            const res = await fetch(`${process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api"}/v1/auth/login`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username, password }),
            });

            if (!res.ok) {
                const data = await res.json().catch(() => ({}));
                throw new Error(data.error || "Login failed");
            }

            const { access_token, tenant_id } = await res.json();

            // Store in cookies
            Cookies.set("access_token", access_token, { expires: 1 }); // 1 day
            Cookies.set("tenant_id", tenant_id, { expires: 1 });

            router.push("/");
            router.refresh();
        } catch (err: any) {
            setError(err.message);
        } finally {
            setIsLoading(false);
        }
    }

    return (
        <div className="flex items-center justify-center min-h-screen bg-gray-50 dark:bg-zinc-950">
            <div className="w-full max-w-md p-8 bg-white dark:bg-zinc-900 rounded-xl shadow-lg border border-gray-200 dark:border-zinc-800">
                <div className="text-center mb-8">
                    <h1 className="text-2xl font-bold bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">Project-Mimir</h1>
                    <p className="text-gray-500 dark:text-zinc-400 mt-2">Sign in to your account</p>
                </div>

                {error && (
                    <div className="mb-4 p-3 bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 rounded-lg text-sm">
                        {error}
                    </div>
                )}

                <form onSubmit={handleSubmit} className="space-y-6">
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-zinc-300 mb-1">Username</label>
                        <input
                            type="text"
                            required
                            className="w-full px-4 py-2 rounded-lg border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-gray-900 dark:text-zinc-100 focus:ring-2 focus:ring-blue-500 outline-none transition-shadow"
                            value={username}
                            onChange={(e) => setUsername(e.target.value)}
                        />
                    </div>

                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-zinc-300 mb-1">Password</label>
                        <input
                            type="password"
                            required
                            className="w-full px-4 py-2 rounded-lg border border-gray-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-gray-900 dark:text-zinc-100 focus:ring-2 focus:ring-blue-500 outline-none transition-shadow"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                        />
                    </div>

                    <button
                        type="submit"
                        disabled={isLoading}
                        className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50"
                    >
                        {isLoading ? "Signing in..." : "Sign in"}
                    </button>
                </form>
            </div>
        </div>
    );
}
