import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import Cookies from "js-cookie";
import { Navbar } from "./navbar";
import { navigateTo } from "@/lib/browser-nav";
import { fetchTenants } from "@/lib/api";

jest.mock("next/navigation", () => ({
    usePathname: () => "/sources",
    useRouter: () => ({ push: jest.fn(), replace: jest.fn() }),
}));

jest.mock("js-cookie", () => ({
    __esModule: true,
    default: {
        get: jest.fn((key: string) => {
            if (key === "access_token") return "test-token";
            if (key === "user_name") return "Test User";
            if (key === "tenant_id") return "tenant-1";
            return undefined;
        }),
        set: jest.fn(),
        remove: jest.fn(),
    },
}));

// No network: the tenant list comes from a mock.
jest.mock("@/lib/api", () => ({
    fetchTenants: jest.fn().mockResolvedValue([{ id: "tenant-1", name: "Tenant One" }]),
    fetchMyTenants: jest.fn().mockResolvedValue([]),
}));

jest.mock("@/lib/browser-nav", () => ({
    navigateTo: jest.fn(),
}));

describe("Navbar logout", () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    it("sends the browser to the server logout route, which ends the SSO session", async () => {
        render(<Navbar />);
        await screen.findByText("Tenant One");

        fireEvent.click(screen.getByTitle("Logout"));

        expect(navigateTo).toHaveBeenCalledTimes(1);
        expect(navigateTo).toHaveBeenCalledWith("/api/auth/logout");
    });

    it("leaves cookie clearing to the server route (it can also clear the httpOnly id_token)", async () => {
        render(<Navbar />);
        await screen.findByText("Tenant One");

        fireEvent.click(screen.getByTitle("Logout"));

        expect(Cookies.remove).not.toHaveBeenCalled();
        expect(fetchTenants).toHaveBeenCalled();
    });
});
