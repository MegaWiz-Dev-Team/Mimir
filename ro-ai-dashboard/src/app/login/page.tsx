"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Cookies from "js-cookie";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export default function LoginPage() {
    const router = useRouter();
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [error, setError] = useState("");
    const [isLoading, setIsLoading] = useState(false);
    const [showForgotDialog, setShowForgotDialog] = useState(false);

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

            const { token: access_token, tenant_id } = await res.json();

            // Store in cookies
            if (access_token) Cookies.set("access_token", access_token, { expires: 1 }); // 1 day
            if (tenant_id) Cookies.set("tenant_id", tenant_id, { expires: 1 });

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

                    <div className="flex justify-end -mt-4">
                        <button
                            type="button"
                            onClick={() => setShowForgotDialog(true)}
                            className="text-sm text-blue-600 hover:text-blue-800 hover:underline dark:text-blue-400 dark:hover:text-blue-300 transition-colors"
                        >
                            Forgot Password?
                        </button>
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

            <Dialog open={showForgotDialog} onOpenChange={setShowForgotDialog}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Emergency Password Reset</DialogTitle>
                        <DialogDescription>
                            Given this is an on-premise installation, standard email-based password recovery is disabled.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="py-4 space-y-4">
                        <p className="text-sm text-gray-700 dark:text-gray-300">
                            To reset the Administrator password, you must have Bash or SSH Console access to the system running the backend.
                        </p>
                        <div className="bg-gray-100 dark:bg-zinc-950 p-4 rounded-md border border-gray-200 dark:border-zinc-800">
                            <p className="text-xs font-mono text-gray-800 dark:text-gray-200">
                                cd /Volumes/T7\&nbsp;Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge<br />
                                <span className="text-blue-600 dark:text-blue-400 mt-2 block">cargo run --bin reset_admin admin "NewPassword123!"</span>
                            </p>
                        </div>
                        <p className="text-sm text-gray-700 dark:text-gray-300 items-center">
                            Replace <code className="bg-gray-200 dark:bg-zinc-800 px-1.5 py-0.5 rounded text-xs mx-1">admin</code> and <code className="bg-gray-200 dark:bg-zinc-800 px-1.5 py-0.5 rounded text-xs mx-1">"NewPassword123!"</code> with your targeted username and new secure password.
                        </p>
                    </div>
                    <DialogFooter>
                        <Button onClick={() => setShowForgotDialog(false)}>Understood</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

        </div>
    );
}
