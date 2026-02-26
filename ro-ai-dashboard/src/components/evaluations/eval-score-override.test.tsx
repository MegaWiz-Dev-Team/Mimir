import { render, screen, fireEvent } from "@testing-library/react";
import { EvalScoreOverride } from "./eval-score-override";
import "@testing-library/jest-dom";

describe("EvalScoreOverride Component", () => {
    it("renders the override trigger button initially", () => {
        render(
            <EvalScoreOverride
                scoreId={1}
                initialAccuracy={5}
                initialCompleteness={4}
                initialRelevance={3}
                initialNotes="Test"
                onSaved={jest.fn()}
            />
        );
        expect(screen.getByRole("button", { name: /override/i })).toBeInTheDocument();
    });

    it("shows editing mode when clicked", () => {
        render(
            <EvalScoreOverride
                scoreId={1}
                initialAccuracy={5}
                initialCompleteness={4}
                initialRelevance={3}
                initialNotes="Test Notes"
                onSaved={jest.fn()}
            />
        );
        fireEvent.click(screen.getByRole("button", { name: /override/i }));

        expect(screen.getByText("Human Override")).toBeInTheDocument();
        expect(screen.getByDisplayValue("5")).toBeInTheDocument(); // Acc
        expect(screen.getByDisplayValue("4")).toBeInTheDocument(); // Comp
        expect(screen.getByDisplayValue("3")).toBeInTheDocument(); // Rel
        expect(screen.getByDisplayValue("Test Notes")).toBeInTheDocument(); // Notes
    });
});
