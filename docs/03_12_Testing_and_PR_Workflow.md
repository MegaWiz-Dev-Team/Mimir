# GitHub Issue & PR Workflow (Issue-Driven Development)

This document outlines the standard operating procedure for reporting test results, tracking bugs, and merging code into the `main` branch for Project Mimir. We follow an **Issue-Driven Development** approach to maintain a clean and traceable project history.

## 1. Opening an Issue (Reporting & Tracking)

Before writing any code or fixing a bug, an Issue must be created on GitHub. This acts as a central hub for discussion and tracking.

### A. Reporting Test Results (E2E)
When performing a large-scale test (e.g., E2E Testing for Phase 1-9), use the provided test templates:
1. Go to the GitHub repository and click **New Issue**.
2. Copy the contents of the relevant test template (e.g., `tests/e2e/issue_template_phase_1_9_e2e.md`).
3. Paste it into the issue description.
4. Name the issue clearly, e.g., `[E2E] Test Plan: Project Mimir (Phase 1-9)`.
5. Check off the `[x]` boxes as you progress through the tests.
6. If a specific test scenario fails, leave a comment on the issue with the Error Log and details.

### B. Reporting Bugs or New Features
1. Open a new issue.
2. Prefix the title with `[Bug]` or `[Feature]`. Example: `[Bug] TS-04 Action Heal does not trigger visual effects`.
3. Provide clear steps to reproduce the bug or detailed requirements for the feature.

---

## 2. Working on the Issue (Branching)

Once an issue is created and assigned, do not work directly on the `main` branch. Create a new branch dedicated to solving that specific issue.

1. Fetch latest changes: `git checkout main && git pull origin main`
2. Create a new branch. The naming convention should reflect the issue type and ID.
   - For bugs: `git checkout -b fix/issue-#-short-description` (e.g., `fix/issue-12-action-heal`)
   - For features: `git checkout -b feat/issue-#-short-description` (e.g., `feat/issue-15-ai-gm`)
3. Write your code and make commits. Keep commit messages clear and concise.

---

## 3. Submitting a Pull Request (PR)

When the work is complete and tested locally, it's time to merge the code back into the `main` branch.

1. Push your branch to GitHub: `git push origin HEAD`
2. Open a **Pull Request (PR)** on GitHub, comparing your branch against `main`.
3. **Crucial Step (Linking the Issue):** In the PR description, you MUST include a linking keyword followed by the Issue number. This tells GitHub to automatically close the issue when the PR is merged.
   - Example 1: `Closes #12` (Closes issue 12)
   - Example 2: `Fixes #15` (Fixes issue 15)
   - Example 3: `Resolves #1` (Resolves issue 1)
4. **Attach Proof of Testing:** Include evidence that your code works. This is highly recommended for all PRs.
   - Paste a checklist of tests performed.
   - Attach screenshots of the UI.
   - Attach short video clips of the feature in action (e.g., in-game rAthena NPC interaction).

---

## 4. Code Review and Merging

1. Assign a team member to review the PR.
2. If changes are requested, make the changes on your local branch, commit, and push again (the PR will update automatically).
3. Once approved, click **Merge pull request** (Usually Squash and Merge is preferred to keep the history clean).
4. GitHub will automatically close the linked Issue.
5. Delete the branch on GitHub and locally to keep the repository tidy.

---

## Summary of the Flow
`Create Issue` ➔ `Create Branch (fix/issue-X)` ➔ `Write Code` ➔ `Push` ➔ `Create PR (Closes #X)` ➔ `Review` ➔ `Merge` ➔ `Issue Auto-Closed`.
