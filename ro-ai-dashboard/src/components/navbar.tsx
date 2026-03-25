"use client";

import { useState, useEffect, useRef } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import Cookies from "js-cookie";
import {
    LogOut, LayoutDashboard, Database, ShieldCheck, Link as LinkIcon,
    Bot, Settings, BookOpen, BarChart3, Activity, Brain, MessageSquare,
    Share2, ChevronDown, Search, FlaskConical, Users, Building2, Boxes
} from "lucide-react";
import { fetchTenants, fetchMyTenants, Tenant } from "@/lib/api";

type NavItem = {
    name: string;
    href: string;
    icon: React.ComponentType<{ className?: string }>;
};

type NavGroup = {
    label: string;
    icon: React.ComponentType<{ className?: string }>;
    items: NavItem[];
};

function DropdownGroup({ group, pathname }: { group: NavGroup; pathname: string }) {
    const [open, setOpen] = useState(false);
    const ref = useRef<HTMLDivElement>(null);
    const timeoutRef = useRef<NodeJS.Timeout | null>(null);

    const isGroupActive = group.items.some(
        (item) => pathname === item.href || (item.href !== "/" && pathname.startsWith(item.href))
    );

    const handleMouseEnter = () => {
        if (timeoutRef.current) clearTimeout(timeoutRef.current);
        setOpen(true);
    };

    const handleMouseLeave = () => {
        timeoutRef.current = setTimeout(() => setOpen(false), 150);
    };

    useEffect(() => {
        return () => {
            if (timeoutRef.current) clearTimeout(timeoutRef.current);
        };
    }, []);

    const Icon = group.icon;

    return (
        <div
            ref={ref}
            className="relative"
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
        >
            <button
                onClick={() => setOpen(!open)}
                className={`flex items-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${isGroupActive
                    ? "bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-400"
                    : "text-gray-600 hover:bg-gray-100 dark:text-zinc-400 dark:hover:bg-zinc-900"
                    }`}
            >
                <Icon className="w-4 h-4" />
                {group.label}
                <ChevronDown className={`w-3 h-3 transition-transform ${open ? "rotate-180" : ""}`} />
            </button>

            {open && (
                <div className="absolute top-full left-0 mt-1 w-52 bg-white dark:bg-zinc-900 border border-gray-200 dark:border-zinc-700 rounded-lg shadow-lg py-1 z-50 animate-in fade-in-0 zoom-in-95 duration-100">
                    {group.items.map((item) => {
                        const ItemIcon = item.icon;
                        const isActive = pathname === item.href || (item.href !== "/" && pathname.startsWith(item.href));
                        return (
                            <Link
                                key={item.name}
                                href={item.href}
                                onClick={() => setOpen(false)}
                                className={`flex items-center gap-2.5 px-3 py-2 text-sm transition-colors ${isActive
                                    ? "bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-400"
                                    : "text-gray-600 hover:bg-gray-50 dark:text-zinc-400 dark:hover:bg-zinc-800"
                                    }`}
                            >
                                <ItemIcon className="w-4 h-4 shrink-0" />
                                {item.name}
                            </Link>
                        );
                    })}
                </div>
            )}
        </div>
    );
}

// Decode JWT payload without verification (just to read claims client-side)
function decodeJwtPayload(token: string): Record<string, any> | null {
    try {
        const parts = token.split(".");
        if (parts.length !== 3) return null;
        const payload = JSON.parse(atob(parts[1]));
        return payload;
    } catch {
        return null;
    }
}

const YGGDRASIL_ISSUER = process.env.NEXT_PUBLIC_YGGDRASIL_ISSUER || "http://localhost:8085";
const OIDC_CLIENT_ID = process.env.NEXT_PUBLIC_YGGDRASIL_CLIENT_ID || "";

export function Navbar() {
    const pathname = usePathname();
    const router = useRouter();
    const [mounted, setMounted] = useState(false);
    const [tenants, setTenants] = useState<Tenant[]>([]);
    const [userRole, setUserRole] = useState<string>("viewer");
    const [userName, setUserName] = useState<string>("");

    useEffect(() => {
        setMounted(true);
        const token = Cookies.get("access_token");
        if (pathname !== "/login" && token) {
            // Decode JWT to get user role and display name
            const claims = decodeJwtPayload(token);
            const role = claims?.role || "viewer";
            setUserRole(role);
            setUserName(claims?.name || claims?.preferred_username || claims?.email || claims?.sub || "");
        }

        // Always fetch tenants (dev mode works without SSO token)
        if (pathname !== "/login") {
            const isAdminRole = userRole === "admin" || userRole === "SuperAdmin";
            const tenantFetcher = isAdminRole ? fetchTenants : fetchTenants;
            tenantFetcher()
                .then(setTenants)
                .catch(() => setTenants([]));
        }
    }, [pathname]);

    if (!mounted || pathname === "/login") return null;

    const isAdmin = userRole === "admin" || userRole === "SuperAdmin";
    const tenantId = Cookies.get("tenant_id") || "default_tenant";
    const currentTenantName = tenants.find(t => t.id === tenantId)?.name;

    const handleLogout = () => {
        Cookies.remove("access_token");
        Cookies.remove("refresh_token");
        Cookies.remove("tenant_id");

        // Redirect to Yggdrasil end_session to invalidate SSO session
        const postLogoutUri = `${window.location.origin}/login`;
        
        let issuer = YGGDRASIL_ISSUER;
        if (issuer.includes("localhost:8085")) {
             issuer = `${window.location.protocol}//${window.location.hostname}:30085`;
        }
        
        const endSessionUrl = new URL(`${issuer}/oidc/v2/end_session`);
        endSessionUrl.searchParams.set("client_id", OIDC_CLIENT_ID);
        endSessionUrl.searchParams.set("post_logout_redirect_uri", postLogoutUri);
        window.location.href = endSessionUrl.toString();
    };

    const handleTenantChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        Cookies.set("tenant_id", e.target.value);
        window.location.reload();
    };

    // Standalone item
    const overviewItem: NavItem = { name: "Overview", href: "/", icon: LayoutDashboard };

    // Grouped items
    const navGroups: NavGroup[] = [
        {
            label: "Data",
            icon: Database,
            items: [
                { name: "Sources", href: "/sources", icon: LinkIcon },
                { name: "Knowledge", href: "/knowledge", icon: BookOpen },
                { name: "Vector", href: "/vector", icon: Search },
                { name: "Quality", href: "/quality_control", icon: ShieldCheck },
            ],
        },
        {
            label: "AI",
            icon: Brain,
            items: [
                // { name: "Playground", href: "/playground", icon: Bot },
                { name: "RAG Playground", href: "/rag-playground", icon: FlaskConical },
                { name: "Agents", href: "/agents", icon: Brain },
                { name: "Graph", href: "/graph", icon: Share2 },
            ],
        },
        {
            label: "Analytics",
            icon: BarChart3,
            items: [
                { name: "Coverage", href: "/coverage", icon: BarChart3 },
                { name: "LLM Analytics", href: "/analytics/llm", icon: Activity },
                { name: "Evaluations", href: "/evaluations", icon: FlaskConical },
                { name: "Logs", href: "/conversations", icon: MessageSquare },
            ],
        },
        {
            label: "Admin",
            icon: Settings,
            items: [
                { name: "Settings", href: "/settings", icon: Settings },
                { name: "Tenants", href: "/tenants", icon: Building2 },
                { name: "Users", href: "/users", icon: Users },
            ],
        },
    ];

    const isOverviewActive = pathname === "/";

    return (
        <nav className="border-b bg-white dark:bg-zinc-950 dark:border-zinc-800">
            <div className="container mx-auto px-4 h-14 flex items-center justify-between">
                <div className="flex items-center gap-6">
                    <Link href="/" className="font-bold text-xl tracking-tight bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent">
                        Project Mimir
                    </Link>

                    <div className="hidden md:flex items-center gap-0.5">
                        {/* Overview — standalone */}
                        <Link
                            href={overviewItem.href}
                            className={`flex items-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${isOverviewActive
                                ? "bg-blue-50 text-blue-700 dark:bg-blue-900/40 dark:text-blue-400"
                                : "text-gray-600 hover:bg-gray-100 dark:text-zinc-400 dark:hover:bg-zinc-900"
                                }`}
                        >
                            <LayoutDashboard className="w-4 h-4" />
                            Overview
                        </Link>

                        {/* Dropdown groups */}
                        {navGroups
                            .filter((group) => group.label !== "Admin" || isAdmin)
                            .map((group) => (
                                <DropdownGroup key={group.label} group={group} pathname={pathname} />
                            ))}
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

                    {userName && (
                        <span className="text-sm text-gray-600 dark:text-zinc-400 truncate max-w-[140px]" title={userName}>
                            {userName}
                        </span>
                    )}

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
