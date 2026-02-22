---
description: Standard Testing and GitHub Synchronization Workflow
---
// turbo-all
# Standard Testing & GitHub Sync Workflow

Follow these steps for every test scenario to ensure consistency between code, documentation, and GitHub tracking.

## 1. Test Preparation
- **Identify**: Select a Test ID from [SI_04_1_Sprint1_TestScript.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/iso_29110/si/SI_04_1_Sprint1_TestScript.md).
- **Issue Check**: Ensure a GitHub Issue exists for the feature/bug related to this test. If not, create one.

## 2. Execution & Recording
- **Manual/Auto Test**: Perform the steps defined in the Test Script.
- **Update Documentation**: 
  - Mark `Pass/Fail` in the `.md` file.
  - Add detailed notes if multiple attempts were needed.
  - Attach screenshot paths if applicable.

## 3. Code & Git Synchronization
- **Branching**: Create a unique branch for the fix/test (e.g., `test/TS-1.2-failed-login`).
- **Commit**: Commit both code changes AND the updated `.md` test script.
- **Push**: Push branch to origin.

## 4. GitHub Integration
- **Pull Request**: Create a PR with a description summarized from the test notes.
- **Linking**: Use `Closes #IssueID` in the PR body.
- **Issue Comment**: Summarize the final result in a comment on the original Issue.

## 5. Deployment & Cleanup
- **Merge**: Merge the PR after review.
- **Local Sync**: Switch to `main`, pull origin, and delete the feature branch.
