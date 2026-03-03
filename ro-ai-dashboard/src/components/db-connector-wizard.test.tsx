import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import { DbConnectorWizard } from "./db-connector-wizard";

// Mock next/navigation
jest.mock("next/navigation", () => ({
    usePathname: () => "/sources",
    useRouter: () => ({ push: jest.fn() }),
}));

// Mock API calls
jest.mock("@/lib/api", () => ({
    testDbConnection: jest.fn(),
    discoverDbSchema: jest.fn(),
    importDbData: jest.fn(),
}));

describe("DbConnectorWizard", () => {
    const defaultProps = {
        open: true,
        onOpenChange: jest.fn(),
        onImportComplete: jest.fn(),
    };

    it("renders wizard dialog when open", () => {
        render(<DbConnectorWizard {...defaultProps} />);
        expect(screen.getByTestId("db-connector-wizard")).toBeInTheDocument();
        expect(screen.getByText("External Database Import")).toBeInTheDocument();
    });

    it("shows database type selection in step 1", () => {
        render(<DbConnectorWizard {...defaultProps} />);
        expect(screen.getByTestId("db-type-mysql")).toBeInTheDocument();
        expect(screen.getByTestId("db-type-postgres")).toBeInTheDocument();
        expect(screen.getByTestId("db-type-sqlite")).toBeInTheDocument();
    });

    it("selects database type on click", () => {
        render(<DbConnectorWizard {...defaultProps} />);
        fireEvent.click(screen.getByTestId("db-type-postgres"));
        const btn = screen.getByTestId("db-type-postgres");
        expect(btn.className).toContain("border-purple-500");
    });

    it("Next button is disabled without required fields", () => {
        render(<DbConnectorWizard {...defaultProps} />);
        const nextBtn = screen.getByText("Next");
        expect(nextBtn.closest("button")).toBeDisabled();
    });

    it("does not render when open is false", () => {
        render(<DbConnectorWizard {...defaultProps} open={false} />);
        expect(screen.queryByTestId("db-connector-wizard")).not.toBeInTheDocument();
    });
});
