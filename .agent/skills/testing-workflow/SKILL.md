---
name: testing-workflow
description: Standard testing and verification workflow for Project Mimir — covering test preparation, execution, result recording, Git synchronization, and GitHub PR integration. Triggers when running tests, verifying features, documenting test results, or performing QA activities.
---

# Testing Workflow Skill

This skill defines the complete testing lifecycle for Project Mimir, from preparation through execution to documentation and Git synchronization.

## Testing Pipeline

```
Test Preparation → Execution → Recording → Git Sync → GitHub Integration → Cleanup
```

## Step 1: Test Preparation

1. **Identify Test Scope**: Determine which features/bugs need testing from the current Sprint scope
2. **Issue Check**: Ensure a GitHub Issue exists for every feature/bug being tested. If not, create one using `mcp_github-mcp-server` tools
3. **Locate Test Script**: Find or create the Sprint's test script `docs/iso_29110/si/SI_04_{N}_Sprint{N}_TestScript.md`
4. **Assign Test IDs**: Use the naming convention:
   - `TC_SP{sprint}_{category}{number}`
   - Categories: `F`(Frontend Build), `U`(Unit/Feature), `R`(Regression), `I`(Integration), `P`(Performance), `S`(Security)

## Step 2: Execution

### Backend Tests (Rust)
```bash
# Run all tests
cd ro-ai-bridge && cargo test

# Run specific module tests
cargo test --lib routes::knowledge

# Run with output for debugging
cargo test -- --nocapture
```

### Frontend Tests (Next.js)
```bash
# Build verification (mandatory first test)
cd ro-ai-dashboard && npx next build

# Run unit tests
npm test

# Run specific test file
npm test -- --testPathPattern="knowledge"
```

### E2E Browser Tests
- Use the browser subagent tool to perform visual verification
- Navigate to the relevant page, interact with UI elements, capture screenshots
- Document navigation steps precisely for reproducibility

## Step 3: Recording Results

Update the test script document with:
1. **Result column**: Use `✅ Pass` or `❌ Fail` emoji markers
2. **หมายเหตุ (Notes)**: Add context — error messages, retry attempts, workarounds
3. **Summary table**: Update totals for Category × Total × Pass × Fail

### On Test Failure
1. Document the failure with exact error output
2. Create or update a GitHub Issue for the failure
3. If fixable in current sprint scope, switch to fix → re-test cycle
4. If not fixable, log in Issue/Change Logs of `PM_02_Status_Reports.md`

## Step 4: Git Synchronization

1. **Branch**: Create/use branch `test/TS-{id}-{description}` or `fix/issue-{N}-{desc}`
2. **Commit**: Commit BOTH code changes AND updated test script `.md` files together
3. **Push**: Push branch to remote

## Step 5: GitHub Integration

1. **Pull Request**: Create PR with description summarizing test results
2. **Linking**: Use `Closes #{IssueID}` in PR body to auto-close Issues
3. **Issue Comment**: Post final test result summary as comment on the GitHub Issue

## Step 6: Cleanup

After PR merge:
1. Switch to `main` branch locally
2. Pull latest from remote
3. Delete the feature/test branch locally and remotely

## Test Naming Convention Reference

| Category       | Prefix | Example      | Description                  |
| -------------- | ------ | ------------ | ---------------------------- |
| Frontend Build | `F`    | `TC_SP21_F1` | Next.js build verification   |
| Unit/Feature   | `U`    | `TC_SP21_U3` | Feature-specific test        |
| Regression     | `R`    | `TC_SP21_R2` | Existing feature still works |
| Integration    | `I`    | `TC_SP21_I1` | Cross-component test         |
| Performance    | `P`    | `TC_SP21_P1` | Polling, latency, load tests |
| Security       | `S`    | `TC_SP21_S1` | Auth, tenant isolation, ACL  |

See `examples/test_naming_convention.md` for detailed naming rules.
