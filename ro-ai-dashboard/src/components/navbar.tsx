"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import Cookies from "js-cookie";
import { LogOut, LayoutDashboard, Database, ShieldCheck, Link as LinkIcon, Bot, Settings, BookOpen, BarChart3, Activity, Brain, MessageSquare } from "lucide-react";
import { fetchTenants, Tenant } from "@/lib/api";

export function Navbar() {
    const pathname = usePathname();
    const router = useRouter();
    const [mounted, setMounted] = useState(false);

    const [tenants, setTenants] = useState<Tenant[]>([]);

    useEffect(() => {
        setMounted(true);
        fetchTenants()
            .then(setTenants)
            .catch(() => setTenants([]));
    }, []);

    if (!mounted || pathname === "/login") return null;

    const tenantId = Cookies.get("tenant_id") || "default_tenant";
    const currentTenantName = tenants.find(t => t.id === tenantId)?.name;

    const handleLogout = () => {
        Cookies.remove("access_token");
        Cookies.remove("tenant_id");
        router.push("/login");
    };

    const handleTenantChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        Cookies.set("tenant_id", e.target.value);
        window.location.reload();
    };

    const navItems = [
        { name: "Overview", href: "/", icon: LayoutDashboard },
        { name: "Sources", href: "/sources", icon: LinkIcon },
        { name: "Knowledge", href: "/knowledge", icon: BookOpen },
        { name: "Quality", href: "/quality_control", icon: ShieldCheck },
        { name: "Playground", href: "/playground", icon: Bot },
        { name: "Agents", href: "/agents", icon: Brain },
        { name: "Logs", href: "/conversations", icon: MessageSquare },
        { name: "Coverage", href: "/coverage", icon: BarChart3 },
        { name: "Analytics", href: "/analytics/llm", icon: Activity },
        { name: "Admin", href: "/settings", icon: Settings },
    ];

    return (
        <nav className="border-b bg-white dark:bg-zinc-950 dark:border-zinc-800">
            <div className="container mx-auto px-4 h-16 flex items-center justify-between">
                <div className="flex items-center gap-8">
                    <Link href="/" className="font-bold text-xl tracking-tight bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                        Project-Mimir
                    </Link>

                    <div className="hidden md:flex items-center gap-1">
                        {navItems.map((item) => {
                            const Icon = item.icon;
                            const isActive = pathname === item.href || (item.href !== "/" && pathname.startsWith(item.href));
                            return (
                                <Link
                                    key={item.name}
                                    href={item.href}
                                    className={`flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${isActive
                                        ? "bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-400"
                                        : "text-gray-600 hover:bg-gray-100 dark:text-zinc-400 dark:hover:bg-zinc-900"
                                        }`}
                                >
                                    <Icon className="w-4 h-4" />
                                    {item.name}
                                </Link>
                            );
                        })}
                    </div>
                </div>

                <div className="flex items-center gap-4">
                    <div className="flex items-center gap-2">
                        <span className="text-sm text-gray-500 dark:text-zinc-400">Tenant:</span>
                        <select
                            value={tenants.some(t => t.id === tenantId) ? tenantId : ""}
                            onChange={handleTenantChange}
                            className="text-sm bg-gray-50 border border-gray-300 text-gray-900 rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-1.5 dark:bg-zinc-900 dark:border-zinc-700 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500 outline-none"
                        >
                            {!tenants.some(t => t.id === tenantId) && (
                                <option value={tenantId}>{currentTenantName || tenantId}</option>
                            )}
                            {tenants.map(t => (
                                <option key={t.id} value={t.id}>{t.name}</option>
                            ))}
                        </select>
                    </div>

                    <button
                        onClick={handleLogout}
                        className="p-2 text-gray-500 hover:text-red-600 hover:bg-red-50 dark:text-zinc-400 dark:hover:text-red-400 dark:hover:bg-red-900/20 rounded-md transition-colors"
                        title="Logout"
                    >
                        <LogOut className="w-5 h-5" />
                    </button>
                </div>
            </div>
        </nav>
    );
}
