---
description: enforce Test-Driven Development (TDD) for all code modifications
---

# Test-Driven Development (TDD) Enforcement

As a core process in Project Mimir, you MUST follow Test-Driven Development (TDD) principles whenever creating new features or fixing bugs. This applies to both the React/Next.js frontend and the Rust backend.

## The TDD Process

1. **Write the Test First**: Before writing or modifying any implementation code, write an automated test that defines the desired functionality or reproduces the bug.
2. **Watch the Test Fail**: Run the newly created test to see it fail (Red phase). This confirms the test is valid and testing the right thing.
3. **Write the Implementation**: Write the minimum amount of code necessary to make the test pass (Green phase).
4. **Refactor**: Clean up the code while ensuring the tests continue to pass.

## Rules
- **No untested code**: Never propose a code implementation without also providing the corresponding tests.
- **Frontend (Next.js)**: Use `jest` and React Testing Library for components and pages. Place tests alongside components (e.g., `component.test.tsx`).
- **Backend (Rust)**: Use `#[cfg(test)]` modules within the same file for unit tests, or the `tests/` directory for integration tests.
- **Execution**: You must run the tests using your terminal execution tools (`npm test`, `cargo test`) and verify they pass before assuming the task is complete.
- **Exceptions**: TDD can only be bypassed if the user explicitly requests to skip writing tests, or if the change is strictly documentation, infrastructure, or non-functional refactoring that does not alter behavior.
