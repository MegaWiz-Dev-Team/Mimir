---
name: tdd
description: Test-Driven Development (TDD) enforcement for Project Mimir — Red-Green-Refactor cycle for Rust backend (#[cfg(test)] modules) and Next.js frontend (Jest + React Testing Library). Triggers when implementing features, fixing bugs, writing tests, or reviewing code for test coverage.
---

# Test-Driven Development (TDD) Skill

Project Mimir follows strict TDD for all code modifications. This skill ensures the Red-Green-Refactor cycle is followed correctly across both the Rust backend and Next.js frontend.

## The TDD Cycle

```
┌─────────┐     ┌─────────┐     ┌──────────┐
│  🔴 RED  │────▶│ 🟢 GREEN│────▶│ 🔵 REFACTOR│
│Write Test│     │Write Code│     │Clean Up   │
│(it fails)│     │(it passes)│    │(tests pass)│
└─────────┘     └─────────┘     └──────────┘
      ▲                                │
      └────────────────────────────────┘
```

### Step 1: Red — Write the Test First
- Define the expected behavior as an automated test
- Run it to confirm it **fails** (validates the test itself)

### Step 2: Green — Write Minimum Code
- Write the **minimum** implementation to make the test pass
- Do not over-engineer; just make it green

### Step 3: Refactor — Clean Up
- Improve code quality, naming, structure
- Ensure all tests still pass after changes

## Rust Backend TDD

### Inline Unit Tests
Place tests in `#[cfg(test)]` modules within the same file:

```rust
// In src/routes/knowledge.rs

pub async fn list_knowledge(/* ... */) -> impl IntoResponse {
    // implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_knowledge_returns_items() {
        // Arrange
        let pool = setup_test_db().await;
        
        // Act
        let result = list_knowledge(/* ... */).await;
        
        // Assert
        assert!(result.status().is_success());
    }

    #[tokio::test]
    async fn test_list_knowledge_empty_db() {
        // Test edge case: no data
    }
}
```

### Integration Tests
Place in `tests/` directory at the crate root:
```rust
// tests/api_integration.rs
#[tokio::test]
async fn test_full_knowledge_pipeline() {
    // Test cross-module interactions
}
```

### Running Rust Tests
```bash
cd ro-ai-bridge

# All tests
cargo test

# Specific module
cargo test --lib routes::knowledge

# With output
cargo test -- --nocapture

# Single test
cargo test test_list_knowledge_returns_items
```

## Next.js Frontend TDD

### Component Tests
Place test files alongside components (`component.test.tsx`):

```typescript
// src/app/knowledge/page.test.tsx
import { render, screen } from '@testing-library/react';
import KnowledgePage from './page';

describe('KnowledgePage', () => {
  it('renders knowledge table', () => {
    render(<KnowledgePage />);
    expect(screen.getByRole('table')).toBeInTheDocument();
  });

  it('shows QA status badge for processed chunks', () => {
    // Arrange with mock data
    // Assert badge renders correctly
  });
});
```

### Running Frontend Tests
```bash
cd ro-ai-dashboard

# All tests
npm test

# Watch mode
npm test -- --watch

# Specific file
npm test -- --testPathPattern="knowledge"

# Coverage report
npm test -- --coverage
```

## Rules

1. **No untested code**: Never propose implementation without corresponding tests
2. **Test before code**: Write the test FIRST, verify it fails, then write code
3. **Run tests**: ALWAYS execute tests using terminal tools and verify they pass
4. **Document tests**: Update ISO test scripts (SI-04) with test outcomes

## Exceptions — When TDD Can Be Bypassed

TDD can ONLY be bypassed if:
- The user **explicitly** requests to skip tests
- The change is **strictly** documentation (`.md` files only)
- The change is infrastructure/config with no behavioral impact (e.g., Docker, CI/CD)
- The change is a non-functional refactoring that doesn't alter behavior

Even with exceptions, note the skip in the Sprint report.

## TDD + ISO Integration

After TDD cycle completes:
1. Update `SI_04_{N}_Sprint{N}_TestScript.md` with test results
2. Link test cases to GitHub Issues
3. Include test counts in `PM_02_{N}_Sprint{N}_Report.md` testing summary

See `examples/rust_tdd_example.md` for a complete walkthrough.
