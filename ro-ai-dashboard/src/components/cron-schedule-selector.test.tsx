import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import { CronScheduleSelector, ScheduleOption } from "./cron-schedule-selector";

describe("CronScheduleSelector", () => {
    const defaultProps = {
        value: "Manual" as ScheduleOption,
        onChange: jest.fn(),
    };

    beforeEach(() => {
        jest.clearAllMocks();
    });

    it("renders with the current value displayed", () => {
        render(<CronScheduleSelector {...defaultProps} />);
        expect(screen.getByText("Manual")).toBeInTheDocument();
    });

    it("renders with Hourly value", () => {
        render(<CronScheduleSelector {...defaultProps} value="Hourly" />);
        expect(screen.getByText("Hourly")).toBeInTheDocument();
    });

    it("opens dropdown on click and shows all options", () => {
        render(<CronScheduleSelector {...defaultProps} />);
        fireEvent.click(screen.getByRole("button"));
        expect(screen.getByTestId("schedule-option-Manual")).toBeInTheDocument();
        expect(screen.getByTestId("schedule-option-Hourly")).toBeInTheDocument();
        expect(screen.getByTestId("schedule-option-Daily")).toBeInTheDocument();
        expect(screen.getByTestId("schedule-option-Weekly")).toBeInTheDocument();
    });

    it("calls onChange when an option is selected", () => {
        const onChange = jest.fn();
        render(<CronScheduleSelector {...defaultProps} onChange={onChange} />);
        fireEvent.click(screen.getByRole("button"));
        fireEvent.click(screen.getByTestId("schedule-option-Daily"));
        expect(onChange).toHaveBeenCalledWith("Daily");
    });

    it("closes dropdown after selection", () => {
        render(<CronScheduleSelector {...defaultProps} />);
        fireEvent.click(screen.getByRole("button"));
        expect(screen.getByTestId("schedule-option-Daily")).toBeInTheDocument();
        fireEvent.click(screen.getByTestId("schedule-option-Daily"));
        expect(screen.queryByTestId("schedule-option-Daily")).not.toBeInTheDocument();
    });

    it("is disabled when disabled prop is true", () => {
        render(<CronScheduleSelector {...defaultProps} disabled />);
        expect(screen.getByRole("button")).toBeDisabled();
    });
});
