---
name: code-review
description: Issue-driven development and code review workflow for Project Mimir — GitHub Issue creation, branch naming conventions, PR templates, self-review checklists, and PR integration. Triggers when creating branches, committing code, creating pull requests, reviewing code, or managing GitHub workflow.
---

# Code Review Skill

Project Mimir follows **Issue-Driven Development** — every code change must be tracked through GitHub Issues and Pull Requests. This skill defines the complete workflow from issue creation to PR merge.

## ⚠️ Critical Rule
**NEVER push directly to `main` branch.** All changes go through feature branches and Pull Requests.

## Issue-Driven Development Flow

```
1. Report (Issue) → 2. Branch → 3. Implement → 4. Test → 5. Commit → 6. PR → 7. Review → 8. Merge
```

### Step 1: Create GitHub Issue
Before ANY code work, create a tracking Issue:
```
Title:  [Bug/Feature]: Short description
Labels: bug / feature / enhancement / documentation
Body:   Clear description, steps to reproduce (bugs), acceptance criteria (features)
```

Use `mcp_github-mcp-server` tools:
- `mcp_github-mcp-server_issue_write` with `method: "create"`

### Step 2: Create Branch
Branch naming convention:
```
fix/issue-{N}-{short-description}     # Bug fixes
feat/issue-{N}-{short-description}    # New features
docs/issue-{N}-{short-description}    # Documentation only
test/TS-{id}-{short-description}      # Test-only branches
refactor/issue-{N}-{short-description} # Refactoring
```

Examples:
```
fix/issue-176-tenant-data-leak
feat/issue-179-selective-qa-generation
docs/issue-185-update-api-specs
```

### Step 3: Implement
Follow the `tdd` skill (Red-Green-Refactor cycle) for all code changes.

### Step 4: Test
Follow the `testing-workflow` skill for test execution and documentation.

### Step 5: Commit
Commit message format:
```
{type}(#{issue}): {description}

{optional body with details}
```

Types: `fix`, `feat`, `docs`, `test`, `refactor`, `chore`, `style`

Examples:
```
feat(#179): add QA status column to knowledge table
fix(#176): filter knowledge queries by tenant_id
docs(#185): update Sprint 21 test script
```

### Step 6: Create Pull Request

Use `mcp_github-mcp-server_create_pull_request` with:
- **Title**: Same as commit message format
- **Body**: Use PR template (see `examples/pr_template.md`)
- **Base**: `main`
- **Head**: Your feature branch

### Step 7: Self-Review Checklist

Before requesting review, verify:

#### Code Quality
- [ ] Code follows Rust/Next.js patterns defined in `rust-backend-patterns` skill
- [ ] No hardcoded secrets or API keys
- [ ] Error handling is proper (no leaked internal errors)
- [ ] All `tenant_id` filters are in place for protected routes

#### Testing
- [ ] All new code has corresponding tests (TDD)
- [ ] `cargo test` passes (backend)
- [ ] `npx next build` passes (frontend)
- [ ] Test script (SI-04) is updated with results

#### Documentation
- [ ] Sprint report (PM-02) updated if sprint is complete
- [ ] Issue/Change Logs updated if applicable
- [ ] Code comments for non-obvious logic

#### Security
- [ ] No SQL injection vulnerabilities (use `sqlx::query!` or `QueryBuilder`)
- [ ] JWT token validation in place
- [ ] Tenant isolation verified

### Step 8: Merge & Cleanup

After PR is approved:
1. Merge the PR (squash merge preferred)
2. Verify Issue is auto-closed via `Closes #{N}` in PR body
3. Locally: `git checkout main && git pull && git branch -d {branch}`

## PR Comment Best Practices
- Add a summary comment on the Issue with final test results
- Reference specific test IDs: "ผ่านการทดสอบ TC_SP21_U1 - TC_SP21_U5 ✅"
- Link to the Sprint report if available

See `examples/pr_template.md` for the standard PR description template.
