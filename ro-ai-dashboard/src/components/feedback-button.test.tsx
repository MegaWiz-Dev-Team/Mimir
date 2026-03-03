import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import { FeedbackButton } from "./feedback-button";

// Mock next/navigation
jest.mock("next/navigation", () => ({
    usePathname: () => "/sources",
}));

// Mock js-cookie
jest.mock("js-cookie", () => ({
    get: jest.fn((key: string) => {
        if (key === "access_token") return "test-token";
        if (key === "tenant_id") return "test-tenant";
        return undefined;
    }),
}));

// Mock API
jest.mock("@/lib/api", () => ({
    submitFeedbackReport: jest.fn().mockResolvedValue({ feedback_id: 42, github_issue_url: "https://github.com/test/issue/42" }),
}));

describe("FeedbackButton", () => {
    it("renders floating action button", () => {
        render(<FeedbackButton />);
        expect(screen.getByTestId("feedback-fab")).toBeInTheDocument();
    });

    it("opens feedback sheet on FAB click", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        expect(screen.getByTestId("feedback-sheet")).toBeInTheDocument();
        expect(screen.getByText("Send Feedback")).toBeInTheDocument();
    });

    it("shows report type selector with 3 types", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        expect(screen.getByTestId("report-type-bug")).toBeInTheDocument();
        expect(screen.getByTestId("report-type-feedback")).toBeInTheDocument();
        expect(screen.getByTestId("report-type-feature")).toBeInTheDocument();
    });

    it("selects report type on click", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        fireEvent.click(screen.getByTestId("report-type-feature"));
        const btn = screen.getByTestId("report-type-feature");
        expect(btn.className).toContain("border-blue-500");
    });

    it("shows title and description inputs", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        expect(screen.getByTestId("feedback-title")).toBeInTheDocument();
        expect(screen.getByTestId("feedback-description")).toBeInTheDocument();
    });

    it("submit button is disabled without title", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        const submitBtn = screen.getByText("Submit Feedback");
        expect(submitBtn.closest("button")).toBeDisabled();
    });

    it("auto-captures current page URL", () => {
        render(<FeedbackButton />);
        fireEvent.click(screen.getByTestId("feedback-fab"));
        expect(screen.getByText(/\/sources/)).toBeInTheDocument();
    });
});
