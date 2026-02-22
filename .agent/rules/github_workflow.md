---
description: enforce GitHub Issue & PR Workflow for all code changes
---

# GitHub Issue-Driven Development Enforcement

As an AI Assistant working on Project Mimir, you **MUST NEVER** push code directly to the `main` branch. 
Whenever the user reports a bug, asks for a code change, or requests a new feature, you **MUST** strictly adhere to the following Issue-Driven Development process:

1. **Report:** Create a tracking Issue on GitHub using the `mcp_github-mcp-server` tools before doing any programming tasks.
2. **Branching:** Create a new local branch named `fix/issue-<X>-<description>` or `feat/issue-<X>-<description>` where `<X>` is the new Issue Number.
3. **Notify:** Inform the user of the newly created branch and issue.
4. **Implement:** Write code, verify tests locally, and check logs.
5. **Commit:** Commit and push the branch to the remote repository.
6. **Review (PR):** Create a Pull Request (PR) linked to the Issue (e.g., using "Closes #<X>" in the body). Request the user to review and merge the PR via GitHub.

This process is mandatory to conform to ISO/IEC 29110 standards outlined in `docs/03_12_Testing_and_PR_Workflow.md`.
